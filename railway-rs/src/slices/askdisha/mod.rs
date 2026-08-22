//! AskDISHA slice (feature-gated module, `docs/ASKDISHA_MODULE.md`).
//!
//! Endpoints under `/rail-api/askdisha/*`:
//! - `GET /status`                -> module + source ids
//! - `GET /nearby?lat=&lng=`      -> stationsByLocation (cache 30 min)
//! - `GET /pin/:pincode`          -> state/city lookup (cache 7 d)
//! - `GET /faqs?lang=en|hi|gu`    -> CDN FAQ list (cache 24 h)
//! - `GET /settings`              -> CDN feature flags (cache 1 h)
//!
//! The router is merged in `web.rs` only when `state.askdisha` is `Some`
//! (`ASKDISHA_ENABLED=1`); a disabled deployment has zero route/network
//! footprint (404 fall-through). Handlers additionally guard with a 503
//! `{"enabled":false}` envelope should they ever run against disabled state.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::state::AppState;

pub mod service;

pub use service::{
    AskError, FaqsResponse, NearbyResponse, NearbyRow, PinResponse, SettingsEnvelope,
    StatusResponse,
};

#[derive(Deserialize, Default)]
struct NearbyQuery {
    lat: Option<String>,
    lng: Option<String>,
}

#[derive(Deserialize, Default)]
struct FaqsQuery {
    lang: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/askdisha/status", get(status_handler))
        .route("/rail-api/askdisha/nearby", get(nearby_handler))
        .route("/rail-api/askdisha/pin/:pincode", get(pin_handler))
        .route("/rail-api/askdisha/faqs", get(faqs_handler))
        .route("/rail-api/askdisha/settings", get(settings_handler))
}

async fn status_handler(State(state): State<AppState>) -> Result<Json<StatusResponse>, AskError> {
    Ok(Json(service::Service::status(&state)?))
}

async fn nearby_handler(
    State(state): State<AppState>,
    Query(q): Query<NearbyQuery>,
) -> Result<Json<NearbyResponse>, AskError> {
    // Absent/empty params are "missing"; anything unparseable or outside the
    // geographic bounds is "invalid" (both 400 per the module contract).
    let lat_raw = q.lat.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let lng_raw = q.lng.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let (Some(lat_raw), Some(lng_raw)) = (lat_raw, lng_raw) else {
        return Err(AppError::bad_request("missing coordinates").into());
    };
    let Some((lat, lng)) = lat_raw
        .parse::<f64>()
        .ok()
        .zip(lng_raw.parse::<f64>().ok())
        .filter(|&(lat, lng)| service::is_valid_coords(lat, lng))
    else {
        return Err(AppError::bad_request("invalid coordinates").into());
    };
    Ok(Json(service::Service::nearby(&state, lat, lng).await?))
}

async fn pin_handler(
    State(state): State<AppState>,
    Path(pincode): Path<String>,
) -> Result<Json<PinResponse>, AskError> {
    if !service::is_valid_pincode(&pincode) {
        return Err(AppError::bad_request("invalid pincode").into());
    }
    Ok(Json(service::Service::pin(&state, &pincode).await?))
}

async fn faqs_handler(
    State(state): State<AppState>,
    Query(q): Query<FaqsQuery>,
) -> Result<Json<FaqsResponse>, AskError> {
    // Absent lang defaults to English; anything outside en|hi|gu is a 400.
    let lang = q.lang.as_deref().unwrap_or("en");
    if !service::is_valid_lang(lang) {
        return Err(AppError::bad_request("invalid language").into());
    }
    Ok(Json(service::Service::faqs(&state, lang).await?))
}

async fn settings_handler(
    State(state): State<AppState>,
) -> Result<Json<SettingsEnvelope>, AskError> {
    Ok(Json(service::Service::settings(&state).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    fn enabled_state() -> AppState {
        AppState::for_test(Config {
            askdisha_enabled: true,
            ..Config::default()
        })
    }

    fn disabled_state() -> AppState {
        // `Config::default()` is enabled since the v2 flip; disable explicitly
        // to mirror an `ASKDISHA_ENABLED=0` deployment.
        AppState::for_test(Config {
            askdisha_enabled: false,
            ..Config::default()
        })
    }

    #[test]
    fn pincode_accepts_exactly_six_digits_not_starting_with_zero() {
        for ok in ["400001", "110001", "999999", "100000"] {
            assert!(service::is_valid_pincode(ok), "{ok} must be valid");
        }
        for bad in [
            "",
            "040001",
            "40000",
            "4000011",
            "40a001",
            "40 001",
            "४००००१",
            "40000.",
            "+40001",
        ] {
            assert!(!service::is_valid_pincode(bad), "{bad} must be invalid");
        }
    }

    #[test]
    fn coord_validator_enforces_bounds_and_finiteness() {
        for ok in [
            (0.0, 0.0),
            (-90.0, -180.0),
            (90.0, 180.0),
            (19.0729, 72.8776),
        ] {
            assert!(service::is_valid_coords(ok.0, ok.1), "{ok:?} must be valid");
        }
        for bad in [
            (90.1, 0.0),
            (-90.0001, 0.0),
            (0.0, 180.0001),
            (0.0, -180.0001),
            (f64::NAN, 0.0),
            (0.0, f64::NAN),
            (f64::INFINITY, 0.0),
            (0.0, f64::NEG_INFINITY),
        ] {
            assert!(
                !service::is_valid_coords(bad.0, bad.1),
                "{bad:?} must be invalid"
            );
        }
    }

    #[test]
    fn nearby_cache_key_rounds_coords_to_three_decimals() {
        assert_eq!(
            service::nearby_key(19.0729845, 72.87761),
            "askdisha:nearby:19.073,72.878"
        );
        // GPS micro-jitter must share one slot after rounding.
        assert_eq!(
            service::nearby_key(19.07298, 72.8776),
            service::nearby_key(19.07291, 72.87801)
        );
        // Negatives keep their sign; whole numbers stay deterministic.
        assert_eq!(
            service::nearby_key(-23.55014, -46.63392),
            "askdisha:nearby:-23.550,-46.634"
        );
        assert_eq!(service::pin_key("400001"), "askdisha:pin:400001");
        assert_eq!(service::faqs_key("hi"), "askdisha:faqs:hi");
    }

    fn nearby_row(code: &str, km: Option<f64>) -> NearbyRow {
        NearbyRow {
            code: code.to_string(),
            name: format!("STATION {code}"),
            name_hi: None,
            name_gu: None,
            distance_km: km,
            state: None,
            district: None,
        }
    }

    #[test]
    fn normalize_nearby_sorts_ascending_none_last_and_caps_at_fifty() {
        let mut rows = Vec::new();
        // Deliberately unordered; includes missing (None) and non-finite.
        for i in 0..55usize {
            let km = match i % 5 {
                0 => Some((i % 11) as f64 * 1.5),
                1 => None,
                2 => Some(f64::NAN),
                _ => Some((i % 7) as f64 * 0.25),
            };
            rows.push(nearby_row(&format!("S{i:02}"), km));
        }
        let out = service::normalize_nearby(rows);

        assert_eq!(out.len(), 50, "cap at 50 rows");
        // Every finite distance must appear in non-decreasing order...
        let finite: Vec<f64> = out
            .iter()
            .filter_map(|r| r.distance_km.filter(|d| d.is_finite()))
            .collect();
        let mut sorted = finite.clone();
        sorted.sort_by(f64::total_cmp);
        assert_eq!(finite, sorted, "finite distances ascend");
        // ...and every trailing row (after the last finite one) has no
        // usable distance (missing or NaN sorts last).
        let last_finite_idx = out
            .iter()
            .rposition(|r| r.distance_km.is_some_and(|d| d.is_finite()))
            .expect("at least one finite row");
        assert!(
            out[last_finite_idx + 1..]
                .iter()
                .all(|r| !r.distance_km.is_some_and(|d| d.is_finite())),
            "rows without a usable distance come last"
        );
    }

    #[test]
    fn distance_rounding_is_one_decimal() {
        assert_eq!(service::round_distance(1.21), 1.2);
        assert_eq!(service::round_distance(4.04), 4.0);
        assert_eq!(service::round_distance(1.26), 1.3);
    }

    #[test]
    fn nearby_row_json_omits_absent_optionals_and_keeps_distance() {
        let row: NearbyRow = serde_json::from_value(serde_json::json!({
            "code": "CLA",
            "name": "KURLA JN",
            "distance_km": 1.2
        }))
        .expect("row parses");
        let v = serde_json::to_value(&row).expect("serializes");
        assert_eq!(v["code"], "CLA");
        assert_eq!(v["distance_km"], 1.2);
        for absent in ["name_hi", "name_gu", "state", "district"] {
            assert!(v.get(absent).is_none(), "{absent} omitted when None");
        }
    }

    #[tokio::test]
    async fn disabled_module_answers_503_enabled_false_envelope() {
        let state = disabled_state();
        assert!(state.askdisha.is_none());

        // Service-level guard on every endpoint family.
        for err in [
            service::Service::status(&state).unwrap_err(),
            service::Service::nearby(&state, 19.07, 72.87)
                .await
                .unwrap_err(),
            service::Service::pin(&state, "400001").await.unwrap_err(),
            service::Service::faqs(&state, "en").await.unwrap_err(),
            service::Service::settings(&state).await.unwrap_err(),
        ] {
            let resp = err.into_response();
            assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_envelope_is_enabled_false(resp).await;
        }

        // Handler-level guard (defensive; routes would be unmerged anyway).
        let resp = settings_handler(State(disabled_state()))
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_envelope_is_enabled_false(resp).await;

        let resp = pin_handler(State(disabled_state()), Path("400001".to_string()))
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_envelope_is_enabled_false(resp).await;
    }

    async fn assert_envelope_is_enabled_false(resp: axum::response::Response) {
        use http_body_util::BodyExt;
        let (_parts, body) = resp.into_parts();
        let bytes = BodyExt::collect(body)
            .await
            .expect("body collects")
            .to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(v["enabled"], serde_json::Value::Bool(false));
        assert!(v.get("source").is_none());
    }

    #[test]
    fn validation_errors_map_to_400_with_error_field() {
        for e in [
            AppError::bad_request("missing coordinates"),
            AppError::bad_request("invalid coordinates"),
            AppError::bad_request("invalid pincode"),
            AppError::bad_request("invalid language"),
        ] {
            let e: AskError = e.into();
            let resp = e.into_response();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }
    }

    /// Cache-first behavior without any network: seed the cache, then read.
    #[tokio::test]
    async fn nearby_serves_from_cache_marks_cached_true() {
        let state = enabled_state();
        let rows = serde_json::json!([
            {"code": "CLA", "name": "KURLA JN", "name_hi": "कुर्ला जन", "distance_km": 1.2},
            {"code": "LTT", "name": "LOKMANYATILAK T", "distance_km": 1.6}
        ]);
        state.cache.set_with_ttl(
            &service::nearby_key(19.07, 72.87),
            rows,
            std::time::Duration::from_secs(60),
        );

        let resp = service::Service::nearby(&state, 19.07, 72.87)
            .await
            .expect("cached hit");
        assert!(resp.cached);
        assert_eq!(resp.source, "corover-api");
        assert_eq!(resp.count, 2);
        assert_eq!(resp.stations[0].code, "CLA");
        assert_eq!(resp.stations[0].distance_km, Some(1.2));
        assert_eq!(resp.stations[0].name_hi.as_deref(), Some("कुर्ला जन"));
        assert_eq!(resp.stations[1].district, None);

        // A different rounded coordinate misses this slot (no client wired in
        // the disabled sense here, so just confirm the key differs).
        assert_ne!(
            service::nearby_key(19.07, 72.87),
            service::nearby_key(19.08, 72.88)
        );
    }

    #[tokio::test]
    async fn pin_serves_from_cache_marks_cached_true() {
        let state = enabled_state();
        state.cache.set_with_ttl(
            &service::pin_key("400001"),
            serde_json::json!({"state": "MAHARASHTRA", "cityList": ["Raigarh(MH)", "Mumbai"]}),
            std::time::Duration::from_secs(60),
        );

        let resp = service::Service::pin(&state, "400001")
            .await
            .expect("cached hit");
        assert!(resp.cached);
        assert_eq!(resp.source, "corover-api");
        assert_eq!(resp.state, "MAHARASHTRA");
        assert_eq!(resp.city_list, vec!["Raigarh(MH)", "Mumbai"]);

        // Wire shape: camelCase `cityList`, no fabricated fields.
        let v = serde_json::to_value(&resp).expect("serializes");
        assert_eq!(v["cityList"][0], "Raigarh(MH)");
        assert!(v.get("cities").is_none());
    }

    #[tokio::test]
    async fn settings_and_faqs_serve_from_cache() {
        let state = enabled_state();

        state.cache.set_with_ttl(
            "askdisha:settings",
            serde_json::json!({"id": 1, "isDisabled": false, "booking": true}),
            std::time::Duration::from_secs(60),
        );
        let set = service::Service::settings(&state)
            .await
            .expect("cached hit");
        assert!(set.cached);
        assert_eq!(set.source, "corover-cdn");
        assert_eq!(set.settings.id, 1);
        assert!(set.settings.booking && !set.settings.is_disabled);

        state.cache.set_with_ttl(
            "askdisha:faqs:hi",
            serde_json::json!(["प्रश्न एक", "प्रश्न दो"]),
            std::time::Duration::from_secs(60),
        );
        let faqs = service::Service::faqs(&state, "hi")
            .await
            .expect("cached hit");
        assert!(faqs.cached);
        assert_eq!(faqs.source, "corover-cdn");
        assert_eq!(faqs.faqs.len(), 2);
    }

    #[test]
    fn lang_whitelist_is_strict() {
        for ok in ["en", "hi", "gu"] {
            assert!(service::is_valid_lang(ok), "{ok} must be allowed");
        }
        for bad in ["", "EN", "fr", "eng", "en-US", " hi"] {
            assert!(!service::is_valid_lang(bad), "{bad} must be rejected");
        }
    }

    #[tokio::test]
    async fn status_reports_sources_when_enabled() {
        let state = enabled_state();
        let status = service::Service::status(&state).expect("enabled");
        assert!(status.enabled);
        assert_eq!(status.sources, vec!["corover-api", "corover-cdn"]);
    }
}
