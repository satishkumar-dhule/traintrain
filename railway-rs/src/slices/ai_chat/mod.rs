//! AI assistant slice: streaming chat relayed to the configured OpenAI-
//! compatible gateway through an agentic tool loop. The server owns the
//! persona (exactly one system turn) and a registry of local rail tools
//! ([`tools`]); when the model requests one, the loop executes it against
//! real upstreams in-process and feeds the result back until the model
//! answers. Clients speak user/assistant turns only.

pub mod service;
pub mod tools;

use std::convert::Infallible;
use std::time::Instant;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::Stream;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::core::ai::{AiEvent, AssembledToolCall, ChatMessage};
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
    /// Active backend tag (`zen` gateway or `local` in-process engine).
    pub backend: String,
    /// Once-per-request fallback backend tag under `local-first`.
    pub fallback: Option<String>,
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

/// Bound on model tool rounds per chat request; runaway loops end with an
/// honest in-band error instead of burning tokens forever.
const MAX_TOOL_ROUNDS: usize = 4;

/// One model round's decoded event stream (boxed so every round shares one
/// concrete type across the loop).
type AiEventStream = std::pin::Pin<Box<dyn Stream<Item = Result<AiEvent, AppError>> + Send>>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/ai/status", get(status_handler))
        .route("/rail-api/ai/chat", post(chat_handler))
}

/// GET /rail-api/ai/status — configuration truth, no upstream probing.
async fn status_handler(State(state): State<AppState>) -> Json<AiStatus> {
    Json(service::status(&state))
}

/// POST /rail-api/ai/chat — validate, prepend the persona, then run the
/// agentic loop: stream each model round straight through to the client,
/// execute requested tools locally, feed results back, until a plain answer
/// completes or the round budget runs out. Pre-stream failures answer as
/// JSON errors; once headers are committed, errors ride in-band as
/// `{"type":"error"}` events.
async fn chat_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    ensure_enabled(&state)?;
    let mut messages = service::trim_history(
        service::validate_messages(&req.messages)?,
        service::HISTORY_MAX_CHARS,
    );

    // Fail fast: round 1 is established here, before any headers are
    // committed, so pre-stream failures surface as honest JSON errors. The
    // persona matches the resolved backend, and `local-first` setups get one
    // fallback attempt on the other backend before giving up.
    let schemas = tools::schemas();
    let primary = state.ai.clone();
    let mut chosen = primary.clone();
    messages.insert(0, ChatMessage::system(service::persona_for(primary.tag())));
    let first = match primary.chat_stream_with_tools(&messages, &schemas).await {
        Ok(s) => s,
        Err(e) => {
            let Some(fb) = state.ai_fallback.clone() else {
                return Err(e);
            };
            tracing::warn!(
                error = %e.message(),
                from = %primary.tag(),
                to = %fb.tag(),
                "primary ai backend failed pre-stream; failing over"
            );
            messages[0] = ChatMessage::system(service::persona_for(fb.tag()));
            let stream = fb.chat_stream_with_tools(&messages, &schemas).await?;
            chosen = fb;
            stream
        }
    };
    let budget = tools::Budget::new(tools::DEFAULT_BUDGET_CHARS);

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);
    tokio::spawn(async move {
        let backend = chosen;
        let mut rounds = 0usize;
        // Last successful projection per executed tool, feeding next-actions.
        let mut last_results: Vec<(String, Value)> = Vec::new();
        let mut current: Option<AiEventStream> = Some(Box::pin(first));
        loop {
            rounds += 1;
            if rounds > MAX_TOOL_ROUNDS {
                let _ = tx
                    .send(Ok(service::error_frame("too many tool steps")))
                    .await;
                break;
            }
            let start = Instant::now();
            let mut stream = match current.take() {
                Some(s) => s,
                None => match backend.chat_stream_with_tools(&messages, &schemas).await {
                    Ok(s) => Box::pin(s),
                    Err(e) => {
                        tracing::warn!(
                            error = %e.message(),
                            round = rounds,
                            "ai chat round failed"
                        );
                        state
                            .metrics
                            .record_source_latency(backend.tag(), start.elapsed());
                        let _ = tx.send(Ok(service::error_frame(&e.message()))).await;
                        break;
                    }
                },
            };
            let mut calls: Vec<AssembledToolCall> = Vec::new();
            let mut usage = (0u64, 0u64);
            let mut errored = false;
            while let Some(item) = stream.next().await {
                match item {
                    Ok(AiEvent::ToolCalls(c)) => calls = c,
                    Ok(AiEvent::Done {
                        prompt_tokens,
                        completion_tokens,
                    }) => usage = (prompt_tokens, completion_tokens),
                    Ok(other) => {
                        let _ = tx.send(Ok(service::encode_event(Ok(other)))).await;
                    }
                    Err(e) => {
                        errored = true;
                        let _ = tx.send(Ok(service::encode_event(Err(e)))).await;
                    }
                }
            }
            drop(stream);
            state
                .metrics
                .record_source_latency(backend.tag(), start.elapsed());

            if calls.is_empty() {
                if !errored {
                    let items: Vec<Value> = tools::next_actions(&last_results)
                        .into_iter()
                        .map(|(label, prompt)| json!({ "label": label, "prompt": prompt }))
                        .collect();
                    let _ = tx
                        .send(Ok(Event::default()
                            .data(json!({"type": "actions", "items": items}).to_string())))
                        .await;
                    let _ = tx.send(Ok(service::done_frame(usage.0, usage.1))).await;
                    tracing::info!(
                        source = %backend.tag(),
                        model = %backend.model(),
                        latency_ms = start.elapsed().as_millis() as u64,
                        prompt_tokens = usage.0,
                        completion_tokens = usage.1,
                        rounds,
                        "ai chat complete"
                    );
                }
                break;
            }

            let names: Vec<&str> = calls.iter().map(|c| c.name.as_str()).collect();
            tracing::info!(tools = ?names, round = rounds, budget_chars = budget.remaining(), "executing ai chat tools");
            let _ = tx
                .send(Ok(
                    Event::default().data(json!({"type": "tools", "names": names}).to_string())
                ))
                .await;

            messages.push(ChatMessage::assistant_with_tool_calls(
                calls.iter().map(openai_tool_call).collect(),
            ));

            // Parallel execution: independent tool calls resolve concurrently
            // (join_all keeps call order for result pairing); each is bounded
            // by its own timeout inside `call_tool`, and failures become
            // error payloads the model can reason about — never HTTP errors.
            let results = futures::future::join_all(calls.iter().map(|call| {
                let state = &state;
                let budget = &budget;
                async move {
                    match tools::call_tool(state, budget, &call.name, &call.arguments).await {
                        Ok(out) => out,
                        Err(e) => json!({"error": e.message()}).to_string(),
                    }
                }
            }))
            .await;
            for (call, payload) in calls.iter().zip(results) {
                // Rich-card frame: the projected tool output rendered as a UI
                // component in the transcript. Budget-exhaustion markers are
                // not card-worthy.
                if let Ok(view) = serde_json::from_str::<Value>(&payload) {
                    if view.get("context_budget_exhausted").is_none() {
                        let _ = tx
                            .send(Ok(Event::default().data(
                                json!({
                                    "type": "card",
                                    "kind": call.name,
                                    "call_id": call.id,
                                    "data": view.clone(),
                                })
                                .to_string(),
                            )))
                            .await;
                        last_results.push((call.name.to_string(), view));
                    }
                }
                messages.push(ChatMessage::tool_result(call.id.clone(), payload));
            }
        }
    });

    let rx_stream = futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    });
    Ok(Sse::new(rx_stream).keep_alive(KeepAlive::default()))
}

fn openai_tool_call(c: &AssembledToolCall) -> serde_json::Value {
    json!({
        "id": c.id,
        "type": "function",
        "function": {"name": c.name, "arguments": c.arguments}
    })
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
