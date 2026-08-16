//! Trains-between-stations slice.
//!
//! Endpoint: `GET /rail-api/ntes/trains-between?src=<CODE>&dst=<CODE>`
//!
//! Live source: NTES public web form `TrainsBetweenStation` (see
//! `crate::core::ntes::NtesWebClient::trains_between`): a session + CSRF are
//! bootstrapped from `/mntes/` and the form is submitted to `/mntes/q` with
//! `jFromStationInput=<CODE - NAME>&jToStationInput=<CODE - NAME>`. The HTML
//! table is parsed into the mobile-shape `trainBtwStationList` JSON.
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
use crate::slices::station_codes::{normalize_code, require_station};
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
    let src = normalize_code(q.src.as_deref());
    let dst = normalize_code(q.dst.as_deref());

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
    if src == dst {
        return Err(AppError::bad_request("Source and destination must differ."));
    }
    require_station(&state, Some(&src), "src")?;
    require_station(&state, Some(&dst), "dst")?;

    Ok(Json(
        service::Service::get_trains_between(&state, &src, &dst).await?,
    ))
}
