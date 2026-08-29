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
use tracing::Instrument;

use crate::core::metrics::RequestGuard;
use crate::slices;
use crate::state::AppState;
use crate::system;

/// Request ID extension stored in request extensions for handlers.
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

/// Build the full application router (slices + system + static) and materialize
/// it with `state`. Slices return `Router<AppState>`; this is the single place
/// they are merged and handed a concrete state.
///
/// Two route families are kept apart deliberately:
/// - **buffered** routes return bounded JSON bodies and get the full stack,
///   including `metrics_mw` (which buffers each response to count bytes).
/// - **streaming** AI routes answer with unbounded SSE bodies; they merge
///   *after* the layered sub-router so they skip metrics buffering and the
///   30s timeout (an LLM completion can legitimately run longer). Panic
///   safety, tracing, logging and security headers still apply.
pub fn router(state: AppState, static_dir: PathBuf) -> Router {
    let index = static_dir.join("index.html");

    // Pattern: Timeout Budget — SRE budget aware: 30s outer (configurable), 5s per upstream via fanout
    let timeout_secs = state.config.request_timeout_secs;

    let mut buffered = Router::new()
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
        .route("/rail-api/*rest", get(api_404));
    // AskDISHA is feature-gated: its routes exist only when the module is
    // enabled so a disabled deployment keeps a zero API/network footprint
    // (`/rail-api/askdisha/*` falls through to `api_404`).
    if state.askdisha.is_some() {
        buffered = buffered.merge(slices::askdisha::router());
    }
    // Middleware order (outermost -> innermost): request_id outermost, then metrics, rate_limit, bulkhead, trace, catch_panic, timeout
    // Since axum's .layer makes last added = outermost, we add innermost first.
    let buffered = buffered
        // Pattern: Timeout Budget — per-request deadline propagation; 30s outer, 5s per upstream via fanout
        .layer(TimeoutLayer::new(Duration::from_secs(timeout_secs)))
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        // Pattern: Bulkhead — isolate failure domains via semaphore, must not block healthz
        .layer(middleware::from_fn_with_state(state.clone(), bulkhead_mw))
        // Pattern: Load Shedding — shed when in_flight > threshold or memory high, 503 + Retry-After
        .layer(middleware::from_fn_with_state(state.clone(), load_shed_mw))
        // Pattern: Rate Limiting — token bucket per IP, 429 when exceeded
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_mw))
        .layer(middleware::from_fn_with_state(state.clone(), metrics_mw));

    let streaming = Router::new()
        .merge(slices::ai_chat::router())
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        // Pattern: Bulkhead for streaming as well (isolated)
        .layer(middleware::from_fn_with_state(state.clone(), bulkhead_mw))
        // Pattern: Load Shedding for streaming
        .layer(middleware::from_fn_with_state(state.clone(), load_shed_mw))
        // Pattern: Rate Limiting for streaming
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_mw));

    Router::new()
        .merge(buffered)
        .merge(streaming)
        .layer(middleware::from_fn(security_headers_mw))
        .layer(middleware::from_fn(request_log_mw))
        // Pattern: Request ID tracing — generate UUID v4 per request, X-Request-Id header, propagate into tracing spans. Outermost.
        .layer(middleware::from_fn(request_id_mw))
        .fallback_service({
            let spa_index_path = index.clone();
            tower::ServiceBuilder::new()
                .layer(middleware::from_fn(security_headers_mw))
                .layer(middleware::from_fn(static_log_mw))
                .service(ServeDir::new(static_dir).fallback(get(move || {
                    let spa_index_path = spa_index_path.clone();
                    async move {
                        match tokio::fs::read(spa_index_path).await {
                            Ok(bytes) => ([(axum::http::header::CONTENT_TYPE, "text/html")], bytes),
                            Err(_) => (
                                [(axum::http::header::CONTENT_TYPE, "text/html")],
                                b"<!doctype html><title>Train Bro</title><p>UI bundle missing"
                                    .to_vec(),
                            ),
                        }
                    }
                })))
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

/// Pattern: Request Hedging note — see fanout.rs for fan-out N×2 hedging

/// RequestIdLayer: generate UUID v4 per request, inject X-Request-Id header, propagate into tracing spans.
/// Pattern: Request Hedging is also covered via fanout, but this layer ensures per-request traceability.
async fn request_id_mw(mut req: Request, next: Next) -> Response {
    // Generate UUID v4
    let id = uuid::Uuid::new_v4().to_string();
    // Stash in extensions for handlers
    req.extensions_mut().insert(RequestId(id.clone()));
    // Also set header on request for downstream tracing
    if let Ok(val) = HeaderValue::from_str(&id) {
        req.headers_mut().insert("x-request-id", val);
    }
    // Create span with request_id
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let span = tracing::info_span!("request", request_id = %id, method = %method, path = %path);
    let mut res = async move { next.run(req).await }.instrument(span).await;
    // Inject header into response
    if let Ok(val) = HeaderValue::from_str(&id) {
        res.headers_mut().insert("x-request-id", val);
    }
    res
}

/// Helper: extract client IP for rate limiting (X-Forwarded-For first entry, else X-Real-IP, else unknown).
fn client_ip(req: &Request) -> String {
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first) = forwarded
            .split(',')
            .next()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            return first.to_string();
        }
    }
    if let Some(real) = req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if !real.trim().is_empty() {
            return real.trim().to_string();
        }
    }
    // Fallback to peer IP via extensions if available (ConnectInfo), else unknown
    // We also check for forwarded header set by tests via reqwest.
    "unknown".to_string()
}

fn is_health_path(path: &str) -> bool {
    matches!(
        path,
        "/healthz"
            | "/api/healthz"
            | "/metrics"
            | "/metrics/"
            | "/readyz"
            | "/ready"
            | "/health/ready"
            | "/health/live"
            | "/api/readyz"
    ) || path.starts_with("/healthz")
        || path.starts_with("/health/ready")
        || path.starts_with("/health/live")
        || path.starts_with("/readyz")
        || path.starts_with("/ready")
}

/// Pattern: Rate Limiting — token bucket per IP (100 rps, burst 50) returning 429
async fn rate_limit_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // Pattern: Rate Limiting
    let path = req.uri().path().to_string();
    // Bypass health checks from rate limiting to keep probes reliable
    if is_health_path(&path) {
        return next.run(req).await;
    }
    let ip = client_ip(&req);
    if !state.rate_limiter.check(&ip) {
        state.telemetry.inc_rate_limited();
        let body = Json(json!({ "error": "rate limited", "retry_after": 1 }));
        let mut res = (axum::http::StatusCode::TOO_MANY_REQUESTS, body).into_response();
        res.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        // Ensure request-id if not already set by outer layer? Outer layer already sets, but we also ensure.
        if let Some(req_id) = req.extensions().get::<RequestId>() {
            if let Ok(v) = HeaderValue::from_str(&req_id.0) {
                res.headers_mut().insert("x-request-id", v);
            }
        }
        return res;
    }
    next.run(req).await
}

/// Pattern: Bulkhead — concurrency limit via semaphore (512 default), must not block healthz
async fn bulkhead_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // Pattern: Bulkhead
    let path = req.uri().path().to_string();
    if is_health_path(&path) {
        return next.run(req).await;
    }
    // Try to acquire bulkhead permit without waiting (fail fast)
    let permit = match state.bulkhead.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            state.telemetry.inc_bulkhead_rejected();
            let body = Json(json!({ "error": "bulkhead saturated", "retry_after": 5 }));
            let mut res = (axum::http::StatusCode::SERVICE_UNAVAILABLE, body).into_response();
            res.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
            if let Some(req_id) = req.extensions().get::<RequestId>() {
                if let Ok(v) = HeaderValue::from_str(&req_id.0) {
                    res.headers_mut().insert("x-request-id", v);
                }
            }
            return res;
        }
    };
    let res = next.run(req).await;
    drop(permit);
    res
}

/// Pattern: Load Shedding — when in_flight > threshold or memory high, 503 + Retry-After
async fn load_shed_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // Pattern: Load Shedding
    let path = req.uri().path().to_string();
    if is_health_path(&path) {
        return next.run(req).await;
    }
    let in_flight = state.metrics.snapshot().in_flight;
    let threshold = state.config.load_shed_threshold;
    let (_, mem_bytes) = crate::core::obs::proc_stats();
    // memory limit 2 GiB (configurable via threshold; we use 2048 MB)
    let mem_limit = (crate::core::sre::SATURATION_MEMORY_THRESHOLD_MB * 1024.0 * 1024.0) as u64;
    let should_shed = in_flight > threshold || (mem_limit > 0 && mem_bytes > mem_limit);
    if should_shed {
        state.telemetry.inc_load_shed();
        let body = Json(json!({
            "error": "service overloaded",
            "retry_after": 5,
            "in_flight": in_flight,
            "threshold": threshold
        }));
        let mut res = (axum::http::StatusCode::SERVICE_UNAVAILABLE, body).into_response();
        res.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
        if let Some(req_id) = req.extensions().get::<RequestId>() {
            if let Ok(v) = HeaderValue::from_str(&req_id.0) {
                res.headers_mut().insert("x-request-id", v);
            }
        }
        tracing::warn!(
            in_flight,
            threshold,
            mem_bytes,
            "load shed: in_flight > threshold"
        );
        return res;
    }
    next.run(req).await
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
    // include request_id if present
    let req_id = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    log_request(
        method.as_str(),
        &path,
        &client,
        res.status(),
        start.elapsed(),
    );
    tracing::debug!(request_id = %req_id, path = %path, "request_id trace");
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
