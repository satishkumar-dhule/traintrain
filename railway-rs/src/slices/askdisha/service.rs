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
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::core::corover::{
    CoroverClient, NearbyStation, PinLookup, SettingsFlag, SOURCE_API, SOURCE_CDN,
};
use crate::core::error::AppError;
use crate::state::AppState;

/// Cache TTLs and limits exactly as fixed by `docs/ASKDISHA_MODULE.md`.
const NEARBY_TTL: Duration = Duration::from_secs(30 * 60);
const PIN_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const FAQS_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const SETTINGS_TTL: Duration = Duration::from_secs(60 * 60);

/// Maximum station rows per `/nearby` response (nearest first).
const NEARBY_LIMIT: usize = 50;

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

/// FAQ language whitelist (`en` | `hi` | `gu`).
pub fn is_valid_lang(lang: &str) -> bool {
    matches!(lang, "en" | "hi" | "gu")
}

/// Coordinates are finite and inside the geographic bounds
/// (`lat ∈ [-90,90]`, `lng ∈ [-180,180]`). `NaN` parses as `f64`, so it must
/// be rejected explicitly via the finiteness check.
pub fn is_valid_coords(lat: f64, lng: f64) -> bool {
    lat.is_finite()
        && lng.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lng)
}

/// `^[1-9][0-9]{5}$` - Indian pincodes never start with `0`.
pub fn is_valid_pincode(pincode: &str) -> bool {
    let b = pincode.as_bytes();
    b.len() == 6 && (b'1'..=b'9').contains(&b[0]) && b[1..].iter().all(|d| d.is_ascii_digit())
}

/// Cache keys are `pub(crate)` so the slice tests can seed/inspect slots.
/// Coordinates round to 3 decimals so micro-jitter in device GPS still hits
/// one shared slot (`19.0729845` and `19.0730011` both cache under
/// `19.073,72.877`).
pub(crate) fn nearby_key(lat: f64, lng: f64) -> String {
    format!("askdisha:nearby:{lat:.3},{lng:.3}")
}

pub(crate) fn pin_key(pincode: &str) -> String {
    format!("askdisha:pin:{pincode}")
}

pub(crate) fn faqs_key(lang: &str) -> String {
    format!("askdisha:faqs:{lang}")
}

/// One distance-sorted row of the `/nearby` response; `distance_km` is the
/// upstream km distance rounded to 1 decimal (absent when upstream sent no
/// distance). Optional fields stay out of the JSON when `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyRow {
    pub code: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_hi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_gu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_km: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
}

impl NearbyRow {
    fn from_station(row: NearbyStation) -> Self {
        Self {
            code: row.code,
            name: row.name,
            name_hi: row.name_hi,
            name_gu: row.name_gu,
            distance_km: row.distance.map(round_distance),
            state: row.state,
            district: row.district,
        }
    }
}

/// Round to one decimal place (`1.21` -> `1.2`) for display distances.
pub(crate) fn round_distance(km: f64) -> f64 {
    (km * 10.0).round() / 10.0
}

/// Sort ascending by `distance_km` (rows without a distance last) and cap at
/// [`NEARBY_LIMIT`]. Pure so the slice tests exercise it without network.
pub(crate) fn normalize_nearby(mut rows: Vec<NearbyRow>) -> Vec<NearbyRow> {
    rows.sort_by(|a, b| {
        let da = sortable_distance(a.distance_km);
        let db = sortable_distance(b.distance_km);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(NEARBY_LIMIT);
    rows
}

/// Missing or non-finite distances sort last (as positive infinity).
fn sortable_distance(km: Option<f64>) -> f64 {
    km.filter(|d| d.is_finite()).unwrap_or(f64::INFINITY)
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub enabled: bool,
    pub sources: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct NearbyResponse {
    pub source: &'static str,
    pub cached: bool,
    pub count: usize,
    pub stations: Vec<NearbyRow>,
}

/// Inner payload cached for `/pin` (the envelope adds `source`/`cached`
/// around it on the way out, exactly like the other endpoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedPin {
    state: String,
    #[serde(rename = "cityList")]
    city_list: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PinResponse {
    pub source: &'static str,
    pub cached: bool,
    pub state: String,
    #[serde(rename = "cityList")]
    pub city_list: Vec<String>,
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

    /// Stations around a coordinate. The cache key rounds to 3 decimals;
    /// rows are sorted nearest-first (no-distance last) and capped at 50
    /// *before* caching, so hits come back already normalized.
    pub async fn nearby(state: &AppState, lat: f64, lng: f64) -> Result<NearbyResponse, AskError> {
        let key = nearby_key(lat, lng);
        if let Some(v) = state.cache.get(&key) {
            if let Ok(stations) = serde_json::from_value::<Vec<NearbyRow>>(v) {
                return Ok(nearby_envelope(true, stations));
            }
        }

        if state.failover.should_skip("corover-api") {
            return Err(AppError::source_unavailable(
                SOURCE_API,
                "circuit open — corover-api temporarily unavailable (cooldown)",
            )
            .into());
        }

        let started = Instant::now();
        let rows = Self::client(state)?
            .stations_by_location(lat, lng)
            .await
            .map_err(|e| {
                if matches!(
                    e,
                    AppError::SourceUnavailable { .. } | AppError::Internal(_)
                ) {
                    state.failover.record_failure("corover-api");
                }
                e
            })?;
        state
            .metrics
            .record_source_latency(SOURCE_API, started.elapsed());
        state.failover.record_success("corover-api");
        let resp = nearby_envelope(
            false,
            rows.into_iter().map(NearbyRow::from_station).collect(),
        );
        state
            .cache
            .set_with_ttl(&key, serde_json::to_value(&resp.stations)?, NEARBY_TTL);
        Ok(resp)
    }

    /// State + cities served by a postal code (hidden utility route, no UI).
    pub async fn pin(state: &AppState, pincode: &str) -> Result<PinResponse, AskError> {
        let key = pin_key(pincode);
        if let Some(v) = state.cache.get(&key) {
            if let Ok(hit) = serde_json::from_value::<CachedPin>(v) {
                return Ok(PinResponse {
                    source: SOURCE_API,
                    cached: true,
                    state: hit.state,
                    city_list: hit.city_list,
                });
            }
        }

        if state.failover.should_skip("corover-api") {
            return Err(AppError::source_unavailable(
                SOURCE_API,
                "circuit open — corover-api temporarily unavailable (cooldown)",
            )
            .into());
        }

        let started = Instant::now();
        let lookup: PinLookup = Self::client(state)?
            .pin_lookup(pincode)
            .await
            .map_err(|e| {
                if matches!(
                    e,
                    AppError::SourceUnavailable { .. } | AppError::Internal(_)
                ) {
                    state.failover.record_failure("corover-api");
                }
                e
            })?;
        state
            .metrics
            .record_source_latency(SOURCE_API, started.elapsed());
        state.failover.record_success("corover-api");
        let inner = CachedPin {
            state: lookup.state,
            city_list: lookup.city_list,
        };
        state
            .cache
            .set_with_ttl(&key, serde_json::to_value(&inner)?, PIN_TTL);
        Ok(PinResponse {
            source: SOURCE_API,
            cached: false,
            state: inner.state,
            city_list: inner.city_list,
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

        if state.failover.should_skip("corover-cdn") {
            return Err(AppError::source_unavailable(
                SOURCE_CDN,
                "circuit open — corover-cdn temporarily unavailable (cooldown)",
            )
            .into());
        }

        let started = Instant::now();
        let faqs = Self::client(state)?.fetch_faqs(lang).await.map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state.failover.record_failure("corover-cdn");
            }
            e
        })?;
        state
            .metrics
            .record_source_latency(SOURCE_CDN, started.elapsed());
        state.failover.record_success("corover-cdn");
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

        if state.failover.should_skip("corover-cdn") {
            return Err(AppError::source_unavailable(
                SOURCE_CDN,
                "circuit open — corover-cdn temporarily unavailable (cooldown)",
            )
            .into());
        }

        let started = Instant::now();
        let settings = Self::client(state)?.fetch_settings().await.map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state.failover.record_failure("corover-cdn");
            }
            e
        })?;
        state
            .metrics
            .record_source_latency(SOURCE_CDN, started.elapsed());
        state.failover.record_success("corover-cdn");
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

fn nearby_envelope(cached: bool, stations: Vec<NearbyRow>) -> NearbyResponse {
    let stations = normalize_nearby(stations);
    NearbyResponse {
        source: SOURCE_API,
        cached,
        count: stations.len(),
        stations,
    }
}
