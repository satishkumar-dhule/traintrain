//! Observability slice.
//!
//! Endpoint: `GET /rail-api/observability` -> `crate::models::ObservabilityResponse`.
//!
//! All numbers are real runtime metrics from `crate::core::metrics` plus
//! per-source latency/status for Railyatri, etrain and NTES. `origins` names
//! are the actual live sources, never fake relays. CPU usage is sampled from
//! `/proc/self/stat`; memory from `/proc/self/statm`; `latency_ms` is the
//! recent average request time. If `/proc` is unavailable fall back to 0
//! honestly.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::models::ObservabilityResponse;
use crate::state::AppState;

pub mod service;

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/observability", get(observability))
}

/// Serve a real runtime observability snapshot as JSON.
async fn observability(State(state): State<AppState>) -> Json<ObservabilityResponse> {
    Json(service::Service::snapshot(&state))
}
