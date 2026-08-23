//! Live station (station arrival board) slice.
//!
//! Endpoint: `GET /rail-api/ntes/live-station?station=<CODE>&hours=<2|4|8>
//! &[destination=<CODE>]`
//!
//! Live source: NTES public web form `LiveStation` (see
//! `crate::core::ntes::NtesWebClient::live_station`): a session is bootstrapped
//! from `/mntes/`, a CSRF token is fetched, and the form is submitted to
//! `/mntes/q` with `jStation=<CODE>&jStnName=<NAME>&nHr=<hours>`. The real
//! form also carries an optional "Going to station" input (`jToStationInput`,
//! filled as `CODE - NAME`) that filters the board upstream; `destination`
//! mirrors it, with the same browser-side rule that the two stations must
//! differ. The HTML table is parsed into the mobile-shape `trainList` JSON.
//! When the form is unreachable the endpoint propagates
//! `AppError::SourceUnavailable` honestly - never fabricate trains.
//!
//! Success model: `crate::models::LiveStationResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::LiveStationResponse;
use crate::slices::live_station::service::Service;
use crate::slices::station_codes::require_station;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct LsQuery {
    station: Option<String>,
    destination: Option<String>,
    hours: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/live-station", get(live_station_handler))
}

/// Trains expected at a station. `station` must be a 2-4 character code that
/// exists in the real station dataset (shared `require_station` rules);
/// `destination`, when present, goes through the same rules and must differ
/// from `station`; `hours` is clamped into 1..=4.
async fn live_station_handler(
    State(state): State<AppState>,
    Query(params): Query<LsQuery>,
) -> Result<Json<LiveStationResponse>, AppError> {
    let station = require_station(&state, params.station.as_deref(), "station")?;

    let destination = match params.destination.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(raw) => {
            let dest = require_station(&state, Some(raw), "destination")?;
            if dest.eq_ignore_ascii_case(&station) {
                return Err(AppError::bad_request(
                    "Destination must differ from the board station.",
                ));
            }
            Some(dest)
        }
    };

    let hours = params
        .hours
        .as_deref()
        .and_then(|h| h.parse::<u32>().ok())
        .unwrap_or(2);
    if !matches!(hours, 2 | 4 | 8) {
        return Err(AppError::bad_request(
            "Live station window must be 2, 4, or 8 hours.",
        ));
    }

    Ok(Json(
        Service::get_live_station(&state, &station, hours, destination.as_deref()).await?,
    ))
}
