//! Trains-between-stations slice.
//!
//! Endpoint: `GET /rail-api/ntes/trains-between?src=<CODE>&dst=<CODE>`
//!
//! Live source: NTES mobile API `TrainBtwStnJson` (see
//! `crate::core::ntes::NtesClient::trains_between`) with payload
//! `stnFrom=<SRC>&stnTo=<DST>&trainType=XXX`. The endpoint returns an empty
//! body from the sandbox - propagate `AppError::SourceUnavailable` honestly.
//!
//! Response list key is `trainBtwStationList` (community shape may also use
//! `trainList`); each entry maps to a `BetweenTrain` (`runs_on` is a 7-bool
//! array Monday-first, from `runOnMon`..`runOnSun` booleans, defaulting to
//! false when absent or non-bool).
//!
//! Validation: both `src` and `dst` are required, trimmed/uppercased 4-char
//! alphanumeric codes. Each must be a known station - matched case-insensitively
//! against `state.datasets.stations`; a code embedded as a token in an official
//! NTES train name (e.g. `MMCT` in `"MMCT NDLS RAJDHANI"`) is also accepted,
//! since such codes denote real stations. `src == dst` is rejected.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::TrainsBetweenResponse;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct TbQuery {
    src: Option<String>,
    dst: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/trains-between", get(trains_between_handler))
}

async fn trains_between_handler(
    State(state): State<AppState>,
    Query(q): Query<TbQuery>,
) -> Result<Json<TrainsBetweenResponse>, AppError> {
    let src = normalize(q.src.as_deref());
    let dst = normalize(q.dst.as_deref());

    if src.is_empty() {
        return Err(AppError::bad_request(
            "Missing required query parameter: src",
        ));
    }
    if dst.is_empty() {
        return Err(AppError::bad_request(
            "Missing required query parameter: dst",
        ));
    }
    if !is_valid_code(&src) {
        return Err(AppError::bad_request(format!(
            "Invalid station code: {src}"
        )));
    }
    if !is_valid_code(&dst) {
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
        service::Service::get_trains_between(&state, &src, &dst).await?,
    ))
}

fn normalize(code: Option<&str>) -> String {
    code.unwrap_or_default().trim().to_uppercase()
}

fn is_valid_code(code: &str) -> bool {
    code.len() == 4 && code.chars().all(|c| c.is_ascii_alphanumeric())
}

fn code_known(state: &AppState, code: &str) -> bool {
    state
        .datasets
        .stations
        .iter()
        .any(|s| s.code.eq_ignore_ascii_case(code))
        || state.datasets.trains.iter().any(|t| {
            t.name
                .split_whitespace()
                .any(|tok| tok.trim_matches('-').eq_ignore_ascii_case(code))
        })
}
