//! Parcel special trains slice.
//!
//! Endpoint: `GET /rail-api/ntes/parcel` (no query parameters; any provided are
//! ignored).
//!
//! Live source: NTES public web form `TrainRunning`/`splTrnDtl` (see
//! `crate::core::ntes::NtesWebClient::parcel_special_trains`): a session + CSRF
//! are bootstrapped from `/mntes/` and the form is POSTed to `/mntes/q`. The
//! HTML list of all currently running parcel special trains is parsed into a
//! `{"list":[{trainNo,trainName,...}]}` JSON shape.
//!
//! Success model: `crate::models::ParcelResponse`; each `ParcelTrain.number` is
//! the 5-digit train number with leading zeros (e.g. `"00111"`).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::core::error::AppError;
use crate::models::ParcelResponse;
use crate::slices::parcel::service::Service;
use crate::state::AppState;

pub mod service;

async fn parcel_handler(State(state): State<AppState>) -> Result<Json<ParcelResponse>, AppError> {
    Ok(Json(Service::get_parcel(&state).await?))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/parcel", get(parcel_handler))
}
