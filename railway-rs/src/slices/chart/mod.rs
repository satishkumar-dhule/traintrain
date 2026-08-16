//! Prepared-chart slice (no-login IRCTC).
//!
//! Endpoint: `GET /rail-api/irctc/chart?train=<number>&date=<DATE>&station=<CODE>`
//!
//! Live source: IRCTC's no-login online-charts API `trainComposition`
//! (`POST /online-charts/api/trainComposition`, see
//! `crate::core::irctc::IrctcClient::train_composition`). It returns the
//! per-coach berth status of a train's prepared chart for a journey date and
//! boarding station. `date` defaults to today (IST); accepted formats are
//! `YYYY-MM-DD`, `DD-MM-YYYY`, `DD/MM/YYYY` and `YYYYMMDD`. `station` is the
//! boarding-station code and is optional - an empty value is forwarded
//! verbatim and rejected by IRCTC if it requires one.
//!
//! Chart data is only published a few hours before departure; before that the
//! upstream may report no chart, which surfaces as an honest
//! `AppError::SourceUnavailable`.
//!
//! The exact response envelope is undocumented (reconstructed from the
//! online-charts UI); the normalizer in `crate::core::irctc::normalize` is
//! tolerant and fails honestly on unrecognized shapes.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::models::ChartResponse;
use crate::state::AppState;

pub mod service;

#[derive(Deserialize, Default)]
struct ChartQuery {
    train: Option<String>,
    date: Option<String>,
    station: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/irctc/chart", get(chart_handler))
}

async fn chart_handler(
    State(state): State<AppState>,
    Query(q): Query<ChartQuery>,
) -> Result<Json<ChartResponse>, AppError> {
    let train = q.train.as_deref().unwrap_or("").trim().to_string();
    if train.is_empty() || train.len() > 8 || !train.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::bad_request("Train must be a number."));
    }

    let date = match q.date.as_deref() {
        Some(raw) if !raw.trim().is_empty() => {
            let date = raw.trim().to_string();
            if !is_valid_date(&date) {
                return Err(AppError::bad_request(format!(
                    "Invalid date: {date}. Use YYYY-MM-DD, DD-MM-YYYY or YYYYMMDD."
                )));
            }
            date
        }
        _ => today_ist(),
    };

    let station = q
        .station
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_uppercase()
        .to_string();

    Ok(Json(
        service::Service::get_chart(&state, &train, &date, &station).await?,
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
