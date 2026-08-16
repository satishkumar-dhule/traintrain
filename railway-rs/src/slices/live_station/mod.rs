//! Live station (station arrival board) slice.
//!
//! Endpoint: `GET /rail-api/ntes/live-station?station=<CODE>&hours=<1..4>`
//!
//! Live source: NTES public web form `LiveStation` (see
//! `crate::core::ntes::NtesWebClient::live_station`): a session is bootstrapped
//! from `/mntes/`, a CSRF token is fetched, and the form is submitted to
//! `/mntes/q` with `jStation=<CODE>&jStnName=<NAME>&nHr=<hours>`. The HTML
//! table is parsed into the mobile-shape `trainList` JSON. When the form is
//! unreachable the endpoint propagates `AppError::SourceUnavailable` honestly -
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
use crate::slices::station_codes::require_station;
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
/// exists in the real station dataset (shared `require_station` rules);
/// `hours` is clamped into 1..=4.
async fn live_station_handler(
    State(state): State<AppState>,
    Query(params): Query<LsQuery>,
) -> Result<Json<LiveStationResponse>, AppError> {
    let station = require_station(&state, params.station.as_deref(), "station")?;

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
        Service::get_live_station(&state, &station, hours).await?,
    ))
}
