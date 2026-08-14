//! Search slice.
//!
//! Endpoints:
//! - `GET /rail-api/search/trains?q=<query>`  -> JSON array of `TrainLite`
//! - `GET /rail-api/search/stations?q=<query>` -> JSON array of `StationLite`
//!
//! Trains: real local `data/trains.json` (`state.datasets.trains`) via
//! `crate::data::filter_trains`. Stations: real `data/stations.json` via
//! `crate::data::filter_stations`. Multi-word queries match any whitespace
//! token, so `q=MUMBAI RAJDHANI` returns real RAJDHANI trains. Empty query
//! or no matches -> empty array.
//!
//! Note: `GET /rail-api/trains?q=` is NOT part of this slice; train search
//! lives here only.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::models::{StationLite, TrainLite};
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
}

/// Real train search over the local NTES master list, capped at 10 hits.
async fn search_trains(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<TrainLite>> {
    let query =
        q.q.as_deref()
            .unwrap_or("")
            .chars()
            .take(MAX_Q_LEN)
            .collect::<String>();
    let mut hits: Vec<TrainLite> = Vec::new();
    for token in query.split_whitespace() {
        for t in service::Service::search_trains(&state, token, SEARCH_LIMIT) {
            if !hits.iter().any(|h| h.number == t.number) {
                hits.push(t);
            }
        }
    }
    hits.truncate(SEARCH_LIMIT);
    Json(hits)
}

/// Real station search over the local station dataset, capped at 10 hits.
async fn search_stations(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<StationLite>> {
    let query =
        q.q.as_deref()
            .unwrap_or("")
            .chars()
            .take(MAX_Q_LEN)
            .collect::<String>();
    let mut hits: Vec<StationLite> = Vec::new();
    for token in query.split_whitespace() {
        for s in service::Service::search_stations(&state, token, SEARCH_LIMIT) {
            if !hits.iter().any(|h| h.code == s.code) {
                hits.push(s);
            }
        }
    }
    hits.truncate(SEARCH_LIMIT);
    Json(hits)
}
