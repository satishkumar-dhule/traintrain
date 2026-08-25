//! Stations slice.
//!
//! Endpoints:
//! - `GET /rail-api/stations?q=<query>`      -> JSON array of
//!   `crate::models::Station`
//! - `GET /rail-api/stations/:code`          -> single `crate::models::Station`
//!   or 404 (`{"error":"Station <CODE> not found."}`)
//! - `GET /rail-api/nearby/stations?lat=&lng=&limit=` -> distance-sorted
//!   [`NearbyResponse`] computed locally with haversine over the hydrated
//!   coordinates in `data/stations.json`
//!
//! The nearby path deliberately lives outside `/stations/*`: axum 0.7
//! (matchit 0.7) rejects static segments that collide with the `:code`
//! parameter at router-build time.
//!
//! All carry the hydrated AskDISHA optionals (F2 passthrough from
//! `StationRecord`); absent keys are omitted on the wire.
//!
//! Data: real local `data/stations.json` via `state.datasets.stations`
//! (`crate::data::filter_stations`). No network, no fabrication. Empty query
//! or no matches returns an empty array (200).

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::Station;
use crate::state::AppState;

pub mod service;

pub use service::{NearbyResponse, NearbyStation};

#[derive(Deserialize, Default)]
struct StationQuery {
    q: Option<String>,
}

async fn stations_handler(
    State(state): State<AppState>,
    Query(params): Query<StationQuery>,
) -> Json<Vec<Station>> {
    let query = crate::core::validate::clamp_q(params.q.as_deref());
    Json(crate::slices::stations::service::Service::search(
        &state, &query, 20,
    ))
}

/// F2 single-station lookup used by the station board page for its optional
/// subtitle/meta enrichment; unknown codes are an honest 404.
async fn station_by_code_handler(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<Station>, AppError> {
    crate::slices::stations::service::Service::by_code(&state, &code)
        .map(Json)
        .ok_or_else(|| {
            AppError::not_found(format!("Station {} not found.", code.trim().to_uppercase()))
        })
}

#[derive(Deserialize, Default)]
struct NearbyQuery {
    lat: Option<f64>,
    lng: Option<f64>,
    limit: Option<usize>,
}

/// `GET /rail-api/nearby/stations` - nearest stations to a coordinate pair,
/// straight from the local dataset (no upstream, no feature flag). Bad or
/// missing coordinates are a 400 so clients see an honest failure.
async fn nearby_stations_handler(
    State(state): State<AppState>,
    Query(params): Query<NearbyQuery>,
) -> Result<Json<NearbyResponse>, AppError> {
    let lat = params
        .lat
        .ok_or_else(|| AppError::bad_request("Missing lat."))?;
    let lng = params
        .lng
        .ok_or_else(|| AppError::bad_request("Missing lng."))?;
    service::validate_coords(lat, lng)?;
    Ok(Json(service::Service::nearby(
        &state,
        lat,
        lng,
        params.limit.unwrap_or(service::DEFAULT_NEARBY_LIMIT),
    )))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/stations", get(stations_handler))
        .route("/rail-api/stations/:code", get(station_by_code_handler))
        .route("/rail-api/nearby/stations", get(nearby_stations_handler))
}
