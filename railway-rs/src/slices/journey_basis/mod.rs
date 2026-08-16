//! Journey Station Basis slice - the second mode of NTES "Spot Your Train".
//!
//! Endpoints:
//! - `GET /rail-api/ntes/journey-stations?train=<NUMBER>` -> the journey
//!   stations NTES offers for `train` (codes, day-change flags, run-days).
//! - `GET /rail-api/ntes/journey-basis?train=<NUMBER>&station=<CODE>&date=<optional>`
//!   -> the live running status of that run seen from `station`.
//!
//! Live source: NTES public web forms `FindStationList` and `ShowRunCStn` (see
//! `crate::core::ntes::NtesWebClient::journey_stations` /
//! `::journey_station_basis`): a session + CSRF are bootstrapped from
//! `/mntes/` and the form is submitted to `/mntes/q` (respectively
//! `/mntes/tr`) with `opt=TrainRunning`. The HTML is parsed into the
//! mobile-shape `{"trainNo","list":[...]}` and `ShowFullRunJson`-shape JSON.
//!
//! Validation: `train` is required - exactly 5 ascii digits and not all-zero,
//! mirroring the other NTES forms. `station` is required for
//! `/journey-basis` and must be a known station code (see
//! `crate::slices::station_codes`).
//!
//! Success models: `crate::models::JourneyStationsResponse`,
//! `crate::models::JourneyBasisResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::{JourneyBasisResponse, JourneyStationsResponse};
use crate::slices::station_codes::require_station;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct JsQuery {
    train: Option<String>,
}

#[derive(Deserialize, Default)]
struct JbQuery {
    train: Option<String>,
    station: Option<String>,
    date: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/rail-api/ntes/journey-stations",
            get(journey_stations_handler),
        )
        .route("/rail-api/ntes/journey-basis", get(journey_basis_handler))
}

async fn journey_stations_handler(
    State(state): State<AppState>,
    Query(q): Query<JsQuery>,
) -> Result<Json<JourneyStationsResponse>, AppError> {
    let train = q.train.map(|t| t.trim().to_string()).unwrap_or_default();
    if !(train.len() == 5 && train.chars().all(|c| c.is_ascii_digit()) && train != "00000") {
        return Err(AppError::bad_request("train must be a 5-digit number"));
    }

    Ok(Json(
        service::Service::get_journey_stations(&state, &train).await?,
    ))
}

async fn journey_basis_handler(
    State(state): State<AppState>,
    Query(q): Query<JbQuery>,
) -> Result<Json<JourneyBasisResponse>, AppError> {
    let train = q.train.map(|t| t.trim().to_string()).unwrap_or_default();
    if !(train.len() == 5 && train.chars().all(|c| c.is_ascii_digit()) && train != "00000") {
        return Err(AppError::bad_request("train must be a 5-digit number"));
    }
    let station = require_station(&state, q.station.as_deref(), "station")?;
    let date = q.date.as_deref().map(str::trim).filter(|d| !d.is_empty());

    Ok(Json(
        service::Service::get_journey_basis(&state, &train, &station, date).await?,
    ))
}
