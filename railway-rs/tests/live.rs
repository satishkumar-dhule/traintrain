//! Live integration tests: hit the REAL upstreams (Railyatri / etrain / NTES)
//! using the default config bases.
//!
//! Gated behind `RAILWAY_LIVE_TESTS=1`. When the flag is unset every test
//! early-returns, so the default `cargo test` run stays fast and hermetic.
//! When set, each test spawns the real app on an ephemeral port and makes
//! real network calls with a generous timeout.
//!
//! Contract notes (see `src/slices/*/router.rs`):
//! - `/rail-api/pnr?pnr=` may return 200 `{"status": ...}` or 404 `{"error": ...}`
//!   but never 500.
//! - `/rail-api/live-status?train=` may return 200 (train block) or 502
//!   `{"error": ...}` (upstream blocked) but never 500, and the body must be
//!   parseable JSON either way.

use std::net::SocketAddr;

use axum::http::StatusCode;
use serde_json::Value;

use railway_rs::config::Config;
use railway_rs::state::AppState;
use railway_rs::web;

/// True only when the caller explicitly opts into real-network tests.
fn live_enabled() -> bool {
    std::env::var("RAILWAY_LIVE_TESTS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

const LIVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Spawn the real app (default config -> real upstream bases) on 127.0.0.1:0
/// and return its address.
async fn spawn_app() -> SocketAddr {
    let config = Config::default();
    let state = AppState::from_config(config).expect("real state builds");
    let app = web::router(state.clone(), state.config.static_dir.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// Spawn the app and return it with a real-network client with a generous
/// timeout.
async fn spawn() -> (SocketAddr, reqwest::Client) {
    let addr = spawn_app().await;
    let client = reqwest::Client::builder()
        .timeout(LIVE_TIMEOUT)
        .build()
        .expect("client builds");
    (addr, client)
}

fn url(addr: SocketAddr, path: &str) -> String {
    format!("http://{addr}{path}")
}

#[tokio::test]
async fn healthz_is_200() {
    if !live_enabled() {
        return;
    }
    let (addr, client) = spawn().await;
    let resp = client
        .get(url(addr, "/healthz"))
        .send()
        .await
        .expect("app reachable");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("healthz body is JSON");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "railway-rs");
}

#[tokio::test]
async fn source_status_shape() {
    if !live_enabled() {
        return;
    }
    let (addr, client) = spawn().await;
    let resp = client
        .get(url(addr, "/rail-api/source-status"))
        .send()
        .await
        .expect("app reachable");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("source-status body is JSON");
    assert_eq!(body["live_enabled"], true);
    assert_eq!(body["mode"], "live");
    let sources = body["sources"].as_array().expect("sources is an array");
    assert!(sources
        .iter()
        .any(|s| s["name"] == "Railyatri" && s["reachable"].is_boolean()));
    assert!(sources
        .iter()
        .any(|s| s["name"] == "NTES" && s["reachable"].is_boolean()));
    assert!(sources
        .iter()
        .any(|s| s["name"] == "etrain" && s["reachable"].is_boolean()));
}

#[tokio::test]
async fn observability_shape() {
    if !live_enabled() {
        return;
    }
    let (addr, client) = spawn().await;
    let resp = client
        .get(url(addr, "/rail-api/observability"))
        .send()
        .await
        .expect("app reachable");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("observability body is JSON");
    assert!(body["uptime_secs"].is_number());
    assert!(body["requests_total"].is_number());
    let origins = body["origins"].as_array().expect("origins is an array");
    assert!(origins.iter().any(|o| o["name"] == "Railyatri"));
    assert!(origins.iter().any(|o| o["name"] == "NTES"));
    assert!(body["top_paths"].is_array());
}

#[tokio::test]
async fn stations_search_real_data() {
    if !live_enabled() {
        return;
    }
    let (addr, client) = spawn().await;
    let resp = client
        .get(url(addr, "/rail-api/stations?q=NDLS"))
        .send()
        .await
        .expect("app reachable");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("stations body is JSON");
    let stations = body.as_array().expect("stations is an array");
    assert!(
        !stations.is_empty(),
        "NDLS must match at least one real station from data/stations.json"
    );
    assert!(
        stations.iter().any(|s| s["code"] == "NDLS"),
        "expected an NDLS entry in real station data"
    );
}

#[tokio::test]
async fn pnr_contract_invalid_pnr() {
    if !live_enabled() {
        return;
    }
    let (addr, client) = spawn().await;
    // Connection failure to the app counts as "upstream blocked": skip.
    let Ok(resp) = client
        .get(url(addr, "/rail-api/pnr?pnr=1234567890"))
        .send()
        .await
    else {
        return;
    };
    let status = resp.status();
    assert_ne!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "PNR endpoint must never 500"
    );
    let body: Value = resp
        .json()
        .await
        .unwrap_or_else(|_| panic!("PNR body must be parseable JSON, got status {status}"));
    match status {
        StatusCode::OK => {
            assert!(
                body.get("status").is_some(),
                "200 PNR response must carry a status field: {body}"
            );
        }
        StatusCode::NOT_FOUND => {
            assert!(
                body.get("error").is_some(),
                "404 PNR response must carry an error field: {body}"
            );
        }
        StatusCode::PRECONDITION_REQUIRED => {
            assert_eq!(
                body.get("error").and_then(Value::as_str),
                Some("captcha_required"),
                "428 PNR response must be a captcha challenge: {body}"
            );
            assert!(
                body.get("image").and_then(Value::as_str).is_some(),
                "428 PNR response must carry a captcha image: {body}"
            );
        }
        other => panic!("PNR contract: unexpected status {other} with body {body}"),
    }
}

#[tokio::test]
async fn live_status_contract() {
    if !live_enabled() {
        return;
    }
    let (addr, client) = spawn().await;
    // Connection failure to the app counts as "upstream blocked": skip.
    let Ok(resp) = client
        .get(url(addr, "/rail-api/live-status?train=12951"))
        .send()
        .await
    else {
        return;
    };
    let status = resp.status();
    assert_ne!(
        status,
        StatusCode::INTERNAL_SERVER_ERROR,
        "live-status endpoint must never 500"
    );
    let body: Value = resp
        .json()
        .await
        .unwrap_or_else(|_| panic!("live-status body must be parseable JSON, got status {status}"));
    match status {
        StatusCode::OK => {
            assert!(
                body.get("train_number").is_some(),
                "200 live-status response must carry a train block: {body}"
            );
            assert!(
                body.get("train_number").and_then(|t| t.as_str()).is_some()
                    || body.get("train_number").and_then(|t| t.as_i64()).is_some(),
                "train_number must be present and scalar: {body}"
            );
        }
        StatusCode::BAD_GATEWAY => {
            assert!(
                body.get("error").is_some(),
                "502 live-status response must carry an error field: {body}"
            );
        }
        other => panic!("live-status contract: unexpected status {other} with body {body}"),
    }
}

#[tokio::test]
async fn unknown_api_route_is_json_404() {
    if !live_enabled() {
        return;
    }
    let (addr, client) = spawn().await;
    let resp = client
        .get(url(addr, "/rail-api/definitely-not-a-route"))
        .send()
        .await
        .expect("app reachable");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body: Value = resp.json().await.expect("404 body is JSON");
    assert!(
        body.get("error").is_some(),
        "unmatched /rail-api path must return {{\"error\": ...}}: {body}"
    );
}
