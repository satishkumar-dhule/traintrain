//! Service layer for the AskDISHA slice.
//!
//! Cache-first per the module contract (`docs/ASKDISHA_MODULE.md`): each
//! endpoint serves from `state.cache` under its documented key/TTL and only
//! calls [`CoroverClient`] on a miss. Source activity is tagged with
//! `corover-api` / `corover-cdn` via `Metrics::record_source_latency`
//! (the same house mechanism every slice uses), so
//! `GET /rail-api/observability` reports real latency samples once traffic
//! flows; failures surface as honest `AppError::SourceUnavailable` (502)
//! carrying the source id.

use std::time::{Duration, Instant};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::json;

use crate::core::corover::{
    CoroverClient, ScheduleResponse, SettingsFlag, StationRow, SOURCE_API, SOURCE_CDN,
};
use crate::core::error::AppError;
use crate::state::AppState;

/// Cache TTLs, keys and limits exactly as fixed by `docs/ASKDISHA_MODULE.md`.
const STATIONS_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const SCHEDULE_TTL: Duration = Duration::from_secs(30 * 60);
const FAQS_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const SETTINGS_TTL: Duration = Duration::from_secs(60 * 60);

/// Maximum station rows per `/stations` response.
const STATIONS_LIMIT: usize = 20;

const SETTINGS_KEY: &str = "askdisha:settings";

/// Slice error: either the module is off (defensive 503 envelope; the router
/// is unmerged when `state.askdisha` is `None`, so this is normally
/// unreachable) or an honest upstream [`AppError`] (400 validation /
/// 502 source-unavailable).
#[derive(Debug)]
pub enum AskError {
    Disabled,
    Upstream(AppError),
}

impl From<AppError> for AskError {
    fn from(e: AppError) -> Self {
        AskError::Upstream(e)
    }
}

impl From<serde_json::Error> for AskError {
    fn from(e: serde_json::Error) -> Self {
        // Cache-value (de)serialization problems are internal, matching the
        // house `AppError: From<serde_json::Error>` mapping.
        AskError::Upstream(AppError::from(e))
    }
}

impl IntoResponse for AskError {
    fn into_response(self) -> Response {
        match self {
            AskError::Disabled => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "enabled": false })),
            )
                .into_response(),
            AskError::Upstream(e) => e.into_response(),
        }
    }
}

/// `^[0-9]{1,5}$` - train numbers are 1 to 5 ASCII digits.
pub fn is_valid_train_no(train_no: &str) -> bool {
    let n = train_no.len();
    (1..=5).contains(&n) && train_no.bytes().all(|b| b.is_ascii_digit())
}

/// FAQ language whitelist (`en` | `hi` | `gu`).
pub fn is_valid_lang(lang: &str) -> bool {
    matches!(lang, "en" | "hi" | "gu")
}

/// Cache keys are `pub(crate)` so the slice tests can seed/inspect slots.
pub(crate) fn stations_key(q: &str) -> String {
    format!("askdisha:stations:{}", q.to_lowercase())
}

/// Absent optional params become empty segments so every distinct upstream
/// query has its own deterministic cache slot.
pub(crate) fn schedule_key(train_no: &str, date: Option<&str>, from: Option<&str>) -> String {
    format!(
        "askdisha:schedule:{}:{}:{}",
        train_no,
        date.unwrap_or_default(),
        from.unwrap_or_default()
    )
}

pub(crate) fn faqs_key(lang: &str) -> String {
    format!("askdisha:faqs:{lang}")
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub enabled: bool,
    pub sources: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct StationsResponse {
    pub source: &'static str,
    pub cached: bool,
    pub count: usize,
    pub stations: Vec<StationRow>,
}

#[derive(Debug, Serialize)]
pub struct ScheduleEnvelope {
    pub source: &'static str,
    pub cached: bool,
    pub schedule: ScheduleResponse,
}

#[derive(Debug, Serialize)]
pub struct FaqsResponse {
    pub source: &'static str,
    pub cached: bool,
    pub faqs: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SettingsEnvelope {
    pub source: &'static str,
    pub cached: bool,
    pub settings: SettingsFlag,
}

pub struct Service;

impl Service {
    /// The gated client, or the defensive [`AskError::Disabled`] guard.
    fn client(state: &AppState) -> Result<&CoroverClient, AskError> {
        state.askdisha.as_deref().ok_or(AskError::Disabled)
    }

    /// Module liveness for `/status` (no upstream call involved).
    pub fn status(state: &AppState) -> Result<StatusResponse, AskError> {
        Self::client(state)?;
        Ok(StatusResponse {
            enabled: true,
            sources: vec![SOURCE_API, SOURCE_CDN],
        })
    }

    /// Typeahead station search. Cache key lowercases the query so `NEW`,
    /// `new` and `New` share one slot; responses are capped at 20 rows.
    pub async fn stations(state: &AppState, q: &str) -> Result<StationsResponse, AskError> {
        let key = stations_key(q);
        if let Some(v) = state.cache.get(&key) {
            if let Ok(rows) = serde_json::from_value::<Vec<StationRow>>(v) {
                return Ok(stations_envelope(true, rows));
            }
        }

        let started = Instant::now();
        let rows = Self::client(state)?.search_station(q).await?;
        state
            .metrics
            .record_source_latency(SOURCE_API, started.elapsed());
        let resp = stations_envelope(false, rows);
        state
            .cache
            .set_with_ttl(&key, serde_json::to_value(&resp.stations)?, STATIONS_TTL);
        Ok(resp)
    }

    /// Train schedule enquiry. `date`/`from` pass through as given (omitted
    /// upstream when absent); the cache key mirrors them positionally.
    pub async fn schedule(
        state: &AppState,
        train_no: &str,
        date: Option<&str>,
        from: Option<&str>,
    ) -> Result<ScheduleEnvelope, AskError> {
        let key = schedule_key(train_no, date, from);
        if let Some(v) = state.cache.get(&key) {
            if let Ok(schedule) = serde_json::from_value::<ScheduleResponse>(v) {
                return Ok(ScheduleEnvelope {
                    source: SOURCE_API,
                    cached: true,
                    schedule,
                });
            }
        }

        let started = Instant::now();
        let schedule = Self::client(state)?
            .trnschedule_enq(train_no, date, from)
            .await?;
        state
            .metrics
            .record_source_latency(SOURCE_API, started.elapsed());
        state
            .cache
            .set_with_ttl(&key, serde_json::to_value(&schedule)?, SCHEDULE_TTL);
        Ok(ScheduleEnvelope {
            source: SOURCE_API,
            cached: false,
            schedule,
        })
    }

    /// CDN FAQ strings for one language.
    pub async fn faqs(state: &AppState, lang: &str) -> Result<FaqsResponse, AskError> {
        let key = faqs_key(lang);
        if let Some(v) = state.cache.get(&key) {
            if let Ok(faqs) = serde_json::from_value::<Vec<String>>(v) {
                return Ok(FaqsResponse {
                    source: SOURCE_CDN,
                    cached: true,
                    faqs,
                });
            }
        }

        let started = Instant::now();
        let faqs = Self::client(state)?.fetch_faqs(lang).await?;
        state
            .metrics
            .record_source_latency(SOURCE_CDN, started.elapsed());
        state
            .cache
            .set_with_ttl(&key, serde_json::to_value(&faqs)?, FAQS_TTL);
        Ok(FaqsResponse {
            source: SOURCE_CDN,
            cached: false,
            faqs,
        })
    }

    /// CDN feature-flag document.
    pub async fn settings(state: &AppState) -> Result<SettingsEnvelope, AskError> {
        if let Some(v) = state.cache.get(SETTINGS_KEY) {
            if let Ok(settings) = serde_json::from_value::<SettingsFlag>(v) {
                return Ok(SettingsEnvelope {
                    source: SOURCE_CDN,
                    cached: true,
                    settings,
                });
            }
        }

        let started = Instant::now();
        let settings = Self::client(state)?.fetch_settings().await?;
        state
            .metrics
            .record_source_latency(SOURCE_CDN, started.elapsed());
        state
            .cache
            .set_with_ttl(SETTINGS_KEY, serde_json::to_value(&settings)?, SETTINGS_TTL);
        Ok(SettingsEnvelope {
            source: SOURCE_CDN,
            cached: false,
            settings,
        })
    }
}

fn stations_envelope(cached: bool, rows: Vec<StationRow>) -> StationsResponse {
    let mut stations = rows;
    stations.truncate(STATIONS_LIMIT);
    StationsResponse {
        source: SOURCE_API,
        cached,
        count: stations.len(),
        stations,
    }
}
