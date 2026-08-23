//! AI insight service logic: ground -> prompt -> single-shot completion.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::core::ai::ChatMessage;
use crate::core::error::AppError;
use crate::state::AppState;

/// Cap on the grounded JSON handed to the model so a huge timetable cannot
/// blow the context window; anything beyond is cut and marked.
const MAX_DATA_CHARS: usize = 16_000;

/// Strict grounding contract: the model may only restate what the JSON says,
/// never fill gaps from its own "knowledge".
const SYSTEM_PROMPT: &str = "\
You explain Indian Railways data to travellers. Answer ONLY from the JSON in \
the user message; do not use outside knowledge. Use plain language of at most \
120 words. Cite exact numbers verbatim (times, delays, station codes, dates). \
If the provided data is insufficient to answer, say so explicitly. Never \
invent schedules, delays or stations.";

#[derive(Debug, Serialize, Deserialize)]
pub struct InsightResponse {
    pub kind: String,
    pub summary: String,
    pub model: String,
    pub data_source: String,
    pub cached: bool,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

pub struct Service;

impl Service {
    /// Explain a grounded rail-data result in plain language.
    ///
    /// Flow: disabled check -> insight cache -> ground from the sibling slice
    /// service (its errors propagate honestly) -> single-shot completion over
    /// the Zen gateway (timed into `zen` source latency) -> cache the DTO.
    pub async fn get_insight(
        state: &AppState,
        kind: &str,
        train: &str,
        src: &str,
        dst: &str,
    ) -> Result<InsightResponse, AppError> {
        ensure_ai_enabled(state)?;

        let cache_key = cache_key(kind, train, src, dst);
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(mut resp) = serde_json::from_value::<InsightResponse>(cached) {
                resp.cached = true;
                return Ok(resp);
            }
        }

        let data = truncate_for_prompt(&ground(state, kind, train, src, dst).await?);
        // The subject line states the requested entity explicitly so
        // grounding survives even when bulky stop history is clamped.
        let subject = if train.is_empty() {
            format!("{src} -> {dst}")
        } else {
            format!("train {train}")
        };

        let started = Instant::now();
        let (summary, prompt_tokens, completion_tokens, backend_tag) = state
            .ai_chat_complete(&[
                ChatMessage::system(SYSTEM_PROMPT),
                ChatMessage::user(format!(
                    "Question: explain this {kind} result\nSubject: {subject}\nData:\n{data}"
                )),
            ])
            .await?;
        let elapsed = started.elapsed();
        state.metrics.record_source_latency(backend_tag, elapsed);

        let resp = InsightResponse {
            kind: kind.to_string(),
            summary,
            model: state.ai.model().to_string(),
            data_source: format!("{backend_tag}+ntes"),
            cached: false,
            prompt_tokens,
            completion_tokens,
        };

        // The empty parameter is absent for this kind; keep logs unambiguous.
        if train.is_empty() {
            tracing::info!(
                kind = %kind,
                %src,
                %dst,
                source = backend_tag,
                latency_ms = elapsed.as_millis(),
                prompt_tokens,
                completion_tokens,
                "ai insight generated"
            );
        } else {
            tracing::info!(
                kind = %kind,
                %train,
                source = backend_tag,
                latency_ms = elapsed.as_millis(),
                prompt_tokens,
                completion_tokens,
                "ai insight generated"
            );
        }

        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
    }
}

fn ensure_ai_enabled(state: &AppState) -> Result<(), AppError> {
    if !state.config.ai_enabled {
        return Err(AppError::source_unavailable(
            "zen",
            "AI disabled by configuration",
        ));
    }
    Ok(())
}

/// Call the grounding slice directly in-process and serialize its DTO to
/// compact JSON. Inner errors are propagated as-is: an honest 502 about the
/// failed source beats a confident summary of absent data.
async fn ground(
    state: &AppState,
    kind: &str,
    train: &str,
    src: &str,
    dst: &str,
) -> Result<String, AppError> {
    let value = match kind {
        "live_status" => {
            // Empty date = today IST; the live-status service resolves the
            // active run with its own IST-today handling.
            serde_json::to_value(
                crate::slices::live_status::service::Service::get_live_status(state, train, "")
                    .await?,
            )?
        }
        "average_delay" => serde_json::to_value(
            crate::slices::average_delay::service::Service::get_average_delay(state, train).await?,
        )?,
        "trains_between" => serde_json::to_value(
            crate::slices::trains_between::service::Service::get_trains_between(state, src, dst)
                .await?,
        )?,
        other => {
            return Err(AppError::bad_request(format!(
                "Unknown insight kind: {other}"
            )))
        }
    };
    Ok(serde_json::to_string(&value)?)
}

fn truncate_for_prompt(data: &str) -> String {
    if data.len() <= MAX_DATA_CHARS {
        return data.to_string();
    }
    let mut cut = MAX_DATA_CHARS;
    while !data.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}...[truncated]", &data[..cut])
}

fn cache_key(kind: &str, train: &str, src: &str, dst: &str) -> String {
    let params = if train.is_empty() {
        format!("{src}:{dst}")
    } else {
        train.to_string()
    };
    format!("ai-insight:{kind}:{params}")
}
