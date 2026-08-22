//! AI insight slice: grounded, single-shot explanations of real rail data.
//!
//! Endpoint: `POST /rail-api/ai/insight`
//! Body: `{"kind":"live_status"|"average_delay"|"trains_between",
//! "params":{"train"?,"src"?,"dst"?}}`
//!
//! The router lives on the unbuffered streaming stack (`web.rs`) because an
//! LLM completion can legitimately run longer than the global 30s timeout;
//! errors returned before any body starts still surface as normal
//! `AppError` JSON (`SourceUnavailable` -> HTTP 502).
//!
//! Grounding contract: the requested sibling slice service is called directly
//! in-process (NTES web forms behind it), its DTO serialized into the prompt,
//! and the model answers ONLY from that JSON. Inner-source failures propagate
//! honestly - absent data is never summarized into a confident answer.
//! The final DTO is cached under `ai-insight:{kind}:{params}` so repeat
//! questions cost zero LLM calls.
//!
//! Success model: `crate::slices::ai_insight::service::InsightResponse`.

pub mod service;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::slices::station_codes::{code_known, normalize_code};
use crate::state::AppState;

#[derive(Deserialize)]
struct InsightRequest {
    kind: Option<String>,
    params: Option<InsightParams>,
}

#[derive(Deserialize, Default)]
struct InsightParams {
    #[serde(default)]
    train: Option<String>,
    #[serde(default)]
    src: Option<String>,
    #[serde(default)]
    dst: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ai/insight", post(insight_handler))
}

async fn insight_handler(
    State(state): State<AppState>,
    Json(req): Json<InsightRequest>,
) -> Result<Json<service::InsightResponse>, AppError> {
    let params = req.params.unwrap_or_default();
    let kind = req.kind.unwrap_or_default().trim().to_lowercase();

    match kind.as_str() {
        "live_status" | "average_delay" => {
            let train = params.train.unwrap_or_default().trim().to_string();
            if !(train.len() == 5 && train.chars().all(|c| c.is_ascii_digit()) && train != "00000")
            {
                return Err(AppError::bad_request("train must be a 5-digit number"));
            }
            Ok(Json(
                service::Service::get_insight(&state, &kind, &train, "", "").await?,
            ))
        }
        "trains_between" => {
            let src = normalize_code(params.src.as_deref());
            let dst = normalize_code(params.dst.as_deref());
            if src.is_empty() {
                return Err(AppError::bad_request("Missing required parameter: src"));
            }
            if dst.is_empty() {
                return Err(AppError::bad_request("Missing required parameter: dst"));
            }
            if !is_valid_station_shape(&src) {
                return Err(AppError::bad_request(format!(
                    "Invalid station code: {src}"
                )));
            }
            if !is_valid_station_shape(&dst) {
                return Err(AppError::bad_request(format!(
                    "Invalid station code: {dst}"
                )));
            }
            if src == dst {
                return Err(AppError::bad_request("Source and destination must differ."));
            }
            if !code_known(&state, &src) {
                return Err(AppError::bad_request(format!("Station {src} not found.")));
            }
            if !code_known(&state, &dst) {
                return Err(AppError::bad_request(format!("Station {dst} not found.")));
            }
            Ok(Json(
                service::Service::get_insight(&state, &kind, "", &src, &dst).await?,
            ))
        }
        other => Err(AppError::bad_request(format!(
            "Unknown insight kind: {other}"
        ))),
    }
}

/// Shape-only check (`^[A-Z0-9]{2,5}$` after trim+uppercase), applied before
/// the local-dataset lookup. Deliberately wider than
/// `station_codes::is_valid_code` (2..=4): NTES also serves 5-char freight/
/// junction codes.
fn is_valid_station_shape(code: &str) -> bool {
    (2..=5).contains(&code.len())
        && code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}
