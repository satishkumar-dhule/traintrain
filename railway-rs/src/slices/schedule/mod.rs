//! Schedule slice.
//!
//! Endpoint: `GET /rail-api/schedule?train=<number>`
//!
//! Primary source: NTES `GetTrainSchedule` (`enquiry.indianrail.gov.in`).
//! Fallback: Railyatri `GET {base}/time-table/{train}` (SSR
//! `__NEXT_DATA__` -> `props.pageProps.trainTimeTable`:
//! `train_number`, `train_name`, `routeGroup[]` (per running day) with
//! `routesummary[]` stops (`station_code`, `station_name`, `sta_min`,
//! `std_min`, `day`); `run_days` is an array like `["MON","TUE",...]`).
//!
//! The winning source is reported in `data_source`. NTES data is fetched
//! first; any NTES failure (unreachable, empty, malformed) falls back to
//! Railyatri so a dead gov endpoint never hides a working public mirror.
//!
//! Success model: `crate::models::ScheduleResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::ScheduleResponse;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct ScheduleQuery {
    train: Option<String>,
}

async fn schedule_handler(
    State(state): State<AppState>,
    Query(params): Query<ScheduleQuery>,
) -> Result<Json<ScheduleResponse>, AppError> {
    let train = params.train.as_deref().unwrap_or("");
    if train.is_empty() || train.len() > 8 || !train.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::bad_request("Train must be a number."));
    }
    let resp = service::Service::get_schedule(&state, train).await?;
    Ok(Json(resp))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/schedule", get(schedule_handler))
}
