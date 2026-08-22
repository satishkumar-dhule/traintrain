//! AI assistant service logic: request validation, persona and SSE wire
//! encoding for the chat relay; configuration status for the status surface.

use axum::response::sse::Event;
use serde_json::json;

use crate::core::ai::{AiEvent, ChatMessage};
use crate::core::error::AppError;
use crate::state::AppState;

use super::{AiStatus, InboundMessage};

/// Server-owned persona, prepended exactly once per chat request so clients
/// cannot override or duplicate it.
pub const PERSONA: &str = "You are Train Bro, a factual Indian Railways assistant. \
Rely only on live data supplied in the conversation; never invent trains, fares, \
availability or delays. If data is missing say so plainly. Answer briefly.";

const MAX_MESSAGES: usize = 40;
const MAX_CONTENT_CHARS: usize = 32_000;

/// Configuration-derived status. No upstream call: the SPA gates on this
/// instantly and honest errors arrive per-request if the gateway is down.
pub fn status(state: &AppState) -> AiStatus {
    AiStatus {
        enabled: state.config.ai_enabled,
        model: state.config.ai_model.clone(),
        keyed: state.config.ai_api_key.is_some(),
        base: state.config.ai_base.clone(),
    }
}

/// Validate inbound turns: user/assistant roles only (the server owns system),
/// non-blank bounded content, bounded conversation length.
pub fn validate_messages(raw: &[InboundMessage]) -> Result<Vec<ChatMessage>, AppError> {
    if raw.is_empty() {
        return Err(AppError::bad_request("messages must not be empty"));
    }
    if raw.len() > MAX_MESSAGES {
        return Err(AppError::bad_request(format!(
            "too many messages: {} > {MAX_MESSAGES}",
            raw.len()
        )));
    }
    let mut out = Vec::with_capacity(raw.len() + 1);
    for m in raw {
        let role = m.role.trim().to_ascii_lowercase();
        if role != "user" && role != "assistant" {
            return Err(AppError::bad_request(
                "role must be 'user' or 'assistant' (system is owned by the server)",
            ));
        }
        let trimmed = m.content.trim();
        if trimmed.is_empty() {
            return Err(AppError::bad_request("message content must not be blank"));
        }
        let chars = trimmed.chars().count();
        if chars > MAX_CONTENT_CHARS {
            return Err(AppError::bad_request(format!(
                "message too long: {chars} > {MAX_CONTENT_CHARS} characters"
            )));
        }
        out.push(ChatMessage {
            role,
            content: trimmed.to_string(),
        });
    }
    Ok(out)
}

/// Encode one relayed item as an SSE `data:` payload. Upstream failures that
/// arrive mid-stream become in-band error events — headers are already sent.
pub fn encode_event(item: Result<AiEvent, AppError>) -> Event {
    let payload = match item {
        Ok(AiEvent::Reasoning(text)) => json!({"type": "reasoning", "text": text}),
        Ok(AiEvent::Delta(text)) => json!({"type": "delta", "text": text}),
        Ok(AiEvent::Done {
            prompt_tokens,
            completion_tokens,
        }) => json!({
            "type": "done",
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens
        }),
        Err(e) => {
            tracing::warn!(error = %e.message(), "relaying ai stream error in-band");
            json!({"type": "error", "message": e.message()})
        }
    };
    Event::default().data(payload.to_string())
}
