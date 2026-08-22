//! Top-level router: merges every vertical slice plus system endpoints,
//! applies shared middleware (metrics, trace, catch-panic, timeout) and serves
//! the static SPA with an `index.html` fallback for client-side routing.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::header;
use axum::http::HeaderValue;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::core::metrics::RequestGuard;
use crate::slices;
use crate::state::AppState;
use crate::system;

/// Build the full application router (slices + system + static) and materialize
/// it with `state`. Slices return `Router<AppState>`; this is the single place
/// they are merged and handed a concrete state.
pub fn router(state: AppState, static_dir: PathBuf) -> Router {
    let index = static_dir.join("index.html");

    Router::new()
        .merge(system::router())
        .merge(slices::pnr::router())
        .merge(slices::schedule::router())
        .merge(slices::live_status::router())
        .merge(slices::live_station::router())
        .merge(slices::trains_between::router())
        .merge(slices::availability::router())
        .merge(slices::chart::router())
        .merge(slices::exceptional::router())
        .merge(slices::station_timetable::router())
        .merge(slices::average_delay::router())
        .merge(slices::heritage::router())
        .merge(slices::parcel::router())
        .merge(slices::journey_basis::router())
        .merge(slices::train_on_map::router())
        .merge(slices::stations::router())
        .merge(slices::search::router())
        .merge(slices::observability::router())
        .route("/rail-api/*rest", get(api_404))
        .layer(middleware::from_fn_with_state(state.clone(), metrics_mw))
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .layer(middleware::from_fn(security_headers_mw))
        .layer(middleware::from_fn(request_log_mw))
        .fallback_service({
            let spa_index_path = index.clone();
            tower::ServiceBuilder::new()
                .layer(middleware::from_fn(security_headers_mw))
                .layer(middleware::from_fn(static_log_mw))
                .service(
                    ServeDir::new(static_dir).not_found_service(get(move || {
                        let spa_index_path = spa_index_path.clone();
                        async move {
                            match tokio::fs::read(spa_index_path).await {
                                Ok(bytes) => (
                                    [(axum::http::header::CONTENT_TYPE, "text/html")],
                                    bytes,
                                ),
                                Err(_) => (
                                    [(axum::http::header::CONTENT_TYPE, "text/html")],
                                    b"<!doctype html><title>RailCompanion</title><p>UI bundle missing".to_vec(),
                                ),
                            }
                        }
                    })),
                )
        })
        .with_state(state)
}

/// JSON 404 for unmatched `/rail-api/*` paths (static fallback handles the rest).
async fn api_404() -> impl IntoResponse {
    (
        axum::http::StatusCode::NOT_FOUND,
        Json(json!({ "error": "not found" })),
    )
}

/// Record per-request path counters, in-flight count, status code, latency and
/// Prometheus telemetry. Applied once at the top level so every route
/// (including static) is counted.
async fn metrics_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let _guard: RequestGuard<'_> = state.metrics.begin_request();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    state.metrics.record_path(&path);
    let start = Instant::now();
    let res = next.run(req).await;
    let status = res.status().as_u16();
    let (parts, body) = res.into_parts();
    let bytes = axum::body::to_bytes(body, 64 * 1024 * 1024)
        .await
        .unwrap_or_default();
    let n = bytes.len() as u64;
    let elapsed = start.elapsed();
    state.metrics.record_status(status);
    state.metrics.record_request_latency(elapsed);
    state.metrics.add_bytes(n);
    state
        .telemetry
        .record_http(method.as_str(), &path, status, elapsed);
    Response::from_parts(parts, axum::body::Body::from(bytes))
}

/// Log every routed request (method, path, status, latency, client) so the
/// UI -> API flow is traceable end-to-end in the server log. Applies to all
/// `/rail-api/*` routes and system endpoints (outermost layer).
async fn request_log_mw(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let client = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();
    let start = Instant::now();
    let res = next.run(req).await;
    log_request(
        method.as_str(),
        &path,
        &client,
        res.status(),
        start.elapsed(),
    );
    res
}

/// Log requests served from the static fallback service (the SPA files) and
/// force revalidation so the browser never keeps serving stale JS while the
/// UI is being debugged.
async fn static_log_mw(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let start = Instant::now();
    let mut res = next.run(req).await;
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    log_request("GET", &path, "-", res.status(), start.elapsed());
    res
}

fn log_request(
    method: &str,
    path: &str,
    client: &str,
    status: axum::http::StatusCode,
    latency: Duration,
) {
    let status_code = status.as_u16();
    let latency_ms = latency.as_millis();
    if status.is_server_error() {
        tracing::error!(%method, %path, %client, %status_code, %latency_ms, "request");
    } else if !status.is_success() {
        tracing::warn!(%method, %path, %client, %status_code, %latency_ms, "request");
    } else {
        tracing::info!(%method, %path, %client, %status_code, %latency_ms, "request");
    }
}

/// Baseline security response headers on every response (outermost layer).
async fn security_headers_mw(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https://*.tile.openstreetmap.org; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'self'",
        ),
    );
    res
}
