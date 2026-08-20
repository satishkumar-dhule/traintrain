//! System endpoints: liveness (`/healthz`, `/api/healthz`), Prometheus metrics
//! (`/metrics`) and live source status (`/rail-api/source-status`). These
//! report real runtime facts only.

use axum::extract::State;
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::core::error::AppError;
use crate::models::{Healthz, SourceHealth, SourceStatus};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/healthz", get(healthz))
        .route("/metrics", get(metrics))
        .route("/sitemap.xml", get(sitemap))
        .route("/rail-api/source-status", get(source_status))
        .route("/rail-api/debug", post(debug_report))
}

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
