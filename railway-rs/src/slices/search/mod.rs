//! Search slice.
//!
//! Endpoints (all offline, backed by the pre-warmed local datasets):
//! - `GET /rail-api/search/trains?q=<query>`   -> JSON array of `TrainLite`
//! - `GET /rail-api/search/stations?q=<query>` -> JSON array of `StationLite`
//! - `GET /rail-api/search/suggest?q=<query>`  -> JSON array of `Suggestion`
//!   (combined stations + trains for one-round-trip IntelliSense autocomplete)
//!
//! Trains: real local `data/trains.json` (`state.datasets.trains`) via
//! `Datasets::search_trains`; stations: real `data/stations.json` via
//! `Datasets::search_stations`. Both lists are pre-warmed into lowercase
//! indexes at startup. Multi-word queries like `q=MUMBAI RAJDHANI` match whole
//! names and rank all-token hits first. Empty query or no matches -> empty
//! array.
//!
//! Note: `GET /rail-api/trains?q=` is NOT part of this slice; train search
//! lives here only.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::models::{StationLite, Suggestion, TrainLite};
use crate::state::AppState;

pub mod service;

const SEARCH_LIMIT: usize = 10;
const MAX_Q_LEN: usize = 128;

#[derive(Deserialize, Default)]
struct SearchQuery {
    q: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/search/trains", get(search_trains))
        .route("/rail-api/search/stations", get(search_stations))
        .route("/rail-api/search/suggest", get(search_suggest))
}

fn clamp_q(q: Option<&str>) -> String {
    q.unwrap_or("").chars().take(MAX_Q_LEN).collect::<String>()
}

/// Real train search over the pre-warmed NTES master list, capped at 10 hits.
async fn search_trains(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<TrainLite>> {
    let query = clamp_q(q.q.as_deref());
    Json(service::Service::search_trains(
        &state,
        &query,
        SEARCH_LIMIT,
    ))
}

/// Real station search over the pre-warmed station dataset, capped at 10 hits.
async fn search_stations(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<StationLite>> {
    let query = clamp_q(q.q.as_deref());
    Json(service::Service::search_stations(
        &state,
        &query,
        SEARCH_LIMIT,
    ))
}

/// Combined station + train autocomplete, capped at 10 hits.
async fn search_suggest(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<Suggestion>> {
    let query = clamp_q(q.q.as_deref());
    Json(service::Service::suggest(&state, &query, SEARCH_LIMIT))
}
