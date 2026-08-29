//! Central SRE module — SLO / SLI / Error Budget per Google SRE Book.
//!
//! Implements the vocabulary and arithmetic from *Site Reliability Engineering*
//! (Beyer, Jones & Petoff) in one place so every slice, middleware and
//! dashboard can speak the same language.
//!
//! # Patterns covered
//! - SLO / SLI / Error Budget (Ch. 4)
//! - Four Golden Signals — Latency, Traffic, Errors, Saturation (Ch. 6)
//! - RED (Rate, Errors, Duration) — micro-service view of the Four Golden
//!   Signals, used for request-scoped health.
//! - USE (Utilization, Saturation, Errors) — resource view, used for host /
//!   infra health.
//! - Circuit Breaker, Bulkhead, Retry with Jitter, Timeout Budget, Hedging /
//!   Fan-out, Graceful Degradation, Load Shedding, Capacity Planning,
//!   Observability Pipeline (Ch. 22, 23, 27)
//!
//! All arithmetic is pure functions over [`crate::core::metrics::MetricsSnapshot`]
//! (and optionally live `Telemetry`/proc stats) so it is trivially testable
//! without a running server.

use crate::core::metrics::MetricsSnapshot;

// ---------------------------------------------------------------------------
// SLO targets — the service's contract with its users.
// ---------------------------------------------------------------------------

/// Availability SLO target: 99.9 % of requests succeed (2xx/3xx) over the
/// rolling window.
pub const SLO_AVAILABILITY_TARGET: f64 = 0.999;

/// Human-readable SLO availability percent (99.9).
pub const SLO_AVAILABILITY_PERCENT: f64 = 99.9;

/// Error budget = 1 − SLO = 0.1 % (0.001). When this is exhausted, releases
/// freeze and only reliability work ships.
pub const SLO_ERROR_BUDGET: f64 = 0.001;

/// Error budget as a percent (0.1).
pub const SLO_ERROR_BUDGET_PERCENT: f64 = 0.1;

/// Rolling window over which the SLO is evaluated (Google SRE default).
pub const SLO_WINDOW_DAYS: u32 = 28;

/// Latency SLO — p95 must be below this (ms).
pub const SLO_LATENCY_P95_TARGET_MS: f64 = 800.0;

/// Latency SLO — p99 must be below this (ms).
pub const SLO_LATENCY_P99_TARGET_MS: f64 = 2000.0;

// Compatibility aliases — some call-sites prefer the `_MS` suffix without
// `TARGET`, expose both.
pub const SLO_LATENCY_P95_MS: f64 = SLO_LATENCY_P95_TARGET_MS;
pub const SLO_LATENCY_P99_MS: f64 = SLO_LATENCY_P99_TARGET_MS;

// ---------------------------------------------------------------------------
// Saturation thresholds — when exceeded the service is considered saturated.
// ---------------------------------------------------------------------------

/// CPU saturation: fraction of one core (0.0–1.0). Above this the host is
/// considered saturated.
pub const SATURATION_CPU_THRESHOLD: f64 = 0.80;

/// Memory saturation expressed as a *fraction* placeholder; callers compare
/// against `mem_mb` budgets externally. Kept at 0.80 for symmetry with CPU.
pub const SATURATION_MEMORY_THRESHOLD_FRACTION: f64 = 0.80;

/// Memory saturation in MiB — above this the process is considered under
/// memory pressure (2 GiB default; tune per deployment).
pub const SATURATION_MEMORY_THRESHOLD_MB: f64 = 2048.0;

/// In-flight request saturation — above this the service should shed load.
pub const SATURATION_IN_FLIGHT_THRESHOLD: u64 = 1000;

/// Requests-per-second saturation for capacity planning (requests/sec).
pub const SATURATION_RPS_THRESHOLD: f64 = 500.0;

// ---------------------------------------------------------------------------
// Fine-print strings — one per SRE pattern, suitable for footers / tooltips.
// Each string begins with "SRE Pattern:" so UIs can render them uniformly.
// The task requests `FIN_PRINT_*` names; we also expose `FINE_PRINT_*` aliases
// with the correct spelling for convenience.
// ---------------------------------------------------------------------------

pub const FIN_PRINT_SLO: &str = "SRE Pattern: Service Level Objective (SLO) — 99.9% availability over 28d rolling window; error budget 0.1% — burn-rate alerting [Google SRE Ch.4]";
pub const FINE_PRINT_SLO: &str = FIN_PRINT_SLO;

pub const FIN_PRINT_SLI: &str = "SRE Pattern: Service Level Indicator (SLI) — availability SLI = (2xx+3xx)/total, latency SLI from histogram p95/p99 [Google SRE Ch.4]";
pub const FINE_PRINT_SLI: &str = FIN_PRINT_SLI;

pub const FIN_PRINT_ERROR_BUDGET: &str = "SRE Pattern: Error Budget — 0.1% (1 - 99.9% SLO); burn rate = error_rate / budget; remaining = 1 - consumed; freeze releases when exhausted [Google SRE Ch.4]";
pub const FINE_PRINT_ERROR_BUDGET: &str = FIN_PRINT_ERROR_BUDGET;

pub const FIN_PRINT_FOUR_GOLDEN_SIGNALS: &str = "SRE Pattern: Four Golden Signals — Latency, Traffic, Errors, Saturation — the minimum to page on [Google SRE Ch.6]";
pub const FINE_PRINT_FOUR_GOLDEN_SIGNALS: &str = FIN_PRINT_FOUR_GOLDEN_SIGNALS;

pub const FIN_PRINT_RED: &str = "SRE Pattern: RED — Rate, Errors, Duration — request-scoped health for microservices (Tom Wilkie) [Google SRE Ch.6]";
pub const FINE_PRINT_RED: &str = FIN_PRINT_RED;

pub const FIN_PRINT_USE: &str = "SRE Pattern: USE — Utilization, Saturation, Errors — resource-scoped health for hosts/queues (Brendan Gregg) [Google SRE Ch.6]";
pub const FINE_PRINT_USE: &str = FIN_PRINT_USE;

pub const FIN_PRINT_CIRCUIT_BREAKER: &str = "SRE Pattern: Circuit Breaker — fail-fast when downstream error rate exceeds threshold; probe half-open after cooldown [Nygard Release It! / Google SRE Ch.22]";
pub const FINE_PRINT_CIRCUIT_BREAKER: &str = FIN_PRINT_CIRCUIT_BREAKER;

pub const FIN_PRINT_BULKHEAD: &str = "SRE Pattern: Bulkhead — isolate failure domains (thread pools, connection pools) so one slow source cannot sink the ship [Nygard / Google SRE Ch.22]";
pub const FINE_PRINT_BULKHEAD: &str = FIN_PRINT_BULKHEAD;

pub const FIN_PRINT_RETRY_WITH_JITTER: &str = "SRE Pattern: Retry with Jitter — exponential backoff + jitter prevents thundering herd; idempotent GETs only, capped at 2 attempts [Google SRE Ch.22]";
pub const FINE_PRINT_RETRY_WITH_JITTER: &str = FIN_PRINT_RETRY_WITH_JITTER;

pub const FIN_PRINT_TIMEOUT_BUDGET: &str = "SRE Pattern: Timeout Budget — per-request deadline propagation; connect 8s, request budgets fan out to upstreams so user never waits forever [Google SRE Ch.22]";
pub const FINE_PRINT_TIMEOUT_BUDGET: &str = FIN_PRINT_TIMEOUT_BUDGET;

pub const FIN_PRINT_HEDGING: &str = "SRE Pattern: Hedging / Fan-out N×2 — race N upstreams, cancel losers; p95 fan-out reduces tail latency without multiplying load [Google SRE Ch.22]";
pub const FINE_PRINT_HEDGING: &str = FIN_PRINT_HEDGING;
pub const FIN_PRINT_HEDGING_FANOUT: &str = FIN_PRINT_HEDGING;
pub const FINE_PRINT_HEDGING_FANOUT: &str = FIN_PRINT_HEDGING;
pub const FIN_PRINT_FANOUT_NX2: &str = FIN_PRINT_HEDGING;
pub const FINE_PRINT_FANOUT_NX2: &str = FIN_PRINT_HEDGING;
pub const FIN_PRINT_HEDGING_FAN_OUT_N_X2: &str = FIN_PRINT_HEDGING;

pub const FIN_PRINT_GRACEFUL_DEGRADATION: &str = "SRE Pattern: Graceful Degradation — serve stale cache or partial results when primaries fail; never fail a read that a fallback can answer [Google SRE Ch.22]";
pub const FINE_PRINT_GRACEFUL_DEGRADATION: &str = FIN_PRINT_GRACEFUL_DEGRADATION;

pub const FIN_PRINT_LOAD_SHEDDING: &str = "SRE Pattern: Load Shedding — when saturation thresholds breach, shed low-priority work (404 fast-path, 429/503) to preserve SLO [Google SRE Ch.23]";
pub const FINE_PRINT_LOAD_SHEDDING: &str = FIN_PRINT_LOAD_SHEDDING;

pub const FIN_PRINT_CAPACITY_PLANNING: &str = "SRE Pattern: Capacity Planning — model rps × latency × in_flight headroom; autoscale at 80% saturation, load-test before launch [Google SRE Ch.27]";
pub const FINE_PRINT_CAPACITY_PLANNING: &str = FIN_PRINT_CAPACITY_PLANNING;

pub const FIN_PRINT_OBSERVABILITY_PIPELINE: &str = "SRE Pattern: Observability Pipeline — Metrics (Prometheus) + Logs (ring) + Traces → Telemetry.sample() → /metrics & /rail-api/observability [Google SRE Ch.10]";
pub const FINE_PRINT_OBSERVABILITY_PIPELINE: &str = FIN_PRINT_OBSERVABILITY_PIPELINE;

/// All fine-print strings in a single table for UIs that want to render them
/// dynamically.
pub const FINE_PRINT_ALL: &[(&str, &str)] = &[
    ("SLO", FINE_PRINT_SLO),
    ("SLI", FINE_PRINT_SLI),
    ("Error Budget", FINE_PRINT_ERROR_BUDGET),
    ("Four Golden Signals", FINE_PRINT_FOUR_GOLDEN_SIGNALS),
    ("RED", FINE_PRINT_RED),
    ("USE", FINE_PRINT_USE),
    ("Circuit Breaker", FINE_PRINT_CIRCUIT_BREAKER),
    ("Bulkhead", FINE_PRINT_BULKHEAD),
    ("Retry with Jitter", FINE_PRINT_RETRY_WITH_JITTER),
    ("Timeout Budget", FINE_PRINT_TIMEOUT_BUDGET),
    ("Hedging/Fan-out N×2", FINE_PRINT_HEDGING),
    ("Graceful Degradation", FINE_PRINT_GRACEFUL_DEGRADATION),
    ("Load Shedding", FINE_PRINT_LOAD_SHEDDING),
    ("Capacity Planning", FINE_PRINT_CAPACITY_PLANNING),
    ("Observability Pipeline", FINE_PRINT_OBSERVABILITY_PIPELINE),
];

// ---------------------------------------------------------------------------
// SLI helpers — pure functions over MetricsSnapshot.
// ---------------------------------------------------------------------------

/// Availability SLI = (2xx + 3xx) / total.
///
/// Returns 1.0 when no requests have been observed (no evidence of failure).
pub fn availability_sli(snapshot: &MetricsSnapshot) -> f64 {
    let total: u64 = snapshot.status_by_code.iter().map(|s| s.count).sum();
    if total == 0 {
        return 1.0;
    }
    let good: u64 = snapshot
        .status_by_code
        .iter()
        .filter(|s| (200..400).contains(&s.code))
        .map(|s| s.count)
        .sum();
    good as f64 / total as f64
}

/// Count of successful (2xx/3xx) requests in the snapshot.
pub fn availability_good_count(snapshot: &MetricsSnapshot) -> u64 {
    snapshot
        .status_by_code
        .iter()
        .filter(|s| (200..400).contains(&s.code))
        .map(|s| s.count)
        .sum()
}

/// Count of failed (4xx/5xx) requests in the snapshot.
pub fn availability_bad_count(snapshot: &MetricsSnapshot) -> u64 {
    let total: u64 = snapshot.status_by_code.iter().map(|s| s.count).sum();
    total.saturating_sub(availability_good_count(snapshot))
}

/// Error rate = 1 - availability_sli (0.0 .. 1.0).
pub fn error_rate(snapshot: &MetricsSnapshot) -> f64 {
    1.0 - availability_sli(snapshot)
}

/// Error-budget consumed fraction = error_rate / SLO_ERROR_BUDGET.
///
/// - 0.0 means no errors.
/// - 1.0 means the budget is exactly exhausted (error_rate == budget).
/// - >1.0 means the budget is blown (burned through).
pub fn error_budget_consumed(snapshot: &MetricsSnapshot) -> f64 {
    if SLO_ERROR_BUDGET == 0.0 {
        return 0.0;
    }
    error_rate(snapshot) / SLO_ERROR_BUDGET
}

/// Error-budget remaining fraction = 1 - consumed, clamped to 0.0..1.0.
///
/// Returns 1.0 when nothing has been consumed and 0.0 when exhausted.
pub fn error_budget_remaining(snapshot: &MetricsSnapshot) -> f64 {
    (1.0 - error_budget_consumed(snapshot)).clamp(0.0, 1.0)
}

/// Burn rate = error_rate / SLO_ERROR_BUDGET.
///
/// Identical to `error_budget_consumed` instantaneously; conceptually the
/// *speed* at which the budget is burning. A burn rate of 1.0 consumes the
/// 28-day budget in exactly 28 days. 10.0 consumes it in 2.8 days.
/// Arithmetically: burn_rate = (1 - SLI) / (1 - SLO_target).
pub fn burn_rate(snapshot: &MetricsSnapshot) -> f64 {
    error_budget_consumed(snapshot)
}

/// Latency SLI check — returns true when both p95 and p99 are within SLO.
///
/// When the snapshot carries no real latency (EMA = 0.0) the caller can pass
/// p95/p99 explicitly (e.g. from the Prometheus histogram). The overloads
/// default to `snapshot.latency_ms` for both percentiles when histograms are
/// unavailable.
pub fn latency_sli_ok(p95_ms: f64, p99_ms: f64) -> bool {
    p95_ms <= SLO_LATENCY_P95_TARGET_MS && p99_ms <= SLO_LATENCY_P99_TARGET_MS
}

/// Latency SLI from a single EMA value — conservative: treat EMA as both p95
/// and p99. Useful when only `MetricsSnapshot.latency_ms` is available.
pub fn latency_sli_ok_from_ema(ema_ms: f64) -> bool {
    latency_sli_ok(ema_ms, ema_ms)
}

/// Saturation check — true when all saturation signals are below their SLO
/// thresholds.
pub fn saturation_ok(cpu_frac: f64, mem_mb: f64, in_flight: u64, rps: f64) -> bool {
    cpu_frac <= SATURATION_CPU_THRESHOLD
        && mem_mb <= SATURATION_MEMORY_THRESHOLD_MB
        && in_flight <= SATURATION_IN_FLIGHT_THRESHOLD
        && rps <= SATURATION_RPS_THRESHOLD
}

// ---------------------------------------------------------------------------
// RED / USE / Four Golden Signals — derived views.
// ---------------------------------------------------------------------------

/// RED signals (Rate, Errors, Duration) derived from a metrics snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RedSignals {
    /// Request rate (requests/sec, `req_per_sec`).
    pub rate: f64,
    /// Error rate (0.0..1.0).
    pub errors: f64,
    /// Duration (EMA latency ms).
    pub duration_ms: f64,
}

impl RedSignals {
    pub fn from_snapshot(snapshot: &MetricsSnapshot) -> Self {
        Self {
            rate: snapshot.req_per_sec,
            errors: error_rate(snapshot),
            duration_ms: snapshot.latency_ms,
        }
    }
}

/// USE signals (Utilization, Saturation, Errors) — infra / resource health.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UseSignals {
    /// CPU utilization fraction (0.0..1.0).
    pub utilization_cpu: f64,
    /// Memory utilization in MiB.
    pub utilization_mem_mb: f64,
    /// Saturation: in-flight requests.
    pub saturation_in_flight: u64,
    /// Error rate (0.0..1.0).
    pub errors: f64,
}

impl UseSignals {
    pub fn from_snapshot(snapshot: &MetricsSnapshot, cpu_frac: f64, mem_mb: f64) -> Self {
        Self {
            utilization_cpu: cpu_frac,
            utilization_mem_mb: mem_mb,
            saturation_in_flight: snapshot.in_flight,
            errors: error_rate(snapshot),
        }
    }

    pub fn saturated(&self) -> bool {
        self.utilization_cpu > SATURATION_CPU_THRESHOLD
            || self.utilization_mem_mb > SATURATION_MEMORY_THRESHOLD_MB
            || self.saturation_in_flight > SATURATION_IN_FLIGHT_THRESHOLD
    }
}

/// Four Golden Signals snapshot (Google SRE Ch. 6).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FourGoldenSignals {
    /// Latency (ms, EMA).
    pub latency_ms: f64,
    /// Traffic (requests/sec).
    pub traffic_rps: f64,
    /// Errors (rate 0.0..1.0).
    pub errors: f64,
    /// Saturation (in-flight + CPU fraction).
    pub saturation_in_flight: u64,
    pub saturation_cpu: f64,
}

impl FourGoldenSignals {
    pub fn from_snapshot(snapshot: &MetricsSnapshot, cpu_frac: f64) -> Self {
        Self {
            latency_ms: snapshot.latency_ms,
            traffic_rps: snapshot.req_per_sec,
            errors: error_rate(snapshot),
            saturation_in_flight: snapshot.in_flight,
            saturation_cpu: cpu_frac,
        }
    }
}

// ---------------------------------------------------------------------------
// SloSnapshot — the one-stop roll-up consumed by dashboards & alerts.
// ---------------------------------------------------------------------------

/// Roll-up of all SLO/SLI/budget/saturation signals for a given
/// [`MetricsSnapshot`] (+ optional live proc stats). Serialize-friendly so it
/// can be returned directly from observability endpoints.
///
/// Build it with [`SloSnapshot::from_metrics`] or
/// [`SloSnapshot::from_metrics_with_telemetry`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct SloSnapshot {
    /// Availability SLI (0.0..1.0).
    pub availability_sli: f64,
    /// Availability SLI target (0.999).
    pub availability_target: f64,
    /// Total error budget for the window (0.001).
    pub error_budget: f64,
    /// Consumed fraction of the error budget (0.0..inf).
    pub error_budget_consumed: f64,
    /// Remaining fraction (0.0..1.0, clamped).
    pub error_budget_remaining: f64,
    /// Burn rate (same as consumed instantaneously; >1.0 = burning fast).
    pub burn_rate: f64,
    /// Total requests observed.
    pub requests_total: u64,
    /// Good (2xx/3xx) requests.
    pub requests_good: u64,
    /// Bad (4xx/5xx) requests.
    pub requests_bad: u64,
    /// Latency p95 (ms) — approximated from EMA when histogram unavailable.
    pub latency_p95_ms: f64,
    /// Latency p99 (ms) — approximated from EMA when histogram unavailable.
    pub latency_p99_ms: f64,
    pub latency_p95_target_ms: f64,
    pub latency_p99_target_ms: f64,
    pub latency_sli_ok: bool,
    /// Live saturation inputs.
    pub saturation_cpu: f64,
    pub saturation_mem_mb: f64,
    pub saturation_in_flight: u64,
    pub saturation_rps: f64,
    pub saturation_cpu_ok: bool,
    pub saturation_mem_ok: bool,
    pub saturation_in_flight_ok: bool,
    pub saturation_rps_ok: bool,
    pub saturation_ok: bool,
    /// Overall SLO health — true only when availability, latency and
    /// saturation are all within targets.
    pub slo_ok: bool,
    /// Rolling window days.
    pub window_days: u32,
    /// RED / USE / Four Golden Signals for convenience.
    pub red: RedSignals,
    pub use_signals: UseSignals,
    pub golden: FourGoldenSignals,
}

impl SloSnapshot {
    /// Compute from a metrics snapshot, using `cpu`/`mem` from `proc_stats`
    /// when provided. Pass `cpu_frac = 0.0, mem_mb = 0.0` when live stats are
    /// unavailable (saturation check will be permissive).
    pub fn from_metrics(snapshot: &MetricsSnapshot) -> Self {
        Self::from_metrics_with_telemetry(snapshot, 0.0, 0.0)
    }

    /// Compute from snapshot + live telemetry (cpu fraction + RSS in MiB).
    pub fn from_metrics_with_telemetry(
        snapshot: &MetricsSnapshot,
        cpu_frac: f64,
        mem_mb: f64,
    ) -> Self {
        let sli = availability_sli(snapshot);
        let total: u64 = snapshot.status_by_code.iter().map(|s| s.count).sum();
        let good = availability_good_count(snapshot);
        let bad = total.saturating_sub(good);
        let consumed = error_budget_consumed(snapshot);
        let remaining = error_budget_remaining(snapshot);
        let burn = burn_rate(snapshot);

        // Latency: snapshot.latency_ms is an EMA; use it for both p95 and p99
        // when a real histogram is not available. Callers with histogram data
        // should call `with_latency_histogram` afterwards.
        let p95 = snapshot.latency_ms;
        let p99 = snapshot.latency_ms;
        let latency_ok = latency_sli_ok(p95, p99);

        let cpu_ok = cpu_frac <= SATURATION_CPU_THRESHOLD;
        let mem_ok = mem_mb <= SATURATION_MEMORY_THRESHOLD_MB;
        let in_flight_ok = snapshot.in_flight <= SATURATION_IN_FLIGHT_THRESHOLD;
        let rps_ok = snapshot.req_per_sec <= SATURATION_RPS_THRESHOLD;
        let sat_ok = cpu_ok && mem_ok && in_flight_ok && rps_ok;

        let availability_ok = sli >= SLO_AVAILABILITY_TARGET;
        let slo_ok = availability_ok && latency_ok && sat_ok;

        let red = RedSignals::from_snapshot(snapshot);
        let use_signals = UseSignals::from_snapshot(snapshot, cpu_frac, mem_mb);
        let golden = FourGoldenSignals::from_snapshot(snapshot, cpu_frac);

        Self {
            availability_sli: sli,
            availability_target: SLO_AVAILABILITY_TARGET,
            error_budget: SLO_ERROR_BUDGET,
            error_budget_consumed: consumed,
            error_budget_remaining: remaining,
            burn_rate: burn,
            requests_total: total,
            requests_good: good,
            requests_bad: bad,
            latency_p95_ms: p95,
            latency_p99_ms: p99,
            latency_p95_target_ms: SLO_LATENCY_P95_TARGET_MS,
            latency_p99_target_ms: SLO_LATENCY_P99_TARGET_MS,
            latency_sli_ok: latency_ok,
            saturation_cpu: cpu_frac,
            saturation_mem_mb: mem_mb,
            saturation_in_flight: snapshot.in_flight,
            saturation_rps: snapshot.req_per_sec,
            saturation_cpu_ok: cpu_ok,
            saturation_mem_ok: mem_ok,
            saturation_in_flight_ok: in_flight_ok,
            saturation_rps_ok: rps_ok,
            saturation_ok: sat_ok,
            slo_ok,
            window_days: SLO_WINDOW_DAYS,
            red,
            use_signals,
            golden,
        }
    }

    /// Override latency percentiles from a real histogram (e.g. Prometheus).
    /// Recomputes `latency_sli_ok` and `slo_ok` accordingly.
    pub fn with_latency_histogram(mut self, p95_ms: f64, p99_ms: f64) -> Self {
        self.latency_p95_ms = p95_ms;
        self.latency_p99_ms = p99_ms;
        self.latency_sli_ok = latency_sli_ok(p95_ms, p99_ms);
        // recompute slo_ok: availability + latency + saturation
        let availability_ok = self.availability_sli >= SLO_AVAILABILITY_TARGET;
        self.slo_ok = availability_ok && self.latency_sli_ok && self.saturation_ok;
        self
    }

    /// Build from a live `Telemetry`-backed snapshot plus proc stats, mirroring
    /// what the observability service does. The `cpu_frac`/`mem_mb` arguments
    /// are the freshest `proc_stats()` sample.
    ///
    /// This is a convenience alias for `from_metrics_with_telemetry`; the
    /// `Telemetry` argument is not read today (no histogram export yet) but is
    /// accepted so callers can thread it through without refactors.
    pub fn from_telemetry(snapshot: &MetricsSnapshot, cpu_frac: f64, mem_bytes: u64) -> Self {
        Self::from_metrics_with_telemetry(snapshot, cpu_frac, mem_bytes as f64 / (1024.0 * 1024.0))
    }

    /// Whether the error budget is exhausted (remaining == 0).
    pub fn error_budget_exhausted(&self) -> bool {
        self.error_budget_remaining == 0.0
    }

    /// Whether the service is burning budget faster than it should (>2×).
    /// A common alert threshold: page when burn_rate > 2 for >5m.
    pub fn is_burning_fast(&self) -> bool {
        self.burn_rate > 2.0
    }

    /// Whether the service is in a critical burn (>10×) — budget gone in ~2.8 days.
    pub fn is_critical_burn(&self) -> bool {
        self.burn_rate > 10.0
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::metrics::Metrics;

    fn snapshot_with_status(codes: &[(u16, u64)]) -> MetricsSnapshot {
        let m = Metrics::new();
        for (code, count) in codes {
            for _ in 0..*count {
                m.record_status(*code);
            }
        }
        // tick at least one request so req_per_sec is defined
        let _ = m.begin_request();
        m.snapshot()
    }

    #[test]
    fn slo_constants_are_sane() {
        assert!((SLO_AVAILABILITY_TARGET - 0.999).abs() < 1e-9);
        assert!((SLO_ERROR_BUDGET - 0.001).abs() < 1e-9);
        assert_eq!(SLO_WINDOW_DAYS, 28);
        assert_eq!(SLO_LATENCY_P95_TARGET_MS, 800.0);
        assert_eq!(SLO_LATENCY_P99_TARGET_MS, 2000.0);
        assert!(SLO_AVAILABILITY_TARGET + SLO_ERROR_BUDGET - 1.0 < 1e-9);
    }

    #[test]
    fn saturation_thresholds_are_sane() {
        assert!((SATURATION_CPU_THRESHOLD - 0.80).abs() < 1e-9);
        assert_eq!(SATURATION_IN_FLIGHT_THRESHOLD, 1000);
        assert!(SATURATION_RPS_THRESHOLD > 0.0);
        assert!(SATURATION_MEMORY_THRESHOLD_MB > 0.0);
    }

    #[test]
    fn fin_print_constants_exist_and_mention_pattern() {
        // Every required pattern must have a FIN_PRINT constant containing
        // "SRE Pattern:" so UIs can rely on a stable prefix.
        let all = [
            FIN_PRINT_SLO,
            FIN_PRINT_SLI,
            FIN_PRINT_ERROR_BUDGET,
            FIN_PRINT_FOUR_GOLDEN_SIGNALS,
            FIN_PRINT_RED,
            FIN_PRINT_USE,
            FIN_PRINT_CIRCUIT_BREAKER,
            FIN_PRINT_BULKHEAD,
            FIN_PRINT_RETRY_WITH_JITTER,
            FIN_PRINT_TIMEOUT_BUDGET,
            FIN_PRINT_HEDGING,
            FIN_PRINT_GRACEFUL_DEGRADATION,
            FIN_PRINT_LOAD_SHEDDING,
            FIN_PRINT_CAPACITY_PLANNING,
            FIN_PRINT_OBSERVABILITY_PIPELINE,
        ];
        for s in all {
            assert!(
                s.starts_with("SRE Pattern:"),
                "fine-print must start with 'SRE Pattern:': {s}"
            );
            assert!(s.len() > 20, "fine-print too short: {s}");
        }
        // Fine-print table should have 15 entries
        assert_eq!(FINE_PRINT_ALL.len(), 15);
    }

    #[test]
    fn fine_print_aliases_match() {
        assert_eq!(FIN_PRINT_SLO, FINE_PRINT_SLO);
        assert_eq!(FIN_PRINT_SLI, FINE_PRINT_SLI);
        assert_eq!(FIN_PRINT_ERROR_BUDGET, FINE_PRINT_ERROR_BUDGET);
        assert_eq!(FIN_PRINT_HEDGING, FIN_PRINT_FANOUT_NX2);
        assert_eq!(FIN_PRINT_HEDGING, FINE_PRINT_HEDGING);
    }

    #[test]
    fn availability_sli_all_success_is_one() {
        let snap = snapshot_with_status(&[(200, 100)]);
        assert!((availability_sli(&snap) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn availability_sli_mixed_counts() {
        // 80 good (2xx/3xx), 20 bad (5xx) -> 0.8
        let snap = snapshot_with_status(&[(200, 70), (302, 10), (500, 20)]);
        assert!((availability_sli(&snap) - 0.8).abs() < 1e-9);
        assert_eq!(availability_good_count(&snap), 80);
        assert_eq!(availability_bad_count(&snap), 20);
    }

    #[test]
    fn availability_sli_counts_3xx_as_good() {
        let snap = snapshot_with_status(&[(301, 10), (302, 10), (500, 10)]);
        // 20 good, 10 bad -> 0.666...
        let sli = availability_sli(&snap);
        assert!((sli - 20.0 / 30.0).abs() < 1e-9);
    }

    #[test]
    fn availability_sli_no_data_is_one() {
        let m = Metrics::new();
        let snap = m.snapshot();
        assert!((availability_sli(&snap) - 1.0).abs() < 1e-9);
        assert!((error_rate(&snap) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn error_budget_at_slo_exactly_exhausted() {
        // 999 good, 1 bad out of 1000 -> error_rate 0.001 == budget => consumed 1.0
        let snap = snapshot_with_status(&[(200, 999), (500, 1)]);
        assert!((error_rate(&snap) - 0.001).abs() < 1e-9);
        assert!((error_budget_consumed(&snap) - 1.0).abs() < 1e-9);
        assert!((error_budget_remaining(&snap) - 0.0).abs() < 1e-9);
        assert!((burn_rate(&snap) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn error_budget_burn_rate_scales() {
        // 990 good, 10 bad out of 1000 -> error_rate 0.01 => 10× budget
        let snap = snapshot_with_status(&[(200, 990), (500, 10)]);
        assert!((error_rate(&snap) - 0.01).abs() < 1e-9);
        assert!((error_budget_consumed(&snap) - 10.0).abs() < 1e-9);
        assert!((burn_rate(&snap) - 10.0).abs() < 1e-9);
        // clamps at 0
        assert_eq!(error_budget_remaining(&snap), 0.0);
    }

    #[test]
    fn error_budget_no_errors_remaining_one() {
        let snap = snapshot_with_status(&[(200, 500)]);
        assert!((error_budget_remaining(&snap) - 1.0).abs() < 1e-9);
        assert!((burn_rate(&snap) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn latency_sli_checks() {
        assert!(latency_sli_ok(100.0, 500.0));
        assert!(latency_sli_ok(800.0, 2000.0)); // at threshold is ok
        assert!(!latency_sli_ok(801.0, 1000.0)); // p95 breach
        assert!(!latency_sli_ok(100.0, 2001.0)); // p99 breach
        assert!(latency_sli_ok_from_ema(500.0));
        assert!(!latency_sli_ok_from_ema(900.0));
        assert!(!latency_sli_ok_from_ema(2500.0));
    }

    #[test]
    fn saturation_ok_happy_path() {
        assert!(saturation_ok(0.5, 512.0, 10, 50.0));
    }

    #[test]
    fn saturation_cpu_breach() {
        assert!(!saturation_ok(0.9, 512.0, 10, 50.0));
    }

    #[test]
    fn saturation_mem_breach() {
        assert!(!saturation_ok(0.5, 4096.0, 10, 50.0));
    }

    #[test]
    fn saturation_in_flight_breach() {
        assert!(!saturation_ok(0.5, 512.0, 2000, 50.0));
    }

    #[test]
    fn slo_snapshot_happy_path() {
        let snap = snapshot_with_status(&[(200, 1000)]);
        // low latency via EMA = default 0, fast enough
        let slo = SloSnapshot::from_metrics(&snap);
        assert!((slo.availability_sli - 1.0).abs() < 1e-9);
        assert!(slo.latency_sli_ok);
        assert!(slo.saturation_ok);
        assert!(slo.slo_ok);
        assert!((slo.error_budget_remaining - 1.0).abs() < 1e-9);
        assert!(!slo.error_budget_exhausted());
        assert!(!slo.is_burning_fast());
        assert_eq!(slo.window_days, 28);
        assert_eq!(slo.availability_target, SLO_AVAILABILITY_TARGET);
    }

    #[test]
    fn slo_snapshot_breach_on_errors() {
        // 950 good, 50 bad => 95% availability -> well below 99.9%
        let snap = snapshot_with_status(&[(200, 950), (500, 50)]);
        let slo = SloSnapshot::from_metrics(&snap);
        assert!((slo.availability_sli - 0.95).abs() < 1e-9);
        assert!(!slo.slo_ok);
        assert!(slo.is_critical_burn());
        assert!(slo.is_burning_fast());
        assert!(slo.error_budget_exhausted());
        assert_eq!(slo.requests_good, 950);
        assert_eq!(slo.requests_bad, 50);
        assert_eq!(slo.requests_total, 1000);
    }

    #[test]
    fn slo_snapshot_breach_on_latency() {
        let snap = snapshot_with_status(&[(200, 1000)]);
        let mut slo = SloSnapshot::from_metrics(&snap);
        // EMA defaults to 0, so initially ok; override with histogram high p95
        slo = slo.with_latency_histogram(900.0, 1500.0);
        assert!(!slo.latency_sli_ok);
        assert!(!slo.slo_ok);
        assert_eq!(slo.latency_p95_ms, 900.0);
        // fixing it restores
        let ok = slo.with_latency_histogram(700.0, 1500.0);
        assert!(ok.latency_sli_ok);
        assert!(ok.slo_ok);
    }

    #[test]
    fn slo_snapshot_breach_on_saturation() {
        let m = Metrics::new();
        for _ in 0..10 {
            m.record_status(200);
        }
        // manually saturate cpu
        let snap = m.snapshot();
        let slo = SloSnapshot::from_metrics_with_telemetry(&snap, 0.95, 512.0);
        assert!(!slo.saturation_cpu_ok);
        assert!(!slo.saturation_ok);
        assert!(!slo.slo_ok);
    }

    #[test]
    fn slo_snapshot_from_telemetry_converts_bytes() {
        let snap = snapshot_with_status(&[(200, 100)]);
        let slo = SloSnapshot::from_telemetry(&snap, 0.1, 512 * 1024 * 1024);
        assert!((slo.saturation_mem_mb - 512.0).abs() < 1e-6);
        assert!(slo.saturation_mem_ok);
    }

    #[test]
    fn red_and_use_and_golden_are_consistent() {
        let snap = snapshot_with_status(&[(200, 900), (500, 100)]);
        let slo = SloSnapshot::from_metrics_with_telemetry(&snap, 0.3, 1024.0);
        assert!((slo.red.errors - 0.1).abs() < 1e-9);
        assert!((slo.use_signals.errors - 0.1).abs() < 1e-9);
        assert!((slo.golden.errors - 0.1).abs() < 1e-9);
        assert!((slo.red.rate - slo.golden.traffic_rps).abs() < 1e-9);
    }

    #[test]
    fn burn_rate_threshold_helpers() {
        let ok_snap = snapshot_with_status(&[(200, 1000)]);
        let ok = SloSnapshot::from_metrics(&ok_snap);
        assert!(!ok.is_burning_fast());
        assert!(!ok.is_critical_burn());

        // 0.5% error = 5× budget
        let mid_snap = snapshot_with_status(&[(200, 995), (500, 5)]);
        let mid = SloSnapshot::from_metrics(&mid_snap);
        assert!(mid.is_burning_fast());
        assert!(!mid.is_critical_burn());

        // 2% error = 20×
        let hot_snap = snapshot_with_status(&[(200, 980), (500, 20)]);
        let hot = SloSnapshot::from_metrics(&hot_snap);
        assert!(hot.is_critical_burn());
    }

    #[test]
    fn slo_snapshot_serializes() {
        let snap = snapshot_with_status(&[(200, 10)]);
        let slo = SloSnapshot::from_metrics(&snap);
        let json = serde_json::to_value(&slo).expect("serializes");
        assert!(json.get("availability_sli").is_some());
        assert!(json.get("burn_rate").is_some());
        assert!(json.get("red").is_some());
    }
}
