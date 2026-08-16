//! Train on Map slice - the NTES "Train on Map" route + live spot views.
//!
//! Endpoint: `GET /rail-api/ntes/train-on-map?train=<NUMBER>&station=<CODE|optional>&date=<optional>`
//!
//! Live source: NTES public web forms `TrnMap` route/spot (see
//! `crate::core::ntes::NtesWebClient::train_route_map` / `::train_spot_map`):
//! a session + CSRF are bootstrapped from `/mntes/` and the form is submitted
//! to `/mntes/TrnMap` with `opt=map&subOpt=route|spot`. The JavaScript
//! variable blocks (`myStns`, `myStnsF`, `runInfo`, `cStn`, `jStn`, ...) are
//! parsed into the route-map / spot-map JSON.
//!
//! Validation: `train` is required - exactly 5 ascii digits and not all-zero,
//! mirroring the other NTES forms. `station` is optional; when present it must
//! be a valid station code (see `crate::slices::station_codes`).
//!
//! Success model: `crate::models::TrainOnMapResponse`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::TrainOnMapResponse;
use crate::slices::station_codes::is_valid_code;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct TomQuery {
    train: Option<String>,
    station: Option<String>,
    date: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/train-on-map", get(train_on_map_handler))
}

async fn train_on_map_handler(
    State(state): State<AppState>,
    Query(q): Query<TomQuery>,
) -> Result<Json<TrainOnMapResponse>, AppError> {
    let train = q.train.map(|t| t.trim().to_string()).unwrap_or_default();
    if !(train.len() == 5 && train.chars().all(|c| c.is_ascii_digit()) && train != "00000") {
        return Err(AppError::bad_request("train must be a 5-digit number"));
    }
    let station = q
        .station
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_uppercase);
    if let Some(code) = &station {
        if !is_valid_code(code) {
            return Err(AppError::bad_request("invalid station code"));
        }
    }
    let date = q.date.as_deref().map(str::trim).filter(|d| !d.is_empty());

    Ok(Json(
        service::Service::get_train_on_map(&state, &train, station.as_deref(), date).await?,
    ))
}
