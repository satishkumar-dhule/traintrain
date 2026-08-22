use std::collections::BTreeSet;

use crate::core::metrics::SeriesPoint;
use crate::core::obs::{log_ring, proc_stats};
use crate::models::{
    CacheStats, ObservabilityResponse, OriginStatus, SeriesData, SourceSeries, StatusCode,
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
            OriginStatus {
                name: name.into(),
                latency: sample.map(|s| s.avg_latency_ms as u64).unwrap_or(0),
                status: "live".into(),
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
