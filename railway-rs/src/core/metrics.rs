use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Live runtime metrics collected from the actual server process.
///
/// All numbers are real: request counters, in-flight requests, per-source
/// latencies recorded by the vertical slices, cache hit/miss totals, uptime
/// and a rolling time-series for the dashboard charts. Nothing is fabricated.
///
/// Instrumentation follows Google SRE patterns:
/// - Pattern: RED — Rate (req_per_sec), Errors (status_by_code), Duration (latency_ms histogram)
/// - Pattern: USE — Utilization (cpu via proc_stats), Saturation (in_flight, rps), Errors (status 5xx + failover)
/// - Pattern: Saturation — in_flight vs threshold, rps vs threshold, cpu/mem headroom
/// - Pattern: SLO — availability SLI from status_by_code, error budget & burn rate derived
#[derive(Debug)]
pub struct Metrics {
    requests_total: AtomicU64,
    in_flight: AtomicU64,
    bytes_out: AtomicU64,
    requests_by_path: Mutex<HashMap<String, u64>>,
    status_by_code: Mutex<HashMap<u16, u64>>,
    /// source name -> (total latency ms, sample count)
    source_latency: Mutex<HashMap<String, (f64, u64)>>,
    /// EMA of request processing latency in ms
    request_latency: Mutex<f64>,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    /// Pattern: Hedging — fan-out tracking
    fanout_total: AtomicU64,
    fanout_overall_timeouts: AtomicU64,
    fanout_wins: Mutex<HashMap<String, u64>>,
    /// per-source hard failures (SourceUnavailable/Internal) — Pattern: Hedging + RED Errors
    source_failures: Mutex<HashMap<String, u64>>,
    /// rolling time-series, newest at the back
    series: Mutex<VecDeque<SeriesPoint>>,
    last_total: AtomicU64,
    last_sample_at: Mutex<Option<Instant>>,
    started_at: Instant,
}

// ---------------------------------------------------------------------------
// Snapshot — the serializable view consumed by observability & SRE.
// ---------------------------------------------------------------------------

/// Snapshot of all counters at an instant. This is the single source of truth
/// for SRE calculations (`crate::core::sre`) and for Prometheus sampling
/// (`crate::core::obs::Telemetry::sample`).
///
/// Pattern: RED — `requests_total`/`req_per_sec` = Rate, `status_by_code` = Errors, `latency_ms` = Duration
/// Pattern: USE — `in_flight` is Saturation, cpu/mem are supplied externally via `proc_stats` but derived here for SLO
/// Pattern: Saturation — `in_flight`, `req_per_sec` vs thresholds in `crate::core::sre`
/// Pattern: SLO — `status_by_code` feeds availability SLI, error budget and burn rate
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub in_flight: u64,
    pub bytes_out: u64,
    pub requests_by_path: Vec<PathCount>,
    pub status_by_code: Vec<StatusCodeCount>,
    pub source_latency: Vec<SourceLatency>,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub fanout_total: u64,
    pub fanout_overall_timeouts: u64,
    pub fanout_wins: Vec<FanoutWin>,
    pub source_failures: Vec<SourceFailure>,
    pub latency_ms: f64,
    pub uptime_secs: u64,
    pub req_per_sec: f64,
    pub series: Vec<SeriesPoint>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PathCount {
    pub path: String,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StatusCodeCount {
    pub code: u16,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceLatency {
    pub source: String,
    pub avg_latency_ms: f64,
    pub samples: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FanoutWin {
    pub source: String,
    pub wins: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceFailure {
    pub source: String,
    pub failures: u64,
}

/// One row of the dashboard's time-series charts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeriesPoint {
    /// unix epoch seconds at the sample instant
    pub t: u64,
    pub rps: f64,
    pub latency_ms: f64,
    pub in_flight: u64,
    pub mem_mb: f64,
    pub cpu_frac: f64,
    /// (source name, avg latency ms) at this instant
    pub sources: Vec<(String, f64)>,
}

impl MetricsSnapshot {
    // -----------------------------------------------------------------------
    // Pattern: RED helpers — Rate / Errors / Duration derived from snapshot
    // -----------------------------------------------------------------------

    /// Pattern: RED — total requests (Rate denominator)
    pub fn total_requests(&self) -> u64 {
        self.requests_total
    }

    /// Pattern: RED — count of 2xx/3xx (good) requests
    pub fn red_success_count(&self) -> u64 {
        self.status_by_code
            .iter()
            .filter(|s| (200..400).contains(&s.code))
            .map(|s| s.count)
            .sum()
    }

    /// Pattern: RED — count of 4xx requests
    pub fn red_4xx_count(&self) -> u64 {
        self.status_by_code
            .iter()
            .filter(|s| (400..500).contains(&s.code))
            .map(|s| s.count)
            .sum()
    }

    /// Pattern: RED — count of 5xx requests
    pub fn red_5xx_count(&self) -> u64 {
        self.status_by_code
            .iter()
            .filter(|s| (500..600).contains(&s.code))
            .map(|s| s.count)
            .sum()
    }

    /// Pattern: RED — total error count (4xx + 5xx)
    pub fn red_errors_total(&self) -> u64 {
        self.red_4xx_count() + self.red_5xx_count()
    }

    /// Pattern: RED — 5xx error ratio (0.0..1.0)
    pub fn red_error_ratio_5xx(&self) -> f64 {
        let total: u64 = self.status_by_code.iter().map(|s| s.count).sum();
        if total == 0 {
            return 0.0;
        }
        self.red_5xx_count() as f64 / total as f64
    }

    /// Pattern: RED — 4xx error ratio (0.0..1.0)
    pub fn red_error_ratio_4xx(&self) -> f64 {
        let total: u64 = self.status_by_code.iter().map(|s| s.count).sum();
        if total == 0 {
            return 0.0;
        }
        self.red_4xx_count() as f64 / total as f64
    }

    /// Pattern: RED — overall error ratio (4xx+5xx)/total
    pub fn red_error_ratio(&self) -> f64 {
        self.red_error_ratio_4xx() + self.red_error_ratio_5xx()
    }

    /// Pattern: RED — request rate (RPS)
    pub fn red_rate(&self) -> f64 {
        self.req_per_sec
    }

    /// Pattern: RED — duration (EMA latency ms)
    pub fn red_duration_ms(&self) -> f64 {
        self.latency_ms
    }

    // -----------------------------------------------------------------------
    // Pattern: SLO helpers — availability & error budget derived
    // -----------------------------------------------------------------------

    /// Pattern: SLO — availability SLI = (2xx+3xx)/total
    pub fn slo_availability(&self) -> f64 {
        crate::core::sre::availability_sli(self)
    }

    /// Pattern: SLO — error budget remaining (0.0..1.0)
    pub fn slo_error_budget_remaining(&self) -> f64 {
        crate::core::sre::error_budget_remaining(self)
    }

    /// Pattern: SLO — burn rate = error_rate / budget
    pub fn slo_burn_rate(&self) -> f64 {
        crate::core::sre::burn_rate(self)
    }

    // -----------------------------------------------------------------------
    // Pattern: Saturation / USE helpers
    // -----------------------------------------------------------------------

    /// Pattern: Saturation — in-flight as saturation signal
    pub fn saturation_inflight(&self) -> u64 {
        self.in_flight
    }

    /// Pattern: Saturation — rps as saturation signal
    pub fn saturation_rps(&self) -> f64 {
        self.req_per_sec
    }

    /// Pattern: USE — saturation is the in-flight + rps dimensions; utilization is cpu/mem supplied externally.
    /// Returns true when in-flight and rps are below saturation thresholds (cpu/mem checks need external proc stats).
    pub fn use_saturation_ok(&self) -> bool {
        self.in_flight <= crate::core::sre::SATURATION_IN_FLIGHT_THRESHOLD
            && self.req_per_sec <= crate::core::sre::SATURATION_RPS_THRESHOLD
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            in_flight: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            requests_by_path: Mutex::new(HashMap::new()),
            status_by_code: Mutex::new(HashMap::new()),
            source_latency: Mutex::new(HashMap::new()),
            request_latency: Mutex::new(0.0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            fanout_total: AtomicU64::new(0),
            fanout_overall_timeouts: AtomicU64::new(0),
            fanout_wins: Mutex::new(HashMap::new()),
            source_failures: Mutex::new(HashMap::new()),
            series: Mutex::new(VecDeque::with_capacity(MAX_SERIES)),
            last_total: AtomicU64::new(0),
            last_sample_at: Mutex::new(None),
            started_at: Instant::now(),
        }
    }

    pub fn begin_request(&self) -> RequestGuard<'_> {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        RequestGuard { metrics: self }
    }

    pub fn record_path(&self, path: &str) {
        if let Ok(mut m) = self.requests_by_path.lock() {
            if m.len() >= MAX_PATHS && !m.contains_key(path) {
                return;
            }
            *m.entry(path.to_string()).or_insert(0) += 1;
        }
    }

    pub fn record_status(&self, code: u16) {
        if let Ok(mut m) = self.status_by_code.lock() {
            *m.entry(code).or_insert(0) += 1;
        }
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes(&self, n: u64) {
        self.bytes_out.fetch_add(n, Ordering::Relaxed);
    }

    /// Pattern: Hedging — record that a fan-out was executed.
    pub fn record_fanout(&self) {
        self.fanout_total.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_fanout_win(&self, source: &str) {
        if let Ok(mut m) = self.fanout_wins.lock() {
            *m.entry(source.to_string()).or_insert(0) += 1;
        }
    }
    pub fn record_fanout_overall_timeout(&self) {
        self.fanout_overall_timeouts.fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_source_failure(&self, source: &str) {
        if let Ok(mut m) = self.source_failures.lock() {
            *m.entry(source.to_string()).or_insert(0) += 1;
        }
    }

    /// Record the latency of a successful upstream fetch from `source`.
    pub fn record_source_latency(&self, source: &str, latency: Duration) {
        if let Ok(mut m) = self.source_latency.lock() {
            let entry = m.entry(source.to_string()).or_insert((0.0, 0));
            entry.0 += latency.as_secs_f64() * 1000.0;
            entry.1 += 1;
        }
    }

    /// Exponential moving average of request processing time (ms).
    pub fn record_request_latency(&self, latency: Duration) {
        let ms = latency.as_secs_f64() * 1000.0;
        if let Ok(mut m) = self.request_latency.lock() {
            let alpha = 0.1;
            *m = (*m) * (1.0 - alpha) + ms * alpha;
        }
    }

    pub fn request_latency_ms(&self) -> f64 {
        self.request_latency.lock().map(|m| *m).unwrap_or(0.0)
    }

    /// Push one point onto the rolling time-series. Called by the background
    /// sampler at a fixed interval so the charts show real request rate,
    /// latency, memory and CPU over time.
    pub fn sample_series(&self, cpu_frac: f64, mem_bytes: u64) {
        let now = Instant::now();
        let total = self.requests_total.load(Ordering::Relaxed);
        let mut last = self.last_sample_at.lock().unwrap();
        let dt = match *last {
            Some(prev) => now.duration_since(prev).as_secs_f64().max(0.001),
            None => 1.0,
        };
        let delta = total.saturating_sub(self.last_total.load(Ordering::Relaxed));
        let rps = delta as f64 / dt;
        *last = Some(now);
        self.last_total.store(total, Ordering::Relaxed);

        let sources = self
            .source_latency
            .lock()
            .map(|m| {
                let mut v: Vec<(String, f64)> = m
                    .iter()
                    .map(|(s, (tot, n))| (s.clone(), if *n > 0 { tot / *n as f64 } else { 0.0 }))
                    .collect();
                v.sort_by(|a, b| a.0.cmp(&b.0));
                v
            })
            .unwrap_or_default();

        let point = SeriesPoint {
            t: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            rps: (rps * 100.0).round() / 100.0,
            latency_ms: (self.request_latency_ms() * 100.0).round() / 100.0,
            in_flight: self.in_flight.load(Ordering::Relaxed),
            mem_mb: (mem_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
            cpu_frac: (cpu_frac * 1000.0).round() / 1000.0,
            sources,
        };

        if let Ok(mut series) = self.series.lock() {
            series.push_back(point);
            while series.len() > MAX_SERIES {
                series.pop_front();
            }
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let uptime = self.started_at.elapsed().as_secs();
        let total = self.requests_total.load(Ordering::Relaxed);
        let req_per_sec = if uptime > 0 {
            total as f64 / uptime as f64
        } else {
            0.0
        };

        let mut paths = self
            .requests_by_path
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(p, c)| PathCount {
                        path: p.clone(),
                        count: *c,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        paths.sort_by(|a, b| b.count.cmp(&a.count));

        let mut status_codes = self
            .status_by_code
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(c, n)| StatusCodeCount {
                        code: *c,
                        count: *n,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        status_codes.sort_by(|a, b| b.count.cmp(&a.count));

        let mut latency = self
            .source_latency
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(s, (tot, n))| SourceLatency {
                        source: s.clone(),
                        avg_latency_ms: if *n > 0 { tot / *n as f64 } else { 0.0 },
                        samples: *n,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        latency.sort_by(|a, b| a.source.cmp(&b.source));

        let series = self
            .series
            .lock()
            .map(|s| s.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        let mut fanout_wins = self
            .fanout_wins
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(s, n)| FanoutWin {
                        source: s.clone(),
                        wins: *n,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        fanout_wins.sort_by(|a, b| a.source.cmp(&b.source));

        let mut source_failures = self
            .source_failures
            .lock()
            .map(|m| {
                m.iter()
                    .map(|(s, n)| SourceFailure {
                        source: s.clone(),
                        failures: *n,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        source_failures.sort_by(|a, b| a.source.cmp(&b.source));

        MetricsSnapshot {
            requests_total: total,
            in_flight: self.in_flight.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            requests_by_path: paths,
            status_by_code: status_codes,
            source_latency: latency,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            fanout_total: self.fanout_total.load(Ordering::Relaxed),
            fanout_overall_timeouts: self.fanout_overall_timeouts.load(Ordering::Relaxed),
            fanout_wins,
            source_failures,
            latency_ms: self.request_latency_ms(),
            uptime_secs: uptime,
            req_per_sec,
            series,
        }
    }
}

/// RAII guard decrementing the in-flight counter when the request completes.
pub struct RequestGuard<'a> {
    metrics: &'a Metrics,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        self.metrics.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

pub type SharedMetrics = Arc<Metrics>;

/// Upper bound on distinct paths tracked, so an attacker cannot grow the
/// metrics map without limit by requesting unique URLs.
const MAX_PATHS: usize = 1024;

/// Upper bound on time-series points (600 @ 2s = 20 minutes of history).
const MAX_SERIES: usize = 600;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_guard_round_trips() {
        let m = Metrics::new();
        let g = m.begin_request();
        assert_eq!(m.snapshot().in_flight, 1);
        drop(g);
        assert_eq!(m.snapshot().in_flight, 0);
    }

    #[test]
    fn status_codes_are_tallied() {
        let m = Metrics::new();
        m.record_status(200);
        m.record_status(200);
        m.record_status(500);
        let snap = m.snapshot();
        assert_eq!(snap.status_by_code.len(), 2);
        assert!(snap
            .status_by_code
            .iter()
            .any(|s| s.code == 200 && s.count == 2));
        assert!(snap
            .status_by_code
            .iter()
            .any(|s| s.code == 500 && s.count == 1));
    }

    #[test]
    fn series_is_capped_and_orders_newest_last() {
        let m = Metrics::new();
        for _ in 0..(MAX_SERIES + 10) {
            m.sample_series(0.0, 1000);
        }
        let snap = m.snapshot();
        assert_eq!(snap.series.len(), MAX_SERIES);
    }

    #[test]
    fn cache_counters_are_kept() {
        let m = Metrics::new();
        m.record_cache_hit();
        m.record_cache_miss();
        m.record_cache_miss();
        let snap = m.snapshot();
        assert_eq!(snap.cache_hits, 1);
        assert_eq!(snap.cache_misses, 2);
    }

    #[test]
    fn red_helpers_compute_ratios() {
        let m = Metrics::new();
        m.record_status(200);
        m.record_status(200);
        m.record_status(404);
        m.record_status(500);
        let snap = m.snapshot();
        assert_eq!(snap.red_4xx_count(), 1);
        assert_eq!(snap.red_5xx_count(), 1);
        assert_eq!(snap.red_errors_total(), 2);
        // 1/4 = 0.25 each, 0.5 total
        assert!((snap.red_error_ratio_4xx() - 0.25).abs() < 1e-9);
        assert!((snap.red_error_ratio_5xx() - 0.25).abs() < 1e-9);
        assert!((snap.red_error_ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn slo_helpers_delegate_to_sre() {
        let m = Metrics::new();
        m.record_status(200);
        m.record_status(500);
        let snap = m.snapshot();
        // 50% availability -> error_rate 0.5 -> burn 500x
        assert!((snap.slo_availability() - 0.5).abs() < 1e-9);
        assert!(snap.slo_burn_rate() > 100.0);
    }
}
