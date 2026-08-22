//! AI insight slice: grounded, single-shot explanations of real rail data.
//! Phase B implements the endpoint; the router is intentionally empty until
//! then so nothing half-baked is exposed.

pub mod service;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}
