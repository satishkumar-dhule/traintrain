//! PNR slice.
//!
//! Endpoint: `GET /rail-api/pnr?pnr=<10-digit>`
//! Optional params for CAPTCHA flows: `captcha_session`, `captcha_text`,
//! `captcha_source` (echo the values from a previous 428 response).
//!
//! Live source: Indian Railways `https://www.indianrail.gov.in` - the official
//! PNR enquiry is captcha-gated. The first request establishes a session and
//! raises HTTP 428 carrying a `captchaDraw.png` image; the client answers and
//! retries against `/enquiry/CommonCaptcha`. Responses with `errorMessage:
//! "PNR No. is not valid"` surface as 404; a wrong answer or expired session
//! re-issues the 428 challenge.
//!
//! Success model: `crate::models::PnrResponse`. Failures surface as honest
//! `AppError`s (400 bad PNR / bad captcha, 404 no booking, 502 source down or
//! unexpected shape, 428 captcha challenge).

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::PnrResponse;
use crate::slices::pnr::service::{CaptchaAnswer, Service};
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct PnrParams {
    pnr: Option<String>,
    captcha_session: Option<String>,
    captcha_text: Option<String>,
    captcha_source: Option<String>,
}

async fn pnr_handler(
    State(state): State<AppState>,
    Query(params): Query<PnrParams>,
) -> Result<Json<PnrResponse>, AppError> {
    let pnr = params.pnr.as_deref().unwrap_or("");
    if pnr.len() != 10 || !pnr.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::bad_request("PNR must be a 10-digit number."));
    }

    let captcha = params.captcha_text.as_ref().map(|_| CaptchaAnswer {
        session_id: params.captcha_session.clone().unwrap_or_default(),
        text: params.captcha_text.clone().unwrap_or_default(),
        source: params.captcha_source.clone().unwrap_or_default(),
    });

    Ok(Json(Service::get_status(&state, pnr, captcha).await?))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/pnr", get(pnr_handler))
}
