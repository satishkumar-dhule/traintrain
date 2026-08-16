//! Train-availability slice (no-login IRCTC).
//!
//! Endpoint: `GET /rail-api/irctc/availability?src=<CODE>&dst=<CODE>&date=<DATE>`
//!
//! Live source: IRCTC's no-login mobile API `altAvlEnq/TC`
//! (`POST /eticketing/protected/mapps1/altAvlEnq/TC`, see
//! `crate::core::irctc::IrctcClient::availability`). It lists direct trains
//! between the two stations on the journey date, with class availability,
//! running days and times. `date` is optional and defaults to today (IST);
//! accepted formats are `YYYY-MM-DD`, `DD-MM-YYYY`, `DD/MM/YYYY` and
//! `YYYYMMDD`.
//!
//! Validation mirrors the trains-between slice: `src`/`dst` are required
//! 4-char station codes known to the local dataset (or embedded as a token in
//! an official train name), and `src == dst` is rejected.
//!
//! Note: IRCTC is Akamai-protected and IP-geofenced to India; from a
//! datacenter IP the upstream answers HTTP 403 and the slice reports an
//! honest `AppError::SourceUnavailable`.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::AvailabilityResponse;
use crate::slices::station_codes::require_station;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct AvlQuery {
    src: Option<String>,
    dst: Option<String>,
    date: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/irctc/availability", get(availability_handler))
}

async fn availability_handler(
    State(state): State<AppState>,
    Query(q): Query<AvlQuery>,
) -> Result<Json<AvailabilityResponse>, AppError> {
    let src = require_station(&state, q.src.as_deref(), "src")?;
    let dst = require_station(&state, q.dst.as_deref(), "dst")?;

    if src == dst {
        return Err(AppError::bad_request("Source and destination must differ."));
    }

    let date = match q.date.as_deref() {
        Some(raw) if !raw.trim().is_empty() => {
            let raw = raw.trim();
            if !is_valid_date(raw) {
                return Err(AppError::bad_request(format!(
                    "Invalid date: {raw}. Use YYYY-MM-DD, DD-MM-YYYY or YYYYMMDD."
                )));
            }
            crate::core::irctc::normalize::date_iso(raw)
        }
        _ => today_ist(),
    };

    Ok(Json(
        service::Service::get_availability(&state, &src, &dst, &date).await?,
    ))
}

fn is_valid_date(date: &str) -> bool {
    ["%Y-%m-%d", "%Y%m%d", "%d-%m-%Y", "%d/%m/%Y"]
        .iter()
        .any(|fmt| chrono::NaiveDate::parse_from_str(date.trim(), fmt).is_ok())
}

fn today_ist() -> String {
    let offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    chrono::Utc::now()
        .with_timezone(&offset)
        .date_naive()
        .to_string()
}
