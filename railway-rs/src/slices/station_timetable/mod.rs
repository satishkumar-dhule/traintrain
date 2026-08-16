//! Station timetable slice.
//!
//! Endpoint: `GET /rail-api/ntes/station-timetable?station=<CODE>&date=<DD-MMM-YYYY|optional>`
//!
//! Live source: NTES public web form `TrainsAtStation` (see
//! `crate::core::ntes::NtesWebClient::station_timetable`): a session is
//! bootstrapped from `/mntes/`, a CSRF token is fetched, and the form is
//! submitted to `/mntes/q` with `jFromStationInput=<CODE - NAME>` and
//! `trainStartDate=<date|No Specific Date>`. The HTML table is parsed into the
//! mobile-shape `{station, stationName, date, total, list}` JSON.
//!
//! Validation: `station` is required and must be a known 4-char alphanumeric
//! station code (see `crate::slices::station_codes::require_station`). `date`
//! is optional, trimmed and passed through to NTES as-is (any `DD-MMM-YYYY`
//! is accepted upstream).
//!
//! Success model: `crate::models::StationTimetableResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::StationTimetableResponse;
use crate::slices::station_codes::require_station;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct SttQuery {
    station: Option<String>,
    date: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/rail-api/ntes/station-timetable",
        get(station_timetable_handler),
    )
}

async fn station_timetable_handler(
    State(state): State<AppState>,
    Query(q): Query<SttQuery>,
) -> Result<Json<StationTimetableResponse>, AppError> {
    let station = require_station(&state, q.station.as_deref(), "station")?;
    let station_name = state
        .datasets
        .station_name(&station)
        .unwrap_or(&station)
        .to_string();
    let date = q
        .date
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_string);

    Ok(Json(
        service::Service::get_station_timetable(&state, &station, &station_name, date).await?,
    ))
}
