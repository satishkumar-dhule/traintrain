//! State-of-the-art observability: Prometheus /metrics, instantaneous CPU/RSS
//! sampling, and an in-memory structured-log ring that backs the dashboard's
//! live log stream (`/rail-api/logs`) without needing an external log shipper.
//!
//! Instrumentation follows Google SRE:
//! - Pattern: RED — Rate, Errors, Duration per microservice (Tom Wilkie)
//! - Pattern: USE — Utilization, Saturation, Errors per resource (Brendan Gregg)
//! - Pattern: Saturation — CPU, memory, in_flight, RPS vs thresholds
//! - Pattern: SLO — availability SLI, error budget, burn rate (SRE Ch.4)

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use prometheus::{
    Encoder, Gauge, GaugeVec, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
    TextEncoder,
};
use serde_json::{json, Value};
use tracing::field::Visit;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use crate::core::metrics::MetricsSnapshot;

// ---------------------------------------------------------------------------
// Proc stats: instantaneous CPU fraction (0.0..1.0 of one core) and RSS bytes.
// ---------------------------------------------------------------------------

static CPU_SAMPLE: Mutex<Option<(Instant, u64)>> = Mutex::new(None);

/// Sample CPU usage (fraction of one core) and RSS in bytes from
/// `/proc/self/stat` + `/proc/self/statm`. The CPU value is an instantaneous
/// delta between consecutive calls (first call returns 0.0). Falls back to
/// `(0.0, 0)` honestly when `/proc` is unavailable (non-Linux).
pub fn proc_stats() -> (f64, u64) {
    let stat = std::fs::read_to_string("/proc/self/stat").ok();
    let statm = std::fs::read_to_string("/proc/self/statm").ok();

    let mem = statm
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|pages| pages * 4096)
        .unwrap_or(0);

    let ticks = stat.as_ref().and_then(|s| utime_plus_stime(s)).unwrap_or(0);

    let now = Instant::now();
    let cpu = {
        let mut last = CPU_SAMPLE.lock().unwrap();
        match last.take() {
            Some((prev_t, prev_ticks)) => {
                let elapsed = now.duration_since(prev_t).as_secs_f64();
                // CLK_TCK is 100 on Linux; a full core = elapsed_secs * 100 ticks.
                let available = elapsed * 100.0;
                let delta = ticks.saturating_sub(prev_ticks) as f64;
                let frac = if available > 0.0 {
                    (delta / available).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                *last = Some((now, ticks));
                frac
            }
            None => {
                *last = Some((now, ticks));
                0.0
            }
        }
    };

    (cpu, mem)
}

/// Field 14 (utime) + field 15 (stime) in clock ticks. The `comm` field can
/// contain spaces inside parens, so we split after the closing paren.
fn utime_plus_stime(stat: &str) -> Option<u64> {
    let end = stat.find(')')?;
    let rest = stat[end + 1..].trim_start();
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?; // 14th field overall - 3 prefix
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some(utime.saturating_add(stime))
}

// ---------------------------------------------------------------------------
// Prometheus /metrics: the industry-standard scrape format, exposed at
// `GET /metrics` so any Prometheus/Grafana stack can ingest it.
// ---------------------------------------------------------------------------

/// Prometheus registry + collectors. Instances are `Arc`-shared through
/// `AppState`; every layer records into it via `record_http` and the sampler
/// task refreshes the gauges via `sample`.
pub struct Telemetry {
    registry: Registry,
    // --- Core HTTP / process ---
    http_requests: IntCounterVec,
    http_duration_seconds: HistogramVec,
    http_in_flight: IntGauge,
    http_requests_rps: Gauge,
    http_latency_ms: Gauge,
    process_uptime_seconds: IntGauge,
    process_cpu_fraction: Gauge,
    process_rss_bytes: Gauge,
    cache_hits_total: IntCounterVec,
    cache_entries: IntGauge,
    source_latency_ms: GaugeVec,
    source_samples_total: IntCounterVec,
    /// Circuit breaker gauges per source.
    circuit_state: GaugeVec,
    circuit_failures: GaugeVec,
    circuit_open_seconds: GaugeVec,
    // --- Pattern: SLO ---
    /// Pattern: SLO — availability SLI (0.0..1.0)
    slo_availability: Gauge,
    /// Pattern: SLO — error budget remaining (0.0..1.0)
    slo_error_budget_remaining: Gauge,
    /// Pattern: SLO — error budget consumed (0.0..inf)
    slo_error_budget_consumed: Gauge,
    /// Pattern: SLO — burn rate (error_rate / budget)
    slo_burn_rate: Gauge,
    /// Pattern: SLO — target availability constant (0.999)
    slo_availability_target: Gauge,
    // --- Pattern: Saturation ---
    /// Pattern: Saturation — cpu fraction (0.0..1.0) headroom vs 0.80 threshold
    saturation_cpu: Gauge,
    /// Pattern: Saturation — memory in MB vs 2048 threshold
    saturation_memory: Gauge,
    /// Pattern: Saturation — memory bytes raw
    saturation_memory_bytes: Gauge,
    /// Pattern: Saturation — in-flight requests vs 1000 threshold
    saturation_inflight: Gauge,
    /// Pattern: Saturation — rps vs 500 threshold
    saturation_rps: Gauge,
    // --- Pattern: RED ---
    /// Pattern: RED — Rate is http_requests_rps; Duration is histogram; Errors are counters/ratios below.
    red_errors_total: IntCounterVec,
    /// Pattern: RED — 5xx ratio gauge
    red_error_ratio_5xx: Gauge,
    /// Pattern: RED — 4xx ratio gauge
    red_error_ratio_4xx: Gauge,
    /// Pattern: RED — overall error ratio
    red_error_ratio: Gauge,
    /// Pattern: RED — explicit RPS gauge alias for RED dashboard
    red_requests_per_second: Gauge,
    // --- Pattern: USE ---
    /// Pattern: USE — Utilization: CPU fraction
    use_cpu: Gauge,
    /// Pattern: USE — Utilization: memory bytes
    use_memory_bytes: Gauge,
    /// Pattern: USE — Utilization: memory MB
    use_memory_mb: Gauge,
    /// Pattern: USE — Saturation: in-flight
    use_saturation_inflight: Gauge,
    /// Pattern: USE — Saturation: rps
    use_saturation_rps: Gauge,
    /// Pattern: USE — Errors: circuit open count
    use_circuit_open: Gauge,
    /// Pattern: USE — Errors: total circuit-open transitions (counter via gauge)
    use_errors_total: IntGauge,
    // --- Resilience ---
    /// Pattern: Rate Limiting — counter for 429 responses
    rate_limited_total: IntCounter,
    /// Pattern: Load Shedding — counter for 503 responses
    load_shed_total: IntCounter,
    /// Pattern: Bulkhead — counter for bulkhead rejections (503 due to concurrency)
    bulkhead_rejected_total: IntCounter,
    /// Pattern: Health Checks — readiness gauge (1 ready, 0 not ready)
    railway_ready: Gauge,
    /// Pattern: Capacity Planning — recommendation gauge
    railway_capacity_recommendation: Gauge,
    /// Pattern: Capacity Planning — saturated count
    railway_capacity_saturated: Gauge,
    /// last-seen totals so `sample` can inc counters by delta (safe to call on
    /// every scrape AND from the background sampler without double counting)
    last_cache_hits: std::sync::atomic::AtomicU64,
    last_cache_misses: std::sync::atomic::AtomicU64,
    last_source_samples: Mutex<HashMap<String, u64>>,
}

impl Telemetry {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let http_requests = IntCounterVec::new(
            Opts::new("railway_http_requests_total", "Total HTTP requests served"),
            &["method", "path", "status"],
        )?;
        let http_duration_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "railway_http_duration_seconds",
                "HTTP request processing time",
            )
            .buckets(vec![
                0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
            ]),
            &["path"],
        )?;
        let http_in_flight =
            IntGauge::new("railway_http_in_flight", "Requests currently in flight")?;
        let http_requests_rps =
            Gauge::new("railway_http_requests_per_second", "Recent request rate")?;
        let http_latency_ms = Gauge::new("railway_http_latency_ms", "EMA of request latency (ms)")?;
        let process_uptime_seconds =
            IntGauge::new("railway_process_uptime_seconds", "Process uptime")?;
        let process_cpu_fraction = Gauge::new(
            "railway_process_cpu_fraction",
            "Instantaneous CPU usage (fraction of one core)",
        )?;
        let process_rss_bytes = Gauge::new("railway_process_rss_bytes", "Resident set size")?;
        let cache_hits_total = IntCounterVec::new(
            Opts::new("railway_cache_total", "Cache lookups by outcome"),
            &["outcome"],
        )?;
        let cache_entries = IntGauge::new("railway_cache_entries", "Live cache entry count")?;
        let source_latency_ms = GaugeVec::new(
            Opts::new("railway_source_latency_ms", "Average upstream latency"),
            &["source"],
        )?;
        let source_samples_total = IntCounterVec::new(
            Opts::new(
                "railway_source_samples_total",
                "Successful upstream samples recorded",
            ),
            &["source"],
        )?;
        let circuit_state = GaugeVec::new(
            Opts::new(
                "railway_circuit_state",
                "Circuit breaker state (1 = active, 0 = inactive) per source and state",
            ),
            &["source", "state"],
        )?;
        let circuit_failures = GaugeVec::new(
            Opts::new(
                "railway_circuit_failures",
                "Consecutive failures per source for the circuit breaker",
            ),
            &["source"],
        )?;
        let circuit_open_seconds = GaugeVec::new(
            Opts::new(
                "railway_circuit_open_seconds",
                "Seconds since circuit opened per source (0 when closed)",
            ),
            &["source"],
        )?;

        // Pattern: SLO — gauges derived from MetricsSnapshot via crate::core::sre
        let slo_availability = Gauge::new(
            "railway_slo_availability",
            "SLO availability SLI (2xx+3xx)/total, Pattern: SLO",
        )?;
        let slo_error_budget_remaining = Gauge::new(
            "railway_slo_error_budget_remaining",
            "Error budget remaining 0.0..1.0, Pattern: SLO",
        )?;
        let slo_error_budget_consumed = Gauge::new(
            "railway_slo_error_budget_consumed",
            "Error budget consumed fraction, Pattern: SLO",
        )?;
        let slo_burn_rate = Gauge::new(
            "railway_slo_burn_rate",
            "Burn rate = error_rate / budget, Pattern: SLO",
        )?;
        let slo_availability_target = Gauge::new(
            "railway_slo_availability_target",
            "SLO target (0.999), Pattern: SLO",
        )?;

        // Pattern: Saturation
        let saturation_cpu = Gauge::new(
            "railway_saturation_cpu",
            "CPU saturation fraction 0.0..1.0 vs 0.80 threshold, Pattern: Saturation",
        )?;
        let saturation_memory = Gauge::new(
            "railway_saturation_memory",
            "Memory saturation MB vs 2048 threshold, Pattern: Saturation",
        )?;
        let saturation_memory_bytes = Gauge::new(
            "railway_saturation_memory_bytes",
            "Memory saturation bytes, Pattern: Saturation",
        )?;
        let saturation_inflight = Gauge::new(
            "railway_saturation_inflight",
            "In-flight requests vs 1000 threshold, Pattern: Saturation",
        )?;
        let saturation_rps = Gauge::new(
            "railway_saturation_rps",
            "RPS vs 500 threshold, Pattern: Saturation",
        )?;

        // Pattern: RED
        let red_errors_total = IntCounterVec::new(
            Opts::new(
                "railway_red_errors_total",
                "RED Errors - total per status class (4xx,5xx), Pattern: RED",
            ),
            &["status_class"],
        )?;
        let red_error_ratio_5xx = Gauge::new(
            "railway_red_error_ratio_5xx",
            "RED 5xx error ratio 0.0..1.0, Pattern: RED",
        )?;
        let red_error_ratio_4xx = Gauge::new(
            "railway_red_error_ratio_4xx",
            "RED 4xx error ratio 0.0..1.0, Pattern: RED",
        )?;
        let red_error_ratio = Gauge::new(
            "railway_red_error_ratio",
            "RED total error ratio (4xx+5xx)/total, Pattern: RED",
        )?;
        let red_requests_per_second = Gauge::new(
            "railway_red_requests_per_second",
            "RED Rate - requests per second, Pattern: RED",
        )?;

        // Pattern: USE
        let use_cpu = Gauge::new(
            "railway_use_cpu",
            "USE Utilization CPU fraction, Pattern: USE",
        )?;
        let use_memory_bytes = Gauge::new(
            "railway_use_memory_bytes",
            "USE Utilization memory bytes, Pattern: USE",
        )?;
        let use_memory_mb = Gauge::new(
            "railway_use_memory_mb",
            "USE Utilization memory MB, Pattern: USE",
        )?;
        let use_saturation_inflight = Gauge::new(
            "railway_use_saturation_inflight",
            "USE Saturation in-flight, Pattern: USE",
        )?;
        let use_saturation_rps = Gauge::new(
            "railway_use_saturation_rps",
            "USE Saturation RPS, Pattern: USE",
        )?;
        let use_circuit_open = Gauge::new(
            "railway_use_circuit_open",
            "USE Errors - circuit open count, Pattern: USE",
        )?;
        let use_errors_total = IntGauge::new(
            "railway_use_errors_total",
            "USE Errors - failover open count, Pattern: USE",
        )?;

        // Pattern: Rate Limiting — token bucket per IP, returns 429
        let rate_limited_total = IntCounter::new(
            "railway_rate_limited_total",
            "Total requests rate limited (429), Pattern: Rate Limiting",
        )?;
        // Pattern: Load Shedding — shed when saturated, returns 503
        let load_shed_total = IntCounter::new(
            "railway_load_shed_total",
            "Total requests shed due to load (503), Pattern: Load Shedding",
        )?;
        // Pattern: Bulkhead — concurrency limit, returns 503 when saturated
        let bulkhead_rejected_total = IntCounter::new(
            "railway_bulkhead_rejected_total",
            "Total requests rejected by bulkhead (503), Pattern: Bulkhead",
        )?;

        // Pattern: Health Checks — readiness gauge (1 ready, 0 not ready)
        let railway_ready = Gauge::new(
            "railway_ready",
            "Readiness probe (1 = ready, 0 = not ready), Pattern: Health Checks",
        )?;
        // Pattern: Capacity Planning — recommendation gauge (1 scale_up, 0 ok, -1 scale_down)
        let railway_capacity_recommendation = Gauge::new(
            "railway_capacity_recommendation",
            "Capacity recommendation (1 = scale_up, 0 = ok, -1 = scale_down), Pattern: Capacity Planning",
        )?;
        // Pattern: Capacity Planning — saturated count (how many saturation signals breached)
        let railway_capacity_saturated = Gauge::new(
            "railway_capacity_saturated",
            "Number of saturated signals (0..4), Pattern: Capacity Planning",
        )?;

        // initialize SLO target const
        slo_availability_target.set(crate::core::sre::SLO_AVAILABILITY_TARGET);
        // default ready unknown -> 1 (assume ready until probed; readiness handler will set accurately)
        railway_ready.set(1.0);
        railway_capacity_recommendation.set(0.0);

        for c in [
            Box::new(http_requests.clone()) as Box<dyn prometheus::core::Collector>,
            Box::new(http_duration_seconds.clone()),
            Box::new(http_in_flight.clone()),
            Box::new(http_requests_rps.clone()),
            Box::new(http_latency_ms.clone()),
            Box::new(process_uptime_seconds.clone()),
            Box::new(process_cpu_fraction.clone()),
            Box::new(process_rss_bytes.clone()),
            Box::new(cache_hits_total.clone()),
            Box::new(cache_entries.clone()),
            Box::new(source_latency_ms.clone()),
            Box::new(source_samples_total.clone()),
            Box::new(circuit_state.clone()),
            Box::new(circuit_failures.clone()),
            Box::new(circuit_open_seconds.clone()),
            Box::new(slo_availability.clone()),
            Box::new(slo_error_budget_remaining.clone()),
            Box::new(slo_error_budget_consumed.clone()),
            Box::new(slo_burn_rate.clone()),
            Box::new(slo_availability_target.clone()),
            Box::new(saturation_cpu.clone()),
            Box::new(saturation_memory.clone()),
            Box::new(saturation_memory_bytes.clone()),
            Box::new(saturation_inflight.clone()),
            Box::new(saturation_rps.clone()),
            Box::new(red_errors_total.clone()),
            Box::new(red_error_ratio_5xx.clone()),
            Box::new(red_error_ratio_4xx.clone()),
            Box::new(red_error_ratio.clone()),
            Box::new(red_requests_per_second.clone()),
            Box::new(use_cpu.clone()),
            Box::new(use_memory_bytes.clone()),
            Box::new(use_memory_mb.clone()),
            Box::new(use_saturation_inflight.clone()),
            Box::new(use_saturation_rps.clone()),
            Box::new(use_circuit_open.clone()),
            Box::new(use_errors_total.clone()),
            Box::new(rate_limited_total.clone()),
            Box::new(load_shed_total.clone()),
            Box::new(bulkhead_rejected_total.clone()),
            Box::new(railway_ready.clone()),
            Box::new(railway_capacity_recommendation.clone()),
            Box::new(railway_capacity_saturated.clone()),
        ] {
            registry.register(c)?;
        }

        Ok(Self {
            registry,
            http_requests,
            http_duration_seconds,
            http_in_flight,
            http_requests_rps,
            http_latency_ms,
            process_uptime_seconds,
            process_cpu_fraction,
            process_rss_bytes,
            cache_hits_total,
            cache_entries,
            source_latency_ms,
            source_samples_total,
            circuit_state,
            circuit_failures,
            circuit_open_seconds,
            slo_availability,
            slo_error_budget_remaining,
            slo_error_budget_consumed,
            slo_burn_rate,
            slo_availability_target,
            saturation_cpu,
            saturation_memory,
            saturation_memory_bytes,
            saturation_inflight,
            saturation_rps,
            red_errors_total,
            red_error_ratio_5xx,
            red_error_ratio_4xx,
            red_error_ratio,
            red_requests_per_second,
            use_cpu,
            use_memory_bytes,
            use_memory_mb,
            use_saturation_inflight,
            use_saturation_rps,
            use_circuit_open,
            use_errors_total,
            rate_limited_total,
            load_shed_total,
            bulkhead_rejected_total,
            railway_ready,
            railway_capacity_recommendation,
            railway_capacity_saturated,
            last_cache_hits: std::sync::atomic::AtomicU64::new(0),
            last_cache_misses: std::sync::atomic::AtomicU64::new(0),
            last_source_samples: Mutex::new(HashMap::new()),
        })
    }

    /// Pattern: Rate Limiting — increment counter when 429 emitted
    pub fn inc_rate_limited(&self) {
        self.rate_limited_total.inc();
    }

    /// Pattern: Load Shedding — increment counter when 503 shed
    pub fn inc_load_shed(&self) {
        self.load_shed_total.inc();
    }

    /// Pattern: Bulkhead — increment counter when bulkhead rejects
    pub fn inc_bulkhead_rejected(&self) {
        self.bulkhead_rejected_total.inc();
    }

    /// Pattern: Health Checks — set readiness gauge (1 ready, 0 not ready)
    pub fn set_ready(&self, ready: bool) {
        self.railway_ready.set(if ready { 1.0 } else { 0.0 });
    }

    /// Pattern: Health Checks — read readiness gauge
    pub fn ready_value(&self) -> f64 {
        self.railway_ready.get()
    }

    /// Pattern: Capacity Planning — set recommendation gauge
    /// rec: "scale_up" => 1, "scale_down" => -1, "ok" => 0
    pub fn set_capacity_recommendation(&self, rec: &str) {
        let v = match rec {
            "scale_up" => 1.0,
            "scale_down" => -1.0,
            _ => 0.0,
        };
        self.railway_capacity_recommendation.set(v);
    }

    pub fn capacity_recommendation_value(&self) -> f64 {
        self.railway_capacity_recommendation.get()
    }

    pub fn set_capacity_saturated(&self, n: f64) {
        self.railway_capacity_saturated.set(n);
    }

    pub fn capacity_saturated_value(&self) -> f64 {
        self.railway_capacity_saturated.get()
    }

    /// Record one finished HTTP request (called by the shared metrics middleware).
    /// Pattern: RED — observes Duration histogram and increments Rate/Errors counters.
    pub fn record_http(&self, method: &str, path: &str, status: u16, duration: Duration) {
        self.http_requests
            .with_label_values(&[method, path, &status.to_string()])
            .inc();
        self.http_duration_seconds
            .with_label_values(&[path])
            .observe(duration.as_secs_f64());

        // Pattern: RED — Errors split by class
        let class = match status {
            400..=499 => Some("4xx"),
            500..=599 => Some("5xx"),
            _ => None,
        };
        if let Some(c) = class {
            self.red_errors_total.with_label_values(&[c]).inc();
        }
    }

    /// Refresh gauges from a metrics snapshot + live process readings.
    /// Called by the background sampler and on every `/metrics` scrape, so a
    /// Prometheus/Grafana stack always sees fresh values. Counters are
    /// incremented by the delta since the last call (never double counted).
    ///
    /// Pattern: RED — Rate (RPS), Errors (ratios), Duration (histogram already observed in record_http)
    /// Pattern: USE — Utilization (cpu, mem), Saturation (in_flight, rps), Errors via failover gauges
    /// Pattern: Saturation — cpu/mem/inflight/rps vs thresholds
    /// Pattern: SLO — availability, error budget, burn rate from snapshot
    pub fn sample(
        &self,
        snap: &MetricsSnapshot,
        cpu_fraction: f64,
        rss_bytes: u64,
        uptime_secs: u64,
        cache_entries: usize,
    ) {
        self.http_in_flight.set(snap.in_flight as i64);
        self.http_requests_rps.set(snap.req_per_sec);
        self.http_latency_ms.set(snap.latency_ms);
        self.process_uptime_seconds.set(uptime_secs as i64);
        self.process_cpu_fraction.set(cpu_fraction);
        self.process_rss_bytes.set(rss_bytes as f64);
        self.cache_entries.set(cache_entries as i64);

        // Pattern: USE — Utilization mirrors process stats
        self.use_cpu.set(cpu_fraction);
        self.use_memory_bytes.set(rss_bytes as f64);
        let mem_mb = rss_bytes as f64 / (1024.0 * 1024.0);
        self.use_memory_mb.set(mem_mb);

        // Pattern: Saturation — snapshot feeds saturation gauges
        self.saturation_cpu.set(cpu_fraction);
        self.saturation_memory.set(mem_mb);
        self.saturation_memory_bytes.set(rss_bytes as f64);
        self.saturation_inflight.set(snap.in_flight as f64);
        self.saturation_rps.set(snap.req_per_sec);
        // Pattern: USE — Saturation dimensions
        self.use_saturation_inflight.set(snap.in_flight as f64);
        self.use_saturation_rps.set(snap.req_per_sec);

        // Pattern: RED — Rate alias + error ratios from snapshot
        self.red_requests_per_second.set(snap.req_per_sec);
        let ratio_4xx = snap.red_error_ratio_4xx();
        let ratio_5xx = snap.red_error_ratio_5xx();
        self.red_error_ratio_4xx.set(ratio_4xx);
        self.red_error_ratio_5xx.set(ratio_5xx);
        self.red_error_ratio.set(ratio_4xx + ratio_5xx);

        // Pattern: SLO — derived via sre helpers (pure over snapshot)
        let slo = crate::core::sre::SloSnapshot::from_metrics_with_telemetry(
            snap,
            cpu_fraction,
            mem_mb,
        );
        self.slo_availability.set(slo.availability_sli);
        self.slo_error_budget_remaining
            .set(slo.error_budget_remaining);
        self.slo_error_budget_consumed.set(slo.error_budget_consumed);
        self.slo_burn_rate.set(slo.burn_rate);

        let delta = |last: &std::sync::atomic::AtomicU64, total: u64| {
            let prev = last.swap(total, Ordering::Relaxed);
            total.saturating_sub(prev)
        };
        self.cache_hits_total
            .with_label_values(&["hit"])
            .inc_by(delta(&self.last_cache_hits, snap.cache_hits));
        self.cache_hits_total
            .with_label_values(&["miss"])
            .inc_by(delta(&self.last_cache_misses, snap.cache_misses));

        if let Ok(mut last) = self.last_source_samples.lock() {
            for s in &snap.source_latency {
                self.source_latency_ms
                    .with_label_values(&[&s.source])
                    .set(s.avg_latency_ms);
                let prev = last.insert(s.source.clone(), s.samples).unwrap_or(0);
                self.source_samples_total
                    .with_label_values(&[&s.source])
                    .inc_by(s.samples.saturating_sub(prev));
            }
        }
    }

    /// Update circuit breaker gauges from a failover snapshot.
    /// Called from the observability service before returning the JSON payload,
    /// and can also be called from background samplers if needed.
    /// Pattern: USE — Errors dimension is failover circuit open count
    pub fn set_failover_snapshot(&self, snap: &[crate::core::failover::Snapshot]) {
        let mut open_count: f64 = 0.0;
        for s in snap {
            let state = match s.state {
                crate::core::failover::State::Closed => "closed",
                crate::core::failover::State::Open => "open",
                crate::core::failover::State::HalfOpen => "half_open",
            };
            if state == "open" {
                open_count += 1.0;
            }
            for st in ["closed", "open", "half_open"] {
                let v = if st == state { 1.0 } else { 0.0 };
                self.circuit_state
                    .with_label_values(&[&s.source, st])
                    .set(v);
            }
            self.circuit_failures
                .with_label_values(&[&s.source])
                .set(s.consecutive_failures as f64);
            self.circuit_open_seconds
                .with_label_values(&[&s.source])
                .set(s.open_secs.unwrap_or(0) as f64);
        }
        // Pattern: USE — Errors is circuit open count
        self.use_circuit_open.set(open_count);
        self.use_errors_total.set(open_count as i64);
    }

    /// Pattern: SLO — expose SLO snapshot via telemetry for dashboards / alerts.
    /// Builds a fresh `SloSnapshot` from the supplied metrics snapshot and live
    /// proc stats. Mirrors what `sample()` computes but returns it for JSON
    /// endpoints without needing to scrape Prometheus.
    pub fn slo_snapshot(
        &self,
        snap: &MetricsSnapshot,
        cpu_fraction: f64,
        rss_bytes: u64,
    ) -> crate::core::sre::SloSnapshot {
        let mem_mb = rss_bytes as f64 / (1024.0 * 1024.0);
        crate::core::sre::SloSnapshot::from_metrics_with_telemetry(snap, cpu_fraction, mem_mb)
    }

    /// Convenience: build SLO snapshot from snapshot alone (cpu/mem = 0).
    pub fn slo_snapshot_from_metrics(
        &self,
        snap: &MetricsSnapshot,
    ) -> crate::core::sre::SloSnapshot {
        crate::core::sre::SloSnapshot::from_metrics(snap)
    }

    /// Direct accessors for Prom scrape tests / debugging.
    pub fn slo_availability_value(&self) -> f64 {
        self.slo_availability.get()
    }
    pub fn saturation_cpu_value(&self) -> f64 {
        self.saturation_cpu.get()
    }

    /// Render the registry in Prometheus text format (v0.0.4).
    pub fn encode(&self) -> String {
        let mut buffer = Vec::new();
        let encoder = TextEncoder::new();
        let families = self.registry.gather();
        let _ = encoder.encode(&families, &mut buffer);
        String::from_utf8_lossy(&buffer).into_owned()
    }
}

// ---------------------------------------------------------------------------
// Structured-log ring: every `tracing` event is captured as a JSON record and
// retained in a bounded ring so the dashboard can show a live log stream.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntryDto {
    pub ts: i64,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: HashMap<String, Value>,
}

pub struct LogRing {
    inner: Mutex<VecDeque<LogEntryDto>>,
    cap: usize,
}

impl LogRing {
    pub fn new(cap: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(cap)),
            cap,
        }
    }

    fn push(&self, entry: LogEntryDto) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.push_back(entry);
            while inner.len() > self.cap {
                inner.pop_front();
            }
        }
    }

    /// Newest-first snapshot, optionally filtered by minimum level.
    pub fn snapshot(&self, limit: usize, min_level: Option<&str>) -> Vec<LogEntryDto> {
        let min_rank = min_level.and_then(level_rank);
        let mut out = self
            .inner
            .lock()
            .map(|inner| {
                inner
                    .iter()
                    .filter(|e| min_rank.is_none_or(|r| level_rank(&e.level).unwrap_or(0) >= r))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out.reverse();
        out.truncate(limit);
        out
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Global ring shared by the tracing layer and the HTTP log endpoint.
pub fn log_ring() -> &'static Arc<LogRing> {
    static RING: OnceLock<Arc<LogRing>> = OnceLock::new();
    RING.get_or_init(|| Arc::new(LogRing::new(2000)))
}

/// A `tracing_subscriber::Layer` that mirrors every event into the global ring
/// as a JSON record. Register it alongside the fmt layers in `main`.
pub struct LogRingLayer {
    ring: Arc<LogRing>,
}

impl LogRingLayer {
    pub fn new(ring: Arc<LogRing>) -> Self {
        Self { ring }
    }
}

impl<S> Layer<S> for LogRingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut fields = HashMap::new();
        event.record(&mut JsonVisitor { out: &mut fields });
        let message = fields
            .remove("message")
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        self.ring.push(LogEntryDto {
            ts: now_epoch_ms(),
            level: meta.level().as_str().to_string(),
            target: meta.target().to_string(),
            message,
            fields,
        });
    }
}

/// Visit a tracing event's fields into a `HashMap<String, Value>`.
struct JsonVisitor<'a> {
    out: &'a mut HashMap<String, Value>,
}

impl Visit for JsonVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.out
            .insert(field.name().to_string(), Value::String(value.to_string()));
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.out.insert(
            field.name().to_string(),
            Value::String(format!("{value:?}")),
        );
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.out.insert(field.name().to_string(), json!(value));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.out.insert(field.name().to_string(), json!(value));
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.out.insert(field.name().to_string(), json!(value));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.out.insert(field.name().to_string(), json!(value));
    }
    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.record_str(field, &value.to_string());
    }
}

fn level_rank(level: &str) -> Option<u8> {
    match level.to_lowercase().as_str() {
        "trace" | "debug" => Some(0),
        "info" => Some(1),
        "warn" => Some(2),
        "error" => Some(3),
        _ => None,
    }
}

fn now_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
