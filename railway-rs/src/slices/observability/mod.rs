//! Observability slice.
//!
//! Endpoints:
//! - `GET /rail-api/observability` -> `crate::models::ObservabilityResponse`
//!   Full runtime snapshot: request counters, per-source latency, CPU/mem,
//!   cache stats, status distribution, time-series and recent logs.
//! - `GET /rail-api/logs?limit=&level=` -> recent structured-log records from
//!   the in-memory ring (no external log shipper required).
//!
//! All numbers are real runtime metrics from `crate::core::metrics` plus
//! per-source latency/status for Railyatri, etrain, NTES and IRCTC. `origins`
//! names are the actual live sources, never fake relays. CPU usage is sampled
//! from `/proc/self/stat`; memory from `/proc/self/statm`; `latency_ms` is the
//! recent average request time. If `/proc` is unavailable fall back to 0
//! honestly.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::core::obs::log_ring;
use crate::models::{LogsQuery, LogsResponse, ObservabilityResponse};
use crate::state::AppState;

pub mod service;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/observability", get(observability))
        .route("/rail-api/logs", get(logs))
}

/// Serve a real runtime observability snapshot as JSON.
async fn observability(State(state): State<AppState>) -> Json<ObservabilityResponse> {
    Json(service::Service::snapshot(&state))
}

/// Tail the structured-log ring. `limit` caps the number of records (default
/// 100, max 500); `level` optionally filters to a minimum severity
/// (`debug|info|warn|error`). Records are newest-first.
async fn logs(Query(params): Query<LogsQuery>) -> Json<LogsResponse> {
    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let min_level = params
        .level
        .filter(|l| {
            matches!(
                l.to_lowercase().as_str(),
                "debug" | "info" | "warn" | "error"
            )
        })
        .map(|l| l.to_lowercase());
    let entries = log_ring().snapshot(limit, min_level.as_deref());
    Json(LogsResponse {
        total: entries.len(),
        limit,
        logs: entries,
    })
}
