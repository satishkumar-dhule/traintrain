//! Exceptional trains (cancelled / rescheduled / diverted) slice.
//!
//! Endpoint: `GET /rail-api/ntes/exceptional?train=04138[&type=cancelled]`
//!
//! Live source: NTES web form `q?opt=TrainRunning&subOpt=excpInfo`
//! (CSRF-protected). The old batch form `q?opt=ExcpTrains&subOpt=show` is
//! disabled server-side ("Requested service in un-available at the moment"),
//! so the endpoint checks one train at a time and caches each train's
//! exception calendar for 2 hours. When the source is blocked from the
//! sandbox, `AppError::SourceUnavailable` is propagated honestly - the
//! exception list is never fabricated.
//!
//! Success model: `crate::models::ExceptionalResponse` (`exceptions[].date` is
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
    train: Option<String>,
    r#type: Option<String>,
}

async fn exceptional_handler(
    State(state): State<AppState>,
    Query(params): Query<ExceptionalQuery>,
) -> Result<Json<ExceptionalResponse>, AppError> {
    let train = params.train.as_deref().unwrap_or("");
    if train.is_empty()
        || !train.chars().all(|c| c.is_ascii_digit())
        || !(4..=5).contains(&train.len())
    {
        return Err(AppError::bad_request(
            "train is required (4-5 digit train number)",
        ));
    }
    let kind = params.r#type.as_deref();
    if let Some(kind) = kind {
        if !matches!(kind, "cancelled" | "rescheduled" | "diverted") {
            return Err(AppError::bad_request(
                "type must be one of: cancelled, rescheduled, diverted",
            ));
        }
    }
    Ok(Json(Service::get_exceptional(&state, train, kind).await?))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/exceptional", get(exceptional_handler))
}
