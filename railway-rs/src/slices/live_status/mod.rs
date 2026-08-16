//! Live train status slice ("Spot Train").
//!
//! Endpoint: `GET /rail-api/live-status?train=<number>&date=<YYYY-MM-DD optional>`
//!
//! The frontend never asks for a date - like the NTES "Spot Train (Live
//! Status)" page it only needs the train number. The backend resolves the
//! running instance's `startDate` via `GetTrainInstance` and reports every
//! run date NTES returns in `instances`, plus the active run's position.
//!
//! Primary source: NTES `ShowFullRunJson` (`enquiry.indianrail.gov.in`),
//! following the proven mobile-protocol flow: resolve the running instance's
//! `startDate` (`DD-MMM-YYYY`) via `GetTrainInstance`
//! (`vInstanceList[].startDate`) and query that exact run.
//! Fallback: Railyatri `GET {base}/live-train-status/{train}` (verified
//! working) - parse `__NEXT_DATA__`:
//! - `props.pageProps.ltsData`: `train_number`, `train_name`,
//!   `train_start_date`, `next_station_code`/`next_station_name`, `title`,
//!   `new_message`, `source_stn_name`, `dest_stn_name`, `at_src`, `at_dstn`,
//!   `at_src_dstn`, `spent_time`, `platform_number`.
//! - `props.pageProps.timeTableData[0].route[]`: scheduled stops (`sta_min`,
//!   `std_min`, `station_code`, `station_name`, `day`).
//!
//! The Railyatri SSR payload does NOT contain per-stop actual arrival times,
//! so derive honest statuses from the real `next_station_code`: stops before
//! it "departed", the next station "expected", the rest "scheduled". Never
//! invent arrival times. NTES `ShowFullRunJson` does provide real per-stop
//! `actualArrival` values, which are surfaced verbatim (and drive honest
//! `delay_minutes` when the delta is unambiguous). A run date that is neither
//! today (IST) nor the train's `train_start_date` is rejected on every source
//! path. `data_source` reports whichever source actually served the data.
//!
//! Success model: `crate::models::LiveStatusResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::LiveStatusResponse;
use crate::state::AppState;

pub mod mapping;
pub mod service;

#[derive(Deserialize, Default)]
struct LiveQuery {
    train: Option<String>,
    date: Option<String>,
}

/// Router for the live-status endpoint.
pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/live-status", get(live_status_handler))
}

async fn live_status_handler(
    State(state): State<AppState>,
    Query(q): Query<LiveQuery>,
) -> Result<Json<LiveStatusResponse>, AppError> {
    let Some(train) = q.train.filter(|t| !t.is_empty()) else {
        return Err(AppError::bad_request("train query parameter is required"));
    };
    if !is_valid_train_number(&train) {
        return Err(AppError::bad_request("train must be a 1-8 digit number"));
    }
    let date = q.date.unwrap_or_default();
    let resp = service::Service::get_live_status(&state, &train, &date).await?;
    Ok(Json(resp))
}

fn is_valid_train_number(train: &str) -> bool {
    train.len() <= 8 && train.chars().all(|c| c.is_ascii_digit())
}
