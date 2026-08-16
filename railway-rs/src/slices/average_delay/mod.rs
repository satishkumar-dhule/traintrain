//! Average-delay slice.
//!
//! Endpoint: `GET /rail-api/ntes/average-delay?train=<NUMBER>`
//!
//! Live source: NTES public web form `AverageDelay` (see
//! `crate::core::ntes::NtesWebClient::average_delay`): a session + CSRF are
//! bootstrapped from `/mntes/` and the form is submitted to `/mntes/q` with
//! `opt=AverageDelay&subOpt=show&trainNo=<NUMBER>`. The HTML table is parsed
//! into the mobile-shape `{"trainNo","trainName","daysOfRun","trainType",
//! "list":[...]}` JSON.
//!
//! Validation: `train` is required. NTES's own form requires exactly 5
//! digits (not all-zero); anything else is a client error.
//!
//! Success model: `crate::models::AverageDelayResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::AverageDelayResponse;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct AdQuery {
    train: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/average-delay", get(average_delay_handler))
}

async fn average_delay_handler(
    State(state): State<AppState>,
    Query(q): Query<AdQuery>,
) -> Result<Json<AverageDelayResponse>, AppError> {
    let train = q.train.map(|t| t.trim().to_string()).unwrap_or_default();
    if !(train.len() == 5 && train.chars().all(|c| c.is_ascii_digit()) && train != "00000") {
        return Err(AppError::bad_request("train must be a 5-digit number"));
    }

    Ok(Json(
        service::Service::get_average_delay(&state, &train).await?,
    ))
}
