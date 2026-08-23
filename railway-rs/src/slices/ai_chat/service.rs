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
availability or delays. If data is missing say so plainly. Answer briefly. \
Format with clean Markdown: short paragraphs, bullet lists for options, tables \
for schedules, **bold** for key facts; no headings larger than ###.";

/// Compact persona for the in-process micro model: shorter context, plain
/// formatting (micro models mangle tables), explicit no-code-fence rule.
pub const PERSONA_LOCAL: &str = "You are Train Bro, a factual Indian Railways \
assistant. Use only the data provided in this conversation; never invent trains, \
times, fares or delays. If data is missing, say so plainly in one short line. \
Reply in 1-4 short sentences or a few simple bullet lines. Plain text only: no \
code blocks, no backticks, no headings, no markdown tables.";

const MAX_MESSAGES: usize = 40;
const MAX_CONTENT_CHARS: usize = 32_000;

/// Server-side context cap for a conversation (chars across all turns).
/// Newest turns win; the last two messages are never dropped so the current
/// question always keeps its immediate predecessor.
pub const HISTORY_MAX_CHARS: usize = 12_000;

/// Drop oldest turns while over budget. Pure; unit-tested.
pub fn trim_history(msgs: Vec<ChatMessage>, max_chars: usize) -> Vec<ChatMessage> {
    let total = |v: &[ChatMessage]| -> usize { v.iter().map(|m| m.content.chars().count()).sum() };
    let mut out = msgs;
    while out.len() > 2 && total(&out) > max_chars {
        out.remove(0);
    }
    out
}

/// Configuration-derived status. No upstream call: the SPA gates on this
/// instantly and honest errors arrive per-request if the gateway is down.
pub fn status(state: &AppState) -> AiStatus {
    AiStatus {
        enabled: state.config.ai_enabled,
        model: state.ai.model().to_string(),
        keyed: state.config.ai_api_key.is_some(),
        base: state.config.ai_base.clone(),
        backend: state.ai.tag().to_string(),
        fallback: state.ai_fallback.as_ref().map(|b| b.tag().to_string()),
    }
}

/// Persona matching a backend's strengths.
pub fn persona_for(backend_tag: &str) -> &'static str {
    if backend_tag == "local" {
        PERSONA_LOCAL
    } else {
        PERSONA
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
            tool_call_id: None,
            tool_calls: None,
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
        // ToolCalls never reach the encoder: the handler intercepts them to
        // run local tools and emits a `tools` chip frame instead.
        Ok(AiEvent::ToolCalls(_)) | Ok(AiEvent::Done { .. }) => {
            json!({"type": "done", "prompt_tokens": 0, "completion_tokens": 0})
        }
        Err(e) => {
            tracing::warn!(error = %e.message(), "relaying ai stream error in-band");
            json!({"type": "error", "message": e.message()})
        }
    };
    Event::default().data(payload.to_string())
}

/// Terminal usage frame emitted once per chat request.
pub fn done_frame(prompt_tokens: u64, completion_tokens: u64) -> Event {
    Event::default()
        .data(json!({"type": "done", "prompt_tokens": prompt_tokens, "completion_tokens": completion_tokens}).to_string())
}

/// In-band error frame for failures after headers are committed.
pub fn error_frame(message: &str) -> Event {
    Event::default().data(json!({"type": "error", "message": message}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.into(),
            content: content.into(),
            tool_call_id: None,
            tool_calls: None,
        }
    }

    #[test]
    fn trims_oldest_first_but_keeps_last_two() {
        let msgs = vec![
            msg("user", &"a".repeat(9_000)),
            msg("assistant", &"b".repeat(4_000)),
            msg("user", "recent question"),
            msg("assistant", "recent answer"),
        ];
        let out = trim_history(msgs, 12_000);
        let total: usize = out.iter().map(|m| m.content.chars().count()).sum();
        assert!(total <= 12_000, "budget respected");
        assert_eq!(out.last().unwrap().content, "recent answer", "newest kept");
        assert!(
            out.iter().all(|m| m.content != "a".repeat(9_000)),
            "oldest oversized turn dropped"
        );
    }

    #[test]
    fn under_budget_is_untouched_and_min_two_kept() {
        let msgs = vec![msg("user", "tiny"), msg("assistant", "reply")];
        let out = trim_history(msgs.clone(), 12_000);
        assert_eq!(out.len(), 2);
        // Even a huge pair is never reduced below two turns.
        let huge = vec![
            msg("user", &"x".repeat(20_000)),
            msg("assistant", &"y".repeat(20_000)),
        ];
        assert_eq!(trim_history(huge, 100).len(), 2);
    }
}
