//! AskDISHA slice (feature-gated module, `docs/ASKDISHA_MODULE.md`).
//!
//! Endpoints under `/rail-api/askdisha/*`:
//! - `GET /status`                       -> module + source ids
//! - `GET /stations?q=`                  -> CoRover typeahead (cache 6 h)
//! - `GET /schedule/:train_no?date&from` -> trnscheduleEnq (cache 30 min)
//! - `GET /faqs?lang=en|hi|gu`           -> CDN FAQ list (cache 24 h)
//! - `GET /settings`                     -> CDN feature flags (cache 1 h)
//!
//! The router is merged in `web.rs` only when `state.askdisha` is `Some`
//! (`ASKDISHA_ENABLED=1`); a disabled deployment has zero route/network
//! footprint. Handlers additionally guard with a 503 `{"enabled":false}`
//! envelope should they ever run against disabled state.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::core::error::AppError;
use crate::state::AppState;

pub mod service;

pub use service::{
    AskError, FaqsResponse, ScheduleEnvelope, SettingsEnvelope, StationsResponse, StatusResponse,
};

#[derive(Deserialize, Default)]
struct StationsQuery {
    q: Option<String>,
}

#[derive(Deserialize, Default)]
struct ScheduleParams {
    date: Option<String>,
    from: Option<String>,
}

#[derive(Deserialize, Default)]
struct FaqsQuery {
    lang: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/askdisha/status", get(status_handler))
        .route("/rail-api/askdisha/stations", get(stations_handler))
        .route(
            "/rail-api/askdisha/schedule/:train_no",
            get(schedule_handler),
        )
        .route("/rail-api/askdisha/faqs", get(faqs_handler))
        .route("/rail-api/askdisha/settings", get(settings_handler))
}

async fn status_handler(State(state): State<AppState>) -> Result<Json<StatusResponse>, AskError> {
    Ok(Json(service::Service::status(&state)?))
}

async fn stations_handler(
    State(state): State<AppState>,
    Query(q): Query<StationsQuery>,
) -> Result<Json<StationsResponse>, AskError> {
    let query = q.q.as_deref().unwrap_or_default().trim();
    if query.is_empty() {
        return Err(AppError::bad_request("missing station query").into());
    }
    Ok(Json(service::Service::stations(&state, query).await?))
}

async fn schedule_handler(
    State(state): State<AppState>,
    Path(train_no): Path<String>,
    Query(params): Query<ScheduleParams>,
) -> Result<Json<ScheduleEnvelope>, AskError> {
    if !service::is_valid_train_no(&train_no) {
        return Err(AppError::bad_request("invalid train number").into());
    }
    let date = params.date.as_deref().filter(|d| !d.is_empty());
    let from = params.from.as_deref().filter(|f| !f.is_empty());
    Ok(Json(
        service::Service::schedule(&state, &train_no, date, from).await?,
    ))
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
        AppState::for_test(Config::default())
    }

    #[test]
    fn train_no_accepts_one_to_five_digits_only() {
        for ok in ["1", "12", "129", "1295", "12951"] {
            assert!(service::is_valid_train_no(ok), "{ok} must be valid");
        }
        for bad in [
            "",
            "123456",
            "12a45",
            "12 45",
            "-129",
            "+12",
            "१२९५१",
            "12.5",
        ] {
            assert!(!service::is_valid_train_no(bad), "{bad} must be invalid");
        }
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

    #[test]
    fn stations_cache_key_lowercases_query() {
        assert_eq!(
            service::stations_key("NEW DelhI"),
            "askdisha:stations:new delhi"
        );
        assert_eq!(service::faqs_key("hi"), "askdisha:faqs:hi");
    }

    #[test]
    fn schedule_cache_key_positions_params_and_allows_absent() {
        assert_eq!(
            service::schedule_key("12951", Some("2026-08-22"), Some("BCT")),
            "askdisha:schedule:12951:2026-08-22:BCT"
        );
        assert_eq!(
            service::schedule_key("12951", None, None),
            "askdisha:schedule:12951::"
        );
        // Same train with different params must not collide.
        assert_ne!(
            service::schedule_key("12951", Some("2026-08-22"), None),
            service::schedule_key("12951", Some("2026-08-23"), None)
        );
    }

    #[tokio::test]
    async fn disabled_module_answers_503_enabled_false_envelope() {
        let state = disabled_state();
        assert!(state.askdisha.is_none());

        // Service-level guard on every endpoint family.
        for err in [
            service::Service::status(&state).unwrap_err(),
            service::Service::stations(&state, "new").await.unwrap_err(),
            service::Service::schedule(&state, "12951", None, None)
                .await
                .unwrap_err(),
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
        let e: AskError = AppError::bad_request("invalid train number").into();
        let resp = e.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// Cache-first behavior without any network: seed the cache, then read.
    #[tokio::test]
    async fn stations_serve_from_cache_marks_cached_true() {
        let state = enabled_state();
        let rows = serde_json::json!([
            {"name": "NEW DELHI", "code": "NDLS"},
            {"name": "NEW BHAUPUR", "code": "NEW"}
        ]);
        state.cache.set_with_ttl(
            &format!("askdisha:stations:{}", "new"),
            rows,
            std::time::Duration::from_secs(60),
        );

        let resp = service::Service::stations(&state, "NEW")
            .await
            .expect("cached hit");
        assert!(resp.cached);
        assert_eq!(resp.source, "corover-api");
        assert_eq!(resp.count, 2);
        assert_eq!(resp.stations[0].code, "NDLS");
    }

    #[tokio::test]
    async fn stations_limit_caps_rows_at_twenty() {
        let state = enabled_state();
        let rows: Vec<serde_json::Value> = (0..35)
            .map(|i| serde_json::json!({"name": format!("ST{i}"), "code": format!("S{i:02}")}))
            .collect();
        state.cache.set_with_ttl(
            "askdisha:stations:bulk",
            serde_json::Value::Array(rows),
            std::time::Duration::from_secs(60),
        );
        let resp = service::Service::stations(&state, "bulk")
            .await
            .expect("cached hit");
        assert_eq!(resp.count, 20);
        assert_eq!(resp.stations.len(), 20);
    }

    #[tokio::test]
    async fn schedule_and_settings_serve_from_cache() {
        let state = enabled_state();

        let sched = serde_json::json!({
            "trainNumber": "12951",
            "trainRunsOnMon": true,
            "stationList": []
        });
        state.cache.set_with_ttl(
            "askdisha:schedule:12951:2026-08-22:BCT",
            sched,
            std::time::Duration::from_secs(60),
        );
        let env = service::Service::schedule(&state, "12951", Some("2026-08-22"), Some("BCT"))
            .await
            .expect("cached hit");
        assert!(env.cached);
        assert_eq!(env.source, "corover-api");
        assert_eq!(env.schedule.train_number, "12951");

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

    #[tokio::test]
    async fn status_reports_sources_when_enabled() {
        let state = enabled_state();
        let status = service::Service::status(&state).expect("enabled");
        assert!(status.enabled);
        assert_eq!(status.sources, vec!["corover-api", "corover-cdn"]);
    }
}
