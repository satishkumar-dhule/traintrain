mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::TestApp;
use railway_rs::core::ntes::NtesCrypto;

#[tokio::test]
async fn missing_train_is_bad_request() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/schedule?train=").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "Train must be a number.");
}

#[tokio::test]
async fn real_fixture_train_12951() {
    let app = TestApp::spawn().await;
    let fixture = std::fs::read_to_string("testdata/ry_schedule_12951.html").unwrap();
    app.mocks["railyatri"].route_html("/time-table/12951", fixture);

    let (status, body) = app.get("/rail-api/schedule?train=12951").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["train_number"], "12951");
    assert!(body["train_name"].as_str().is_some_and(|s| !s.is_empty()));

    let run_days = body["running_days"].as_array().unwrap();
    assert!(run_days.iter().any(|d| d == "MON"));

    let stops = body["stops"].as_array().unwrap();
    assert!(stops.len() > 100);
    assert_eq!(stops[0]["code"], "MMCT");

    assert_eq!(body["data_source"], "Railyatri");
    assert_eq!(body["cache_ttl"], 120);
}

#[tokio::test]
async fn unknown_train_is_not_found() {
    let app = TestApp::spawn().await;
    app.mocks["railyatri"].route_error("/time-table/99999", StatusCode::NOT_FOUND);

    let (status, body) = app.get("/rail-api/schedule?train=99999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Train 99999 not found.");
}

#[tokio::test]
async fn source_failure_is_bad_gateway() {
    let app = TestApp::spawn().await;
    app.mocks["railyatri"].route_error("/time-table/1", StatusCode::INTERNAL_SERVER_ERROR);

    let (status, body) = app.get("/rail-api/schedule?train=1").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("Railyatri"));
}

#[tokio::test]
async fn ntes_is_primary_source_when_reachable() {
    let app = TestApp::spawn().await;
    let payload = r#"{"trainNo":"12951","trainName":"MUMBAI RAJDHANI","trainScheduleList":[{"stationCode":"MMCT","stationName":"MUMBAI CENTRAL","arrivalTime":"--","departureTime":"17:40","day":1,"stopNumber":1},{"stationCode":"NDLS","stationName":"NEW DELHI","arrivalTime":"08:32","departureTime":"--","day":2,"stopNumber":2}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({ "jsonIn": NtesCrypto::build(payload) }),
    );

    // Railyatri has no mock route, so a 200 with data_source NTES proves the
    // gov source was used first and no fallback happened.
    let (status, body) = app.get("/rail-api/schedule?train=12951").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["train_number"], "12951");
    assert_eq!(body["train_name"], "MUMBAI RAJDHANI");

    let stops = body["stops"].as_array().unwrap();
    assert_eq!(stops.len(), 2);
    assert_eq!(stops[0]["code"], "MMCT");
    assert_eq!(stops[0]["departure"], "17:40");
    assert_eq!(stops[1]["arrival"], "08:32");
    assert_eq!(stops[1]["day"], 2);

    assert_eq!(body["data_source"], "NTES");
    assert_eq!(body["cache_ttl"], 120);
}

#[tokio::test]
async fn ntes_failure_falls_back_to_railyatri() {
    let app = TestApp::spawn().await;
    let fixture = std::fs::read_to_string("testdata/ry_schedule_12951.html").unwrap();
    app.mocks["railyatri"].route_html("/time-table/12951", fixture);

    // No ntes mock route -> NTES returns SourceUnavailable -> Railyatri wins.
    let (status, body) = app.get("/rail-api/schedule?train=12951").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data_source"], "Railyatri");
    assert!(body["stops"].as_array().is_some_and(|s| s.len() > 100));
}

#[tokio::test]
async fn both_sources_down_is_bad_gateway_mentioning_all() {
    let app = TestApp::spawn().await;
    app.mocks["railyatri"].route_error("/time-table/1", StatusCode::INTERNAL_SERVER_ERROR);

    let (status, body) = app.get("/rail-api/schedule?train=1").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let err = body["error"].as_str().unwrap_or_default();
    assert!(err.contains("NTES"), "error should mention NTES: {err}");
    assert!(
        err.contains("Railyatri"),
        "error should mention Railyatri: {err}"
    );
}
