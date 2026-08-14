//! Live station (station arrival board) slice.
//!
//! Endpoint: `GET /rail-api/ntes/live-station?station=<CODE>&hours=<1..4>`
//!
//! Live source: NTES mobile API `TrainsAtStationJson` (see
//! `crate::core::ntes::NtesClient::station_live`) with payload
//! `jStation=<CODE>&nHr=<hours>&jToStation=`. The endpoint returns an empty
//! body from the sandbox - propagate `AppError::SourceUnavailable` honestly,
//! never fabricate trains.
//!
//! Success model: `crate::models::LiveStationResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::LiveStationResponse;
use crate::slices::live_station::service::Service;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct LsQuery {
    station: Option<String>,
    hours: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/live-station", get(live_station_handler))
}

/// Trains expected at a station. `station` must be a 4-character code that
/// exists in the real station dataset; `hours` is clamped into 1..=4.
async fn live_station_handler(
    State(state): State<AppState>,
    Query(params): Query<LsQuery>,
) -> Result<Json<LiveStationResponse>, AppError> {
    let station = params
        .station
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_uppercase();
    if station.len() != 4 || !station.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::bad_request(
            "Station code must be a 4-character code.",
        ));
    }
    let known = state
        .datasets
        .stations
        .iter()
        .any(|s| s.code.eq_ignore_ascii_case(&station));
    if !known {
        return Err(AppError::bad_request(format!(
            "Station {station} not found."
        )));
    }

    let hours = params
        .hours
        .as_deref()
        .and_then(|h| h.parse::<u32>().ok())
        .unwrap_or(2)
        .clamp(1, 4);

    Ok(Json(
        Service::get_live_station(&state, &station, hours).await?,
    ))
}
