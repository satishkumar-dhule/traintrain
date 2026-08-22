//! AI assistant slice: streaming chat relayed to the configured OpenAI-
//! compatible gateway. The server owns the persona (exactly one system turn,
//! prepended here); clients speak user/assistant turns only.

pub mod service;

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::Stream;
use futures::StreamExt;
use serde::Deserialize;

use crate::core::ai::ChatMessage;
use crate::core::error::AppError;
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

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub messages: Vec<InboundMessage>,
}

#[derive(Debug, Deserialize)]
pub struct InboundMessage {
    pub role: String,
    pub content: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/ai/status", get(status_handler))
        .route("/rail-api/ai/chat", post(chat_handler))
}

/// GET /rail-api/ai/status — configuration truth, no upstream probing.
async fn status_handler(State(state): State<AppState>) -> Json<AiStatus> {
    Json(service::status(&state))
}

/// POST /rail-api/ai/chat — validate, prepend the persona, relay Zen's SSE
/// stream. Pre-stream failures answer as JSON errors; once headers are
/// committed, errors ride in-band as `{"type":"error"}` events.
async fn chat_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    ensure_enabled(&state)?;
    let mut messages = service::validate_messages(&req.messages)?;
    messages.insert(0, ChatMessage::system(service::PERSONA));

    let events = state.ai.chat_stream(&messages).await?;
    let stream = events.map(|item| Ok(service::encode_event(item)));
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Shared guard for AI endpoints: disabled-by-config is surfaced as an
/// honest source-unavailable error rather than a silent 404.
fn ensure_enabled(state: &AppState) -> Result<(), AppError> {
    if state.config.ai_enabled {
        Ok(())
    } else {
        Err(AppError::source_unavailable(
            "zen",
            "AI disabled by configuration",
        ))
    }
}
