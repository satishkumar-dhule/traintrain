//! Exceptional trains (cancelled / rescheduled / diverted) slice.
//!
//! Endpoint: `GET /rail-api/ntes/exceptional?type=cancelled|rescheduled|diverted`
//!
//! Live source: NTES web forms (`q?opt=ExcpTrains&subOpt=show`, CSRF-protected)
//! and the mobile `TrainExcpInfo` (per-train). Both are blocked from the
//! sandbox - propagate `AppError::SourceUnavailable` honestly, never fabricate
//! the exception list.
//!
//! Success model: `crate::models::ExceptionalResponse` (`trains[].date` is
//! `YYYY-MM-DD`).

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::ExceptionalResponse;
use crate::slices::exceptional::service::Service;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct ExceptionalQuery {
    r#type: Option<String>,
}

async fn exceptional_handler(
    State(state): State<AppState>,
    Query(params): Query<ExceptionalQuery>,
) -> Result<Json<ExceptionalResponse>, AppError> {
    let kind = params.r#type.as_deref().unwrap_or("");
    if !matches!(kind, "cancelled" | "rescheduled" | "diverted") {
        return Err(AppError::bad_request(
            "type must be one of: cancelled, rescheduled, diverted",
        ));
    }
    Ok(Json(Service::get_exceptional(&state, kind).await?))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/exceptional", get(exceptional_handler))
}
