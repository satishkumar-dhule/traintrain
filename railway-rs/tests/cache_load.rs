//! Cache unit tests (against the real `railway_rs::core::Cache`) plus a small
//! HTTP load test against a spawned `TestApp`.
//!
//! Notes on what is real in this codebase:
//! - `Cache` is a TTL map with `get`/`set`/`remove`/`len`/`is_empty`/`clear`
//!   and NO hit/miss counters, so those are asserted via observable behavior
//!   rather than fabricated counters.
//! - `/rail-api/stations` and `/rail-api/search/*` are served purely from local
//!   datasets (no upstream), which makes them hermetic and fast for load.
//! - `/rail-api/ntes/trains-between` is genuinely cached (key
//!   `trains_between:<SRC>:<DST>`; raw NTES payload stored, re-mapped on read),
//!   so the cache-over-HTTP test uses it and verifies a second request is
//!   served even after the upstream mock is broken.
//!   NOTE: `/rail-api/live-status` also writes to the cache but its read path
//!   re-parses the *wire* model as the *normalized* shape, so its cache never
//!   actually hits - not usable for a cache-behavior assertion.

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::{json, Value};

use common::TestApp;
use railway_rs::core::ntes::NtesCrypto;
use railway_rs::core::Cache;

// ---------------------------------------------------------------------------
// A) Cache unit tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cache_newly_created_cache_is_empty() {
    let c = Cache::new(Duration::from_secs(60));
    assert!(c.is_empty());
    assert_eq!(c.len(), 0);
}

#[tokio::test]
async fn cache_set_and_get_roundtrip() {
    let c = Cache::new(Duration::from_secs(60));
    c.set("pnr:8456789012", json!({"pax": [{"name": "A"}]}));
    assert_eq!(
        c.get("pnr:8456789012"),
        Some(json!({"pax": [{"name": "A"}]}))
    );
}

#[tokio::test]
async fn cache_missing_key_is_none() {
    let c = Cache::new(Duration::from_secs(60));
    assert_eq!(c.get("no-such-key"), None);
    assert_eq!(c.get("also-missing"), None);
}

#[tokio::test]
async fn cache_get_returns_none_after_ttl_expiry() {
    let c = Cache::new(Duration::from_millis(50));
    c.set("a", json!(1));
    assert!(c.get("a").is_some());
    // Sleep well past the TTL (50ms -> 200ms) to avoid timer flakiness.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(c.get("a"), None);
    assert_eq!(c.len(), 0, "expired entry must be removed lazily on read");
}

#[tokio::test]
async fn cache_overwrite_updates_value_and_keeps_single_entry() {
    let c = Cache::new(Duration::from_secs(60));
    c.set("schedule:12951", json!({"train": "12951", "run": "old"}));
    c.set("schedule:12951", json!({"train": "12951", "run": "new"}));
    assert_eq!(
        c.get("schedule:12951"),
        Some(json!({"train": "12951", "run": "new"}))
    );
    assert_eq!(c.len(), 1, "overwriting a key must not grow the map");
}

#[tokio::test]
async fn cache_set_sweeps_expired_entries_on_write() {
    let c = Cache::new(Duration::from_millis(50));
    c.set("stale", json!(1));
    tokio::time::sleep(Duration::from_millis(200)).await;
    c.set("fresh", json!(2));
    assert_eq!(c.len(), 1, "write must sweep the expired 'stale' entry");
    assert_eq!(c.get("stale"), None);
    assert_eq!(c.get("fresh"), Some(json!(2)));
}

#[tokio::test]
async fn cache_remove_and_clear_work() {
    let c = Cache::new(Duration::from_secs(60));
    c.set("a", json!(1));
    c.set("b", json!(2));
    c.remove("a");
    assert!(c.get("a").is_none());
    assert!(c.get("b").is_some());
    c.clear();
    assert!(c.is_empty());
    assert_eq!(c.get("b"), None);
}

#[tokio::test]
async fn cache_keys_are_independent() {
    let c = Cache::new(Duration::from_secs(60));
    c.set("pnr:1", json!("one"));
    c.set("pnr:2", json!("two"));
    assert_eq!(c.get("pnr:1"), Some(json!("one")));
    assert_eq!(c.get("pnr:2"), Some(json!("two")));
}

// ---------------------------------------------------------------------------
// B) HTTP load test
// ---------------------------------------------------------------------------

/// Fire 200 concurrent requests at the local-data stations endpoint across 8
/// workers. No upstream involved, so this is hermetic; asserts every request
/// succeeds and returns a JSON array.
#[tokio::test(flavor = "multi_thread")]
async fn load_test_stations_endpoint_200_concurrent_requests() {
    let app = TestApp::spawn().await;
    let url = format!("{}/rail-api/stations?q=NDLS", app.base_url());

    const N: usize = 200;
    const WORKERS: usize = 8;

    let mut set = tokio::task::JoinSet::new();
    for _ in 0..WORKERS {
        let url = url.clone();
        set.spawn(async move {
            let mut ok = 0usize;
            let mut not_found = 0usize;
            let mut server_error = 0usize;
            let mut valid_array = 0usize;
            for _ in 0..N / WORKERS {
                let resp = reqwest::get(&url).await.expect("request to app");
                if resp.status() == StatusCode::NOT_FOUND {
                    not_found += 1;
                }
                if resp.status().is_server_error() {
                    server_error += 1;
                }
                if resp.status().is_success() {
                    ok += 1;
                    let body: Value = resp.json().await.unwrap_or(Value::Null);
                    if body.is_array() && !body.as_array().unwrap().is_empty() {
                        valid_array += 1;
                    }
                }
            }
            (ok, not_found, server_error, valid_array)
        });
    }

    let mut total_ok = 0usize;
    let mut total_not_found = 0usize;
    let mut total_5xx = 0usize;
    let mut total_valid = 0usize;
    while let Some(res) = set.join_next().await {
        let (ok, nf, s5, arr) = res.expect("worker task");
        total_ok += ok;
        total_not_found += nf;
        total_5xx += s5;
        total_valid += arr;
    }

    assert_eq!(total_ok, N, "all requests must return 2xx");
    assert_eq!(total_not_found, 0, "no 404s under load");
    assert_eq!(total_5xx, 0, "no 5xx under load");
    assert_eq!(
        total_valid, N,
        "every success body must be a non-empty JSON array"
    );
}

/// A smaller concurrent load pass over the local train-search endpoint: one
/// task per request so the full count is actually fired.
#[tokio::test(flavor = "multi_thread")]
async fn load_test_train_search_100_concurrent_requests() {
    let app = TestApp::spawn().await;
    let url = format!("{}/rail-api/search/trains?q=12951", app.base_url());

    const N: usize = 100;

    let mut set = tokio::task::JoinSet::new();
    for _ in 0..N {
        let url = url.clone();
        set.spawn(async move {
            let resp = reqwest::get(&url).await.expect("request to app");
            (resp.status().is_success(), resp.status())
        });
    }

    let mut total_ok = 0usize;
    let mut failures = Vec::new();
    while let Some(res) = set.join_next().await {
        let (ok, status) = res.expect("request task");
        if ok {
            total_ok += 1;
        } else {
            failures.push(status);
        }
    }

    assert_eq!(total_ok, N, "all {N} requests must succeed");
    assert!(
        failures.is_empty(),
        "unexpected non-2xx statuses: {failures:?}"
    );
}

// ---------------------------------------------------------------------------
// C) Cache-behavior over HTTP
// ---------------------------------------------------------------------------

/// `/rail-api/ntes/trains-between` is the genuinely cached slice (key
/// `trains_between:<SRC>:<DST>`; the raw NTES payload is stored and re-mapped
/// on read). First request fetches from the ntes mock and populates the cache;
/// after the upstream mock is broken, a second request must still succeed
/// because it is served from cache. Verified via the public `AppState.cache`
/// API (the Cache type exposes no hit/miss counters).
#[tokio::test]
async fn cached_slice_serves_second_request_from_cache() {
    let app = TestApp::spawn().await;
    let payload = r#"{"trainBtwStationList":[{"trainNo":"12951","trainName":"MUMBAI RAJDHANI","depTime":"17:40","arrTime":"08:32","runOnMon":true,"runOnTue":true,"runOnWed":true,"runOnThu":true,"runOnFri":true,"runOnSat":true,"runOnSun":true}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({ "jsonIn": NtesCrypto::build(payload) }),
    );

    let (s1, body1) = app
        .get("/rail-api/ntes/trains-between?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(body1["trains"][0]["number"], "12951");
    assert_eq!(
        app.state.cache.len(),
        1,
        "trains_between entry must be stored after the first request"
    );

    // Break the upstream: a cache hit must still serve the second request.
    app.mocks["ntes"].route_error("/crisns/AppServAnd", StatusCode::INTERNAL_SERVER_ERROR);

    let (s2, body2) = app
        .get("/rail-api/ntes/trains-between?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(
        s2,
        StatusCode::OK,
        "second request must be served from cache"
    );
    assert_eq!(body2["trains"][0]["number"], "12951");

    let cached = app.state.cache.get("trains_between:MMCT:NDLS");
    assert!(
        cached.is_some(),
        "raw entry reachable via public Cache::get"
    );
}

/// The observability slice reports real request counters; assert the stations
/// path count lands in `top_paths` and `requests_total` is incremented.
/// (No cache hit/miss counters exist in the metrics model - see file header.)
#[tokio::test]
async fn observability_reports_request_metrics() {
    let app = TestApp::spawn().await;
    const N: usize = 5;
    for _ in 0..N {
        let (status, _) = app.get("/rail-api/stations?q=NDLS").await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, body) = app.get("/rail-api/observability").await;
    assert_eq!(status, StatusCode::OK);

    let total = body["requests_total"]
        .as_u64()
        .expect("requests_total present");
    assert!(
        total > N as u64,
        "expected > {N} recorded requests, got {total}"
    );

    let paths = body["top_paths"].as_array().expect("top_paths present");
    let stations_count = paths
        .iter()
        .find(|p| p[0] == "/rail-api/stations")
        .and_then(|p| p[1].as_u64())
        .unwrap_or(0);
    assert_eq!(stations_count, N as u64);

    assert!(body["uptime_secs"].is_u64());
}
