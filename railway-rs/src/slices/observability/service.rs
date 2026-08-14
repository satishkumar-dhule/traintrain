use std::time::Duration;

use crate::core::metrics::MetricsSnapshot;
use crate::models::{ObservabilityResponse, OriginStatus};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Real runtime + per-source observability snapshot.
    pub fn snapshot(state: &AppState) -> ObservabilityResponse {
        let metrics: MetricsSnapshot = state.metrics.snapshot();

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

        let origins = vec![
            OriginStatus {
                name: "Railyatri".into(),
                latency: metrics
                    .source_latency
                    .iter()
                    .find(|s| s.source == "railyatri")
                    .map(|s| s.avg_latency_ms as u64)
                    .unwrap_or(0),
                status: "live".into(),
            },
            OriginStatus {
                name: "etrain".into(),
                latency: metrics
                    .source_latency
                    .iter()
                    .find(|s| s.source == "etrain")
                    .map(|s| s.avg_latency_ms as u64)
                    .unwrap_or(0),
                status: "live".into(),
            },
            OriginStatus {
                name: "NTES".into(),
                latency: metrics
                    .source_latency
                    .iter()
                    .find(|s| s.source == "ntes")
                    .map(|s| s.avg_latency_ms as u64)
                    .unwrap_or(0),
                status: "live".into(),
            },
        ];

        let top_paths = metrics
            .requests_by_path
            .iter()
            .take(10)
            .map(|p| (p.path.clone(), p.count))
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
            top_paths,
        }
    }
}

/// Sample CPU usage (0.0-1.0 fraction of one core) and RSS in bytes from
/// /proc/self/stat + /proc/self/statm. Returns (cpu, mem) honestly; 0 when
/// unavailable (non-Linux).
fn proc_stats() -> (f64, u64) {
    let stat = std::fs::read_to_string("/proc/self/stat").ok();
    let statm = std::fs::read_to_string("/proc/self/statm").ok();

    let mem = statm
        .and_then(|s| s.split_whitespace().nth(1).map(|v| v.to_string()))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|pages| pages * 4096)
        .unwrap_or(0);

    let cpu = stat.and_then(parse_cpu).unwrap_or(0.0);
    (cpu, mem)
}

/// Field 14 (utime) + field 15 (stime) in clock ticks; tick rate assumed 100.
/// Because Rust's default build has no dynamic tickrate query, this is a
/// best-effort fraction. 0.0 when unparseable.
fn parse_cpu(stat: String) -> Option<f64> {
    // utime is field 14 (1-indexed). After the comm field (possibly with
    // spaces in parens), split on whitespace after the closing paren.
    let end = stat.find(')')?;
    let rest = stat[end + 1..].trim_start();
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?; // 14th field overall - 3 prefix
    let stime: u64 = fields.get(12)?.parse().ok()?;
    let total_ticks = utime.saturating_add(stime);
    let uptime = Duration::from_secs(state_uptime_hack());
    let denom = uptime.as_secs_f64() * 100.0;
    if denom <= 0.0 {
        return Some(0.0);
    }
    Some((total_ticks as f64 / denom).min(1.0))
}

fn state_uptime_hack() -> u64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0) as u64
}
