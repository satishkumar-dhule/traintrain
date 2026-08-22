//! AI assistant slice: streaming chat relayed to the configured OpenAI-
//! compatible gateway. Phase A ships the status surface (the contract the UI
//! uses to enable/disable itself); the SSE chat route lands with this slice's
//! Phase B.

pub mod service;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::state::AppState;

/// Feature status so the SPA can render honest empty states.
#[derive(serde::Serialize)]
pub struct AiStatus {
    pub enabled: bool,
    pub model: String,
    /// Whether an API key is configured (`false` = keyless free tier).
    pub keyed: bool,
    pub base: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ai/status", get(status_handler))
}

/// GET /rail-api/ai/status — configuration truth, no upstream probing.
async fn status_handler(State(state): State<AppState>) -> Json<AiStatus> {
    Json(service::status(&state))
}
