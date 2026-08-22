//! Stations slice.
//!
//! Endpoints:
//! - `GET /rail-api/stations?q=<query>`      -> JSON array of
//!   `crate::models::Station`
//! - `GET /rail-api/stations/:code`          -> single `crate::models::Station`
//!   or 404 (`{"error":"Station <CODE> not found."}`)
//!
//! Both carry the hydrated AskDISHA optionals (F2 passthrough from
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

const MAX_Q_LEN: usize = 128;

#[derive(Deserialize, Default)]
struct StationQuery {
    q: Option<String>,
}

async fn stations_handler(
    State(state): State<AppState>,
    Query(params): Query<StationQuery>,
) -> Json<Vec<Station>> {
    let query = params
        .q
        .as_deref()
        .unwrap_or("")
        .chars()
        .take(MAX_Q_LEN)
        .collect::<String>();
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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/stations", get(stations_handler))
        .route("/rail-api/stations/:code", get(station_by_code_handler))
}
