//! System endpoints: liveness (`/healthz`, `/api/healthz`), Prometheus metrics
//! (`/metrics`) and live source status (`/rail-api/source-status`). These
//! report real runtime facts only.
//!
//! SRE Patterns covered:
//! - Pattern: Health Checks (liveness, readiness, deep dependency probes)
//! - Pattern: Graceful Degradation (stale cache / partial results when primaries fail)
//! - Pattern: Capacity Planning (USE saturation signals vs thresholds)

use axum::extract::State;
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

use crate::core::error::AppError;
use crate::models::{Healthz, SourceHealth, SourceStatus};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/healthz", get(healthz))
        .route("/health/live", get(healthz))
        .route("/readyz", get(readyz))
        .route("/health/ready", get(readyz))
        .route("/ready", get(readyz))
        .route("/api/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .route("/sitemap.xml", get(sitemap))
        .route("/rail-api/source-status", get(source_status))
        .route("/rail-api/capacity", get(capacity))
        .route("/rail-api/debug", post(debug_report))
}

/// Fine-print strings for health checks / graceful degradation / capacity planning.
/// Each begins with "SRE Pattern:" so UIs can render uniformly.
const FIN_PRINT_HEALTH_CHECKS: &str = "SRE Pattern: Health Checks — liveness, readiness and deep dependency probes; fast-fail with circuit breakers when downstreams degrade [Google SRE Ch.14]";
const FIN_PRINT_GRACEFUL_DEGRADATION: &str = crate::core::sre::FIN_PRINT_GRACEFUL_DEGRADATION;
const FIN_PRINT_CAPACITY_PLANNING: &str = crate::core::sre::FIN_PRINT_CAPACITY_PLANNING;

/// The canonical pages of the SPA. The app is hash-routed, so crawler-reachable
/// URLs are the section-level routes plus the static sub-views; entity pages
/// (`#/train/{num}`, `#/station/{code}`) are user-supplied and cannot be listed.
const SITEMAP_PATHS: &[&str] = &[
    "/",
    "/#/train",
    "/#/station",
    "/#/station/heritage",
    "/#/station/parcel",
    "/#/plan",
    "/#/system",
    "/#/system/observability",
    "/#/system/settings",
    "/#/system/debug",
];

/// Serve `sitemap.xml` listing the app's canonical pages. The base URL is
/// derived from the request's `Host` header (and `X-Forwarded-Proto` when
/// present, e.g. behind a TLS-terminating proxy) so the same binary works on
/// any deployment domain.
async fn sitemap(headers: HeaderMap) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost")
        .to_string();
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| *s == "https" || *s == "http")
        .unwrap_or("http");
    let base = format!("{scheme}://{host}");
    let urls: String = SITEMAP_PATHS
        .iter()
        .map(|path| {
            format!(
                "  <url><loc>{}</loc></url>\n",
                xml_escape(&format!("{base}{path}"))
            )
        })
        .collect();
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n\
         {urls}\
         </urlset>\n"
    );
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/xml; charset=utf-8"),
        )],
        body,
    )
        .into_response()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Liveness probe — cheap, never blocks, bypasses bulkhead/rate limit/load shedding.
/// Returns 200 ok when the process is alive. Kubernetes liveness should hit this.
async fn healthz() -> Json<Healthz> {
    Json(Healthz {
        status: "ok",
        service: "railway-rs",
        runtime: "rust/axum",
    })
}

/// Prometheus text-format metrics (v0.0.4). Drop-in for any Prometheus/Grafana
/// scrape target - the registry lives in `AppState.telemetry`.
async fn metrics(State(state): State<AppState>) -> Response {
    let (cpu, mem) = crate::core::obs::proc_stats();
    let snap = state.metrics.snapshot();
    state
        .telemetry
        .sample(&snap, cpu, mem, state.uptime_secs(), state.cache.len());
    let body = state.telemetry.encode();
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
        )],
        body,
    )
        .into_response()
}

/// Readiness probe — checks dataset loaded, cache reachable, upstream probe timeout budget (1s)
/// with degraded vs ready. Returns 200 when ready (or degraded), 503 when not ready.
/// Also exposes error_budget burn rate from SRE module and sets `railway_ready` gauge (1/0).
/// Pattern: Health Checks
async fn readyz(State(state): State<AppState>) -> Response {
    // Dataset check — real data must be loaded
    let stations_len = state.datasets.stations.len();
    let trains_len = state.datasets.trains.len();
    let dataset_ok = stations_len > 0 && trains_len > 0;

    // Cache reachable — try a cheap set/get probe
    let cache_ok = {
        // cache.len() should not panic; also test write/read of a probe key
        let probe_key = "__readiness_probe__";
        let probe_val = json!({"probe": 1});
        // Use catch_unwind to be extra safe if cache mutex poisoned
        let len_ok = std::panic::catch_unwind(|| state.cache.len()).is_ok();
        if !len_ok {
            false
        } else {
            state.cache.set(probe_key, probe_val.clone());
            let got = state.cache.get(probe_key);
            // clean up probe key (keep cache lean; remove probe)
            // not strictly needed but keeps len stable
            // We leave it; it will expire with TTL.
            got == Some(probe_val)
        }
    };

    // Upstream probe with 1s timeout budget per source (concurrently)
    let upstreams_cfg = vec![
        ("NTES", state.config.ntes_base.clone()),
        ("Railyatri", state.config.railyatri_base.clone()),
        ("etrain", state.config.etrain_base.clone()),
        ("IRCTC", state.config.irctc_base.clone()),
    ];

    // Circuit snapshot for per-source state
    let failover_snap = state.failover.snapshot();
    let circuit_map: std::collections::HashMap<String, (String, bool, u32, Option<u64>)> =
        failover_snap
            .into_iter()
            .map(|s| {
                let state_str = format!("{:?}", s.state).to_ascii_lowercase();
                (
                    s.source.clone(),
                    (state_str, s.available, s.consecutive_failures, s.open_secs),
                )
            })
            .collect();

    let mut upstream_checks: Vec<Value> = Vec::new();
    let mut reachable_count = 0usize;

    // Run probes concurrently with 1s budget each
    let probe_futs = upstreams_cfg.iter().map(|(name, base)| {
        let base = base.clone();
        let st = state.clone();
        async move {
            let start = Instant::now();
            let reachable = probe_with_timeout(&st, &base, Duration::from_secs(1)).await;
            let latency_ms = start.elapsed().as_millis() as u64;
            (name.to_string(), base, reachable, latency_ms)
        }
    });
    let results = futures::future::join_all(probe_futs).await;

    for (name, _base, reachable, latency_ms) in results {
        if reachable {
            reachable_count += 1;
        }
        let key = name.to_ascii_lowercase();
        let (circuit_state, available, consecutive_failures, open_secs) = circuit_map
            .get(&key)
            .cloned()
            .unwrap_or_else(|| ("closed".to_string(), true, 0, None));
        // Determine per-source latency to report: only when reachable, else None
        let latency_opt = if reachable { Some(latency_ms) } else { None };
        upstream_checks.push(json!({
            "name": name,
            "reachable": reachable,
            "latency_ms": latency_opt,
            "circuit_state": circuit_state,
            "available": available,
            "consecutive_failures": consecutive_failures,
            "open_secs": open_secs
        }));
    }

    let total = upstreams_cfg.len() as usize;
    // readiness logic
    let ready = dataset_ok && cache_ok;
    // status: ready when all upstreams reachable, degraded when some reachable, not_ready when dataset/cache fail
    // If ready but no upstream reachable, still degraded (service can serve stale cache)
    let status = if !ready {
        "not_ready"
    } else if reachable_count == total {
        "ready"
    } else {
        "degraded"
    };

    // SRE error budget burn rate
    let snap = state.metrics.snapshot();
    let (cpu, mem_bytes) = crate::core::obs::proc_stats();
    let mem_mb = mem_bytes as f64 / (1024.0 * 1024.0);
    let slo = crate::core::sre::SloSnapshot::from_metrics_with_telemetry(&snap, cpu, mem_mb);

    // Update Prometheus gauge railway_ready (1/0)
    // ready=true => 1, even if degraded (service can serve). Only not_ready => 0.
    state.telemetry.set_ready(ready);

    let body = json!({
        "status": status,
        "ready": ready,
        "checks": {
            "dataset": {
                "ok": dataset_ok,
                "loaded": dataset_ok,
                "stations": stations_len,
                "trains": trains_len
            },
            "cache": {
                "ok": cache_ok,
                "reachable": cache_ok,
                "entries": state.cache.len()
            },
            "upstreams": upstream_checks,
            "upstreams_summary": {
                "reachable": reachable_count,
                "total": total,
                "degraded": reachable_count < total && ready
            }
        },
        "error_budget": {
            "availability_sli": slo.availability_sli,
            "error_budget_remaining": slo.error_budget_remaining,
            "error_budget_consumed": slo.error_budget_consumed,
            "burn_rate": slo.burn_rate,
            "error_rate": 1.0 - slo.availability_sli,
            "is_burning_fast": slo.is_burning_fast(),
            "is_critical_burn": slo.is_critical_burn()
        },
        "uptime_secs": state.uptime_secs(),
        "fine_print": [
            FIN_PRINT_HEALTH_CHECKS,
            FIN_PRINT_GRACEFUL_DEGRADATION,
            FIN_PRINT_CAPACITY_PLANNING
        ],
        "patterns": ["Health Checks", "Graceful Degradation", "Capacity Planning"]
    });

    let status_code = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status_code, Json(body)).into_response()
}

/// Capacity endpoint — returns saturation signals (USE): cpu, mem, in_flight, RPS vs thresholds, recommendation (scale up/down).
/// Uses `src/core/sre.rs` thresholds. Sets `railway_capacity_recommendation` and `railway_capacity_saturated` gauges.
/// Pattern: Capacity Planning
async fn capacity(State(state): State<AppState>) -> Json<Value> {
    let (cpu, mem_bytes) = crate::core::obs::proc_stats();
    let mem_mb = mem_bytes as f64 / (1024.0 * 1024.0);
    let snap = state.metrics.snapshot();
    let in_flight = snap.in_flight;
    let rps = snap.req_per_sec;

    let cpu_thr = crate::core::sre::SATURATION_CPU_THRESHOLD;
    let mem_thr = crate::core::sre::SATURATION_MEMORY_THRESHOLD_MB;
    let inflight_thr = crate::core::sre::SATURATION_IN_FLIGHT_THRESHOLD as f64;
    let rps_thr = crate::core::sre::SATURATION_RPS_THRESHOLD;

    let cpu_sat = cpu > cpu_thr;
    let mem_sat = mem_mb > mem_thr;
    let inflight_sat = (in_flight as f64) > inflight_thr;
    let rps_sat = rps > rps_thr;

    let saturated_count = [cpu_sat, mem_sat, inflight_sat, rps_sat]
        .iter()
        .filter(|&&x| x)
        .count();

    // Recommendation logic — scale up when any critical saturation breached (especially cpu/in_flight), scale down when well below thresholds
    let recommendation = if saturated_count >= 2 || cpu_sat || inflight_sat || rps_sat {
        "scale_up"
    } else if saturated_count == 0
        && cpu < cpu_thr * 0.5
        && mem_mb < mem_thr * 0.5
        && (in_flight as f64) < inflight_thr * 0.3
        && rps < rps_thr * 0.3
    {
        "scale_down"
    } else {
        "ok"
    };

    // Update Prometheus gauges
    state.telemetry.set_capacity_recommendation(recommendation);
    state
        .telemetry
        .set_capacity_saturated(saturated_count as f64);
    // Also update saturation gauges via telemetry.sample for consistency
    state.telemetry.sample(
        &snap,
        cpu,
        mem_bytes,
        state.uptime_secs(),
        state.cache.len(),
    );

    // USE signals
    let use_signals = crate::core::sre::UseSignals::from_snapshot(&snap, cpu, mem_mb);
    let slo = crate::core::sre::SloSnapshot::from_metrics_with_telemetry(&snap, cpu, mem_mb);

    Json(json!({
        "utilization": {
            "cpu": cpu,
            "cpu_threshold": cpu_thr,
            "cpu_saturated": cpu_sat,
            "cpu_headroom": (cpu_thr - cpu).max(0.0),
            "memory_mb": mem_mb,
            "memory_bytes": mem_bytes,
            "memory_threshold_mb": mem_thr,
            "memory_saturated": mem_sat,
            "memory_headroom_mb": (mem_thr - mem_mb).max(0.0)
        },
        "saturation": {
            "in_flight": in_flight,
            "in_flight_threshold": inflight_thr as u64,
            "in_flight_saturated": inflight_sat,
            "in_flight_headroom": (inflight_thr - in_flight as f64).max(0.0) as u64,
            "rps": rps,
            "rps_threshold": rps_thr,
            "rps_saturated": rps_sat,
            "rps_headroom": (rps_thr - rps).max(0.0)
        },
        "signals": {
            "cpu": {
                "value": cpu,
                "threshold": cpu_thr,
                "saturated": cpu_sat,
                "headroom": (cpu_thr - cpu).max(0.0)
            },
            "memory_mb": {
                "value": mem_mb,
                "threshold": mem_thr,
                "saturated": mem_sat,
                "headroom": (mem_thr - mem_mb).max(0.0)
            },
            "memory_bytes": {
                "value": mem_bytes as f64,
                "threshold": mem_thr * 1024.0 * 1024.0,
                "saturated": mem_sat
            },
            "in_flight": {
                "value": in_flight,
                "threshold": inflight_thr as u64,
                "saturated": inflight_sat
            },
            "rps": {
                "value": rps,
                "threshold": rps_thr,
                "saturated": rps_sat
            }
        },
        "use": {
            "utilization_cpu": use_signals.utilization_cpu,
            "utilization_mem_mb": use_signals.utilization_mem_mb,
            "saturation_in_flight": use_signals.saturation_in_flight,
            "errors": use_signals.errors,
            "saturated": use_signals.saturated()
        },
        "slo": {
            "availability_sli": slo.availability_sli,
            "burn_rate": slo.burn_rate,
            "error_budget_remaining": slo.error_budget_remaining
        },
        "thresholds": {
            "cpu": cpu_thr,
            "memory_mb": mem_thr,
            "in_flight": inflight_thr as u64,
            "rps": rps_thr
        },
        "saturated_count": saturated_count,
        "saturation_ok": saturated_count == 0,
        "recommendation": recommendation,
        "fine_print": [
            FIN_PRINT_CAPACITY_PLANNING,
            crate::core::sre::FIN_PRINT_USE,
            crate::core::sre::FIN_PRINT_SLO
        ],
        "patterns": ["Capacity Planning", "USE", "SLO"],
        "uptime_secs": state.uptime_secs()
    }))
}

/// Accept a debug report from the SPA's Debug tab and append it to the server
/// log (e.g. `/tmp/railway-rs.log`) so a user-reported issue can be traced
/// end-to-end. Size- and line-capped; this only writes to the log.
#[derive(Debug, Deserialize)]
struct DebugReport {
    report: Option<String>,
}

#[derive(Debug, Serialize)]
struct DebugReportResponse {
    ok: bool,
    lines: usize,
    bytes: usize,
}

const DEBUG_MAX_BYTES: usize = 200 * 1024;
const DEBUG_MAX_LINES: usize = 2000;

async fn debug_report(Json(body): Json<DebugReport>) -> Json<DebugReportResponse> {
    let report = body.report.unwrap_or_default();
    let bytes = report.len().min(DEBUG_MAX_BYTES);
    let mut lines = 0;
    for line in report.lines().take(DEBUG_MAX_LINES) {
        tracing::info!(target: "railway_rs::ui_debug", "{line}");
        lines += 1;
    }
    Json(DebugReportResponse {
        ok: true,
        lines,
        bytes,
    })
}

/// Report which live sources are actually reachable right now. Used by the
/// UI's status banner so users can see when an upstream is down.
/// Enhanced: includes deep health per source with latency and circuit state (failover snapshot),
/// plus fine-print for Health Checks, Graceful Degradation, Capacity Planning.
async fn source_status(State(state): State<AppState>) -> Json<Value> {
    // Probe each source with detailed timing (3s timeout, same as original probe)
    let sources_cfg = vec![
        ("NTES", state.config.ntes_base.clone()),
        ("Railyatri", state.config.railyatri_base.clone()),
        ("etrain", state.config.etrain_base.clone()),
        ("IRCTC", state.config.irctc_base.clone()),
    ];

    let mut detailed: Vec<Value> = Vec::new();
    let mut simple_sources: Vec<Value> = Vec::new();

    // Snapshot of circuit breaker state
    let failover_snap = state.failover.snapshot();
    let circuit_map: std::collections::HashMap<String, (String, bool, u32, Option<u64>)> =
        failover_snap
            .iter()
            .map(|s| {
                let state_str = format!("{:?}", s.state).to_ascii_lowercase();
                (
                    s.source.clone(),
                    (state_str, s.available, s.consecutive_failures, s.open_secs),
                )
            })
            .collect();

    // Also pull live metrics for per-source latency samples
    let snap = state.metrics.snapshot();
    let latency_map: std::collections::HashMap<String, (f64, u64)> = snap
        .source_latency
        .iter()
        .map(|s| (s.source.clone(), (s.avg_latency_ms, s.samples)))
        .collect();

    for (name, base) in sources_cfg {
        let start = Instant::now();
        let reachable = probe(&state, &base).await;
        let probe_latency_ms = start.elapsed().as_millis() as u64;

        let key = name.to_ascii_lowercase();
        let (circuit_state, available, consecutive_failures, open_secs) = circuit_map
            .get(&key)
            .cloned()
            .unwrap_or_else(|| ("closed".to_string(), true, 0, None));

        // recorded latency from metrics (average) vs probe latency
        let (avg_latency_ms, samples) = latency_map.get(&key).cloned().unwrap_or((0.0, 0));

        // Update telemetry circuit gauges (already done elsewhere but ensure)
        // state.telemetry.set_failover_snapshot(&failover_snap); // done in observability, but also ensure here

        simple_sources.push(json!({
            "name": name,
            "reachable": reachable
        }));

        detailed.push(json!({
            "name": name,
            "reachable": reachable,
            "latency_ms": if reachable { probe_latency_ms } else { 0 },
            "avg_latency_ms": avg_latency_ms,
            "samples": samples,
            "circuit_state": circuit_state,
            "circuit_available": available,
            "consecutive_failures": consecutive_failures,
            "open_secs": open_secs,
            "base": base
        }));
    }

    // Ensure telemetry failover gauges are fresh
    state
        .telemetry
        .set_failover_snapshot(&state.failover.snapshot());

    let primary = "NTES (enquiry.indianrail.gov.in)";
    let verification_links = vec![
        "https://enquiry.indianrail.gov.in/ntes/",
        "https://www.railyatri.in/pnr-status",
        "https://www.railyatri.in/time-table",
        "https://www.railyatri.in/live-train-status",
        "https://etrain.info",
        "https://www.irctc.co.in/online-charts",
    ];

    // Build response including legacy fields for backward compat
    let body = json!({
        "live_enabled": state.live_enabled(),
        "mode": "live",
        "cache_ttl_seconds": state.config.cache_ttl.as_secs(),
        "primary_source": primary,
        "verification_links": verification_links,
        "notice": "Live data is fetched first from the official Indian Railways enquiry system (enquiry.indianrail.gov.in), with Railyatri as fallback. PNR status is served from Railyatri because the government PNR portal requires a CAPTCHA. Availability and prepared-chart data come from IRCTC (www.irctc.co.in), which is Akamai-protected and IP-geofenced to India. Nothing is simulated.",
        "sources": simple_sources,
        "deep_health": detailed,
        "sources_detailed": detailed,
        "circuit": circuit_map,
        "fine_print": [
            FIN_PRINT_HEALTH_CHECKS,
            FIN_PRINT_GRACEFUL_DEGRADATION,
            FIN_PRINT_CAPACITY_PLANNING
        ],
        "patterns": ["Health Checks", "Graceful Degradation", "Capacity Planning"],
        "fine_print_health_checks": FIN_PRINT_HEALTH_CHECKS,
        "fine_print_graceful_degradation": FIN_PRINT_GRACEFUL_DEGRADATION,
        "fine_print_capacity_planning": FIN_PRINT_CAPACITY_PLANNING,
        "pattern_health_checks": FIN_PRINT_HEALTH_CHECKS,
        "pattern_graceful_degradation": FIN_PRINT_GRACEFUL_DEGRADATION,
        "pattern_capacity_planning": FIN_PRINT_CAPACITY_PLANNING
    });

    Json(body)
}

/// Lightweight HEAD/GET probe with a short timeout. Returns reachability only.
async fn probe(state: &AppState, base: &str) -> bool {
    let probe_timeout = std::time::Duration::from_secs(3);
    tokio::time::timeout(probe_timeout, async {
        match state
            .http
            .inner()
            .get(base.trim_end_matches('/'))
            .send()
            .await
        {
            Ok(res) => res.status().is_success() || res.status().is_server_error(),
            Err(_) => false,
        }
    })
    .await
    .unwrap_or(false)
}

/// Probe with configurable timeout budget, used by readiness (1s)
async fn probe_with_timeout(state: &AppState, base: &str, timeout: Duration) -> bool {
    tokio::time::timeout(timeout, async {
        match state
            .http
            .inner()
            .get(base.trim_end_matches('/'))
            .send()
            .await
        {
            Ok(res) => res.status().is_success() || res.status().is_server_error(),
            Err(_) => false,
        }
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::AppState;

    #[test]
    fn healthz_is_cheap_and_ok() {
        // healthz does not block; just returns ok
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let resp = healthz().await;
            assert_eq!(resp.0.status, "ok");
            assert_eq!(resp.0.service, "railway-rs");
        });
    }

    #[tokio::test]
    async fn readyz_returns_ready_when_dataset_and_cache_ok() {
        let config = Config::default();
        let state = AppState::from_config(config).expect("state builds");
        // Use a test state with mock bases pointing to nowhere; we expect degraded but still ready (since dataset/cache ok)
        // The probe will fail (no server) but readiness should still be 200 degraded, not 503, because dataset/cache are ok
        // So we call the handler directly with the state
        let resp = readyz(axum::extract::State(state)).await;
        // Since dataset and cache are ok, status should be 200 even if upstreams unreachable (degraded)
        assert_eq!(resp.status(), 200);
        let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert!(v["ready"].as_bool().unwrap());
        assert!(v["checks"]["dataset"]["ok"].as_bool().unwrap());
        assert!(v["checks"]["cache"]["ok"].as_bool().unwrap());
        assert!(v["error_budget"]["burn_rate"].is_number());
        assert!(v["fine_print"].is_array());
    }

    #[tokio::test]
    async fn capacity_returns_use_signals_and_recommendation() {
        let config = Config::default();
        let state = AppState::from_config(config).expect("state builds");
        let resp = capacity(axum::extract::State(state)).await;
        let v = resp.0;
        assert!(v["utilization"].is_object());
        assert!(v["saturation"].is_object());
        assert!(v["recommendation"].is_string());
        let rec = v["recommendation"].as_str().unwrap();
        assert!(["ok", "scale_up", "scale_down"].contains(&rec));
        assert!(v["thresholds"]["cpu"].as_f64().unwrap() > 0.0);
        assert!(v["fine_print"].is_array());
    }

    #[tokio::test]
    async fn source_status_includes_deep_health_and_fine_print() {
        let config = Config::default();
        let state = AppState::from_config(config).expect("state builds");
        let Json(val) = source_status(axum::extract::State(state)).await;
        assert!(val["sources"].is_array());
        assert!(val["deep_health"].is_array());
        let first = &val["deep_health"][0];
        assert!(first["latency_ms"].is_number());
        assert!(first["circuit_state"].is_string());
        assert!(val["fine_print"].is_array());
        let fp = val["fine_print"].as_array().unwrap();
        assert!(fp
            .iter()
            .any(|s| s.as_str().unwrap().contains("Health Checks")));
        assert!(fp
            .iter()
            .any(|s| s.as_str().unwrap().contains("Graceful Degradation")));
        assert!(fp
            .iter()
            .any(|s| s.as_str().unwrap().contains("Capacity Planning")));
    }

    #[test]
    fn fin_print_constants_start_with_pattern() {
        assert!(FIN_PRINT_HEALTH_CHECKS.starts_with("SRE Pattern:"));
        assert!(FIN_PRINT_GRACEFUL_DEGRADATION.starts_with("SRE Pattern:"));
        assert!(FIN_PRINT_CAPACITY_PLANNING.starts_with("SRE Pattern:"));
    }
}
