//! System endpoints: liveness (`/healthz`, `/api/healthz`), Prometheus metrics
//! (`/metrics`) and live source status (`/rail-api/source-status`). These
//! report real runtime facts only.

use axum::extract::State;
use axum::http::header;
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};

use crate::core::error::AppError;
use crate::models::{Healthz, SourceHealth, SourceStatus};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/healthz", get(healthz))
        .route("/metrics", get(metrics))
        .route("/rail-api/source-status", get(source_status))
}

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

/// Report which live sources are actually reachable right now. Used by the
/// UI's status banner so users can see when an upstream is down.
async fn source_status(State(state): State<AppState>) -> Result<Json<SourceStatus>, AppError> {
    let sources = vec![
        ("NTES", probe(&state, &state.config.ntes_base).await),
        (
            "Railyatri",
            probe(&state, &state.config.railyatri_base).await,
        ),
        ("etrain", probe(&state, &state.config.etrain_base).await),
        ("IRCTC", probe(&state, &state.config.irctc_base).await),
    ];

    let primary = "NTES (enquiry.indianrail.gov.in)";
    let verification_links = vec![
        "https://enquiry.indianrail.gov.in/ntes/",
        "https://www.railyatri.in/pnr-status",
        "https://www.railyatri.in/time-table",
        "https://www.railyatri.in/live-train-status",
        "https://etrain.info",
        "https://www.irctc.co.in/online-charts",
    ];

    Ok(Json(SourceStatus {
        live_enabled: state.live_enabled(),
        mode: "live",
        cache_ttl_seconds: state.config.cache_ttl.as_secs(),
        primary_source: primary.to_string(),
        verification_links,
        notice: "Live data is fetched first from the official Indian Railways enquiry system (enquiry.indianrail.gov.in), with Railyatri as fallback. PNR status is served from Railyatri because the government PNR portal requires a CAPTCHA. Availability and prepared-chart data come from IRCTC (www.irctc.co.in), which is Akamai-protected and IP-geofenced to India. Nothing is simulated."
            .to_string(),
        sources: sources
            .into_iter()
            .map(|(n, r)| SourceHealth {
                name: n,
                reachable: r,
            })
            .collect(),
    }))
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
