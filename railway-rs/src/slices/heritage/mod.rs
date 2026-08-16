//! Heritage trains slice.
//!
//! Endpoint: `GET /rail-api/ntes/heritage?selection=<N>`
//!
//! Live source: NTES public web form `HeritageTrainsBetweenStation` (see
//! `crate::core::ntes::NtesWebClient::heritage_trains`): the form is submitted
//! with `heritageStn = 0..5` and the HTML table is parsed into the
//! mobile-shape `{selection, total, list}` JSON.
//!
//! Success model: `crate::models::HeritageResponse`.
//!
//! Validation: `selection` is optional (defaults to 0 = all) and must parse to
//! a `u8` in `0..=5`; anything else is a 400.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::HeritageResponse;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct HeritageQuery {
    selection: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/heritage", get(heritage_handler))
}

async fn heritage_handler(
    State(state): State<AppState>,
    Query(q): Query<HeritageQuery>,
) -> Result<Json<HeritageResponse>, AppError> {
    let selection = match q.selection.as_deref() {
        None => 0,
        Some(s) => match s.parse::<u8>() {
            Ok(n) if n <= 5 => n,
            _ => return Err(AppError::bad_request("selection must be between 0 and 5")),
        },
    };
    Ok(Json(
        service::Service::get_heritage(&state, selection).await?,
    ))
}
