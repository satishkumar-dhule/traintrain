//! Stations slice.
//!
//! Endpoint: `GET /rail-api/stations?q=<query>` -> JSON array of
//! `crate::models::Station`.
//!
//! Data: real local `data/stations.json` via `state.datasets.stations`
//! (`crate::data::filter_stations`). No network, no fabrication. Empty query
//! or no matches returns an empty array (200).

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

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

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/stations", get(stations_handler))
}
