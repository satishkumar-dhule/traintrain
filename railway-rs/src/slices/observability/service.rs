use std::collections::BTreeSet;

use crate::core::metrics::SeriesPoint;
use crate::core::obs::{log_ring, proc_stats};
use crate::core::sre;
use crate::models::{
    CacheStats, CapacitySnapshot, ObservabilityResponse, OriginStatus, SeriesData, SourceSeries,
    StatusCode,
};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Real runtime + per-source observability snapshot.
    pub fn snapshot(state: &AppState) -> ObservabilityResponse {
        let metrics = state.metrics.snapshot();

        let (cpu_usage, mem_usage) = proc_stats();

        // Latency of the serving process: rolling average across sources that
        // have been sampled. Falls back to 0.0 only when nothing was sampled yet.
        let avg = {
            let samples = &metrics.source_latency;
            let total: f64 = samples.iter().map(|s| s.avg_latency_ms).sum();
            if samples.is_empty() {
                0.0
            } else {
                total / samples.len() as f64
            }
        };

        let origin = |name: &str, source: &str| {
            let sample = metrics.source_latency.iter().find(|s| s.source == source);
            // Flip-flop: if the circuit is open the origin is degraded, not live.
            let degraded = !state.failover.is_available(source);
            OriginStatus {
                name: name.into(),
                latency: sample.map(|s| s.avg_latency_ms as u64).unwrap_or(0),
                status: if degraded {
                    "degraded (circuit open)".into()
                } else {
                    "live".into()
                },
                requests: sample.map(|s| s.samples).unwrap_or(0),
            }
        };

        let origins = vec![
            origin("Railyatri", "railyatri"),
            origin("etrain", "etrain"),
            origin("NTES", "ntes"),
            origin("IRCTC", "irctc"),
            origin("Paytm", "paytm"),
        ];
        // AskDISHA sources are listed only while the module is enabled; they
        // report real recorded latencies once askdisha traffic has flowed.
        let mut origins = origins;
        if state.askdisha.is_some() {
            origins.push(origin("AskDISHA API", crate::core::corover::SOURCE_API));
            origins.push(origin("AskDISHA CDN", crate::core::corover::SOURCE_CDN));
        }

        let top_paths = metrics
            .requests_by_path
            .iter()
            .take(10)
            .map(|p| (p.path.clone(), p.count))
            .collect();

        let status_codes = metrics
            .status_by_code
            .iter()
            .map(|s| StatusCode {
                code: s.code,
                count: s.count,
            })
            .collect();

        let hits = metrics.cache_hits;
        let misses = metrics.cache_misses;
        let lookups = hits.saturating_add(misses);
        let hit_rate = if lookups > 0 {
            (hits as f64 / lookups as f64) * 100.0
        } else {
            0.0
        };

        let logs = log_ring().snapshot(40, None);

        let raw_failover = state.failover.snapshot();
        state.telemetry.set_failover_snapshot(&raw_failover);
        let failover = raw_failover
            .into_iter()
            .map(|s| crate::models::FailoverStatus {
                source: s.source,
                state: format!("{:?}", s.state).to_ascii_lowercase(),
                consecutive_failures: s.consecutive_failures,
                available: s.available,
                open_secs: s.open_secs,
            })
            .collect();

        // ── SRE: SLO/SLI/Error Budget, RED/USE, Four Golden Signals, Capacity ──
        // Pattern: SLO — availability SLI, error budget, burn rate
        // Pattern: RED — Rate, Errors, Duration
        // Pattern: USE — Utilization, Saturation, Errors
        // Pattern: Four Golden Signals — Latency, Traffic, Errors, Saturation
        // Pattern: Capacity Planning — saturation vs thresholds
        let mem_mb = mem_usage as f64 / (1024.0 * 1024.0);
        let slo_snapshot =
            sre::SloSnapshot::from_metrics_with_telemetry(&metrics, cpu_usage, mem_mb);
        // Update telemetry SLO gauges so /metrics scrape stays consistent
        state.telemetry.sample(
            &metrics,
            cpu_usage,
            mem_usage,
            state.uptime_secs(),
            state.cache.len(),
        );
        let red = sre::RedSignals::from_snapshot(&metrics);
        let use_signals = sre::UseSignals::from_snapshot(&metrics, cpu_usage, mem_mb);
        let golden = sre::FourGoldenSignals::from_snapshot(&metrics, cpu_usage);
        // Capacity Planning decision — uses same thresholds as /rail-api/capacity
        let cpu_thr = sre::SATURATION_CPU_THRESHOLD;
        let mem_thr = sre::SATURATION_MEMORY_THRESHOLD_MB;
        let inflight_thr = sre::SATURATION_IN_FLIGHT_THRESHOLD as f64;
        let rps_thr = sre::SATURATION_RPS_THRESHOLD;
        let cpu_sat = cpu_usage > cpu_thr;
        let mem_sat = mem_mb > mem_thr;
        let inflight_sat = (metrics.in_flight as f64) > inflight_thr;
        let rps_sat = metrics.req_per_sec > rps_thr;
        let saturated_count = [cpu_sat, mem_sat, inflight_sat, rps_sat]
            .iter()
            .filter(|&&x| x)
            .count();
        let recommendation = if saturated_count >= 2 || cpu_sat || inflight_sat || rps_sat {
            "scale_up".to_string()
        } else if saturated_count == 0
            && cpu_usage < cpu_thr * 0.5
            && mem_mb < mem_thr * 0.5
            && (metrics.in_flight as f64) < inflight_thr * 0.3
            && metrics.req_per_sec < rps_thr * 0.3
        {
            "scale_down".to_string()
        } else {
            "ok".to_string()
        };
        let capacity = CapacitySnapshot {
            recommendation: recommendation.clone(),
            saturated_count,
            saturation_ok: saturated_count == 0,
            cpu: cpu_usage,
            cpu_threshold: cpu_thr,
            cpu_saturated: cpu_sat,
            memory_mb: mem_mb,
            memory_threshold_mb: mem_thr,
            memory_saturated: mem_sat,
            in_flight: metrics.in_flight,
            in_flight_threshold: inflight_thr as u64,
            in_flight_saturated: inflight_sat,
            rps: metrics.req_per_sec,
            rps_threshold: rps_thr,
            rps_saturated: rps_sat,
            fine_print: sre::FIN_PRINT_CAPACITY_PLANNING.to_string(),
        };
        // Fine-print: one string per pattern, plus full table for UI
        let fine_print: Vec<String> = sre::FINE_PRINT_ALL
            .iter()
            .map(|(_, v)| v.to_string())
            .collect();
        let patterns: Vec<String> = sre::FINE_PRINT_ALL
            .iter()
            .map(|(k, _)| k.to_string())
            .collect();
        let sre_patterns: Vec<(String, String)> = sre::FINE_PRINT_ALL
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        ObservabilityResponse {
            active_connections: metrics.in_flight,
            latency_ms: avg as u64,
            req_per_sec: metrics.req_per_sec as u64,
            cpu_usage,
            mem_usage,
            origins,
            uptime_secs: state.uptime_secs(),
            requests_total: metrics.requests_total,
            bytes_out: metrics.bytes_out,
            top_paths,
            status_codes,
            cache: CacheStats {
                hits,
                misses,
                hit_rate: (hit_rate * 100.0).round() / 100.0,
                entries: state.cache.len(),
            },
            series: to_series_data(&metrics.series),
            logs,
            failover,
            slo: Some(slo_snapshot),
            red: Some(red),
            use_signals: Some(use_signals),
            golden: Some(golden),
            capacity: Some(capacity),
            fine_print,
            patterns,
            sre_patterns,
        }
    }
}

/// Transpose the row-oriented sample points into column-oriented arrays so
/// the frontend charts can consume them directly.
fn to_series_data(points: &[SeriesPoint]) -> SeriesData {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for p in points {
        for (name, _) in &p.sources {
            names.insert(name.clone());
        }
    }

    let sources = names
        .into_iter()
        .map(|name| {
            let latency_ms = points
                .iter()
                .map(|p| {
                    p.sources
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, v)| *v)
                        .unwrap_or(0.0)
                })
                .collect();
            SourceSeries { name, latency_ms }
        })
        .collect();

    SeriesData {
        times: points.iter().map(|p| p.t).collect(),
        rps: points.iter().map(|p| p.rps).collect(),
        latency_ms: points.iter().map(|p| p.latency_ms).collect(),
        mem_mb: points.iter().map(|p| p.mem_mb).collect(),
        cpu_frac: points.iter().map(|p| p.cpu_frac).collect(),
        in_flight: points.iter().map(|p| p.in_flight).collect(),
        sources,
    }
}
