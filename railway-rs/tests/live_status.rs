mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::TestApp;
use railway_rs::core::ntes::NtesCrypto;

fn live_fixture() -> String {
    std::fs::read_to_string("testdata/ry_live_12951.html").unwrap()
}

fn mock_12951(app: &TestApp) {
    app.mock("railyatri")
        .route_html("/live-train-status/12951", live_fixture());
}

fn fixture_next_station_code() -> String {
    let norm = railway_rs::core::railyatri::parse_live_status(&live_fixture()).unwrap();
    norm["next_station_code"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn missing_train_is_400() {
    let app = TestApp::spawn().await;
    let (status, _body) = app.get("/rail-api/live-status").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn live_status_from_real_fixture() {
    let app = TestApp::spawn().await;
    mock_12951(&app);

    let (status, body) = app.get("/rail-api/live-status?train=12951").await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(body["train_number"], "12951");
    assert!(!body["train_name"].as_str().unwrap().is_empty());

    let location = body["current_location_info"].as_str().unwrap();
    assert!(!location.is_empty());
    assert!(
        location.contains("MUMBAI")
            || location.contains("NEW DELHI")
            || location.contains("BORIVALI")
    );

    let stations = body["stations"].as_array().unwrap();
    assert!(stations.len() > 100);

    let next_code = fixture_next_station_code();
    let expected: Vec<_> = stations
        .iter()
        .filter(|s| s["status"] == "expected")
        .collect();
    assert_eq!(expected.len(), 1, "exactly one station marked expected");
    assert_eq!(expected[0]["code"], next_code);

    assert_eq!(body["data_source"], "Railyatri");

    for s in stations {
        assert_eq!(
            s["actual_arrival"], "",
            "actual arrival must never be invented"
        );
        assert_eq!(s["delay_minutes"], 0, "delay must never be invented");
    }
}

#[tokio::test]
async fn unknown_train_is_404() {
    let app = TestApp::spawn().await;
    app.mock("railyatri")
        .route_error("/live-train-status/99999", StatusCode::NOT_FOUND);

    let (status, body) = app.get("/rail-api/live-status?train=99999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("99999"));
}

#[tokio::test]
async fn upstream_error_is_502() {
    let app = TestApp::spawn().await;
    app.mock("railyatri").route_error(
        "/live-train-status/55555",
        StatusCode::INTERNAL_SERVER_ERROR,
    );

    let (status, body) = app.get("/rail-api/live-status?train=55555").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body["error"].as_str().unwrap().contains("unavailable"));
}

#[tokio::test]
async fn past_date_is_rejected() {
    let app = TestApp::spawn().await;
    mock_12951(&app);

    let (status, body) = app
        .get("/rail-api/live-status?train=12951&date=2020-01-01")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("today"));
}

#[tokio::test]
async fn ntes_is_primary_source_when_reachable() {
    let app = TestApp::spawn().await;
    let payload = r#"{"trainNo":"12951","trainName":"MUMBAI RAJDHANI","startStationCode":"MMCT","startStationName":"MUMBAI CENTRAL","endStationCode":"NDLS","endStationName":"NEW DELHI","atStationCode":"BVI","atStationName":"BORIVALI","nextStationCode":"BVI","nextStationName":"BORIVALI","platformNumber":"5","trainStartDate":"2026-01-01","stationList":[{"stationCode":"MMCT","stationName":"MUMBAI CENTRAL","arrivalTime":"17:40","actualArrival":"17:40","day":1},{"stationCode":"BVI","stationName":"BORIVALI","arrivalTime":"18:05","actualArrival":"18:15","day":1},{"stationCode":"NDLS","stationName":"NEW DELHI","arrivalTime":"08:32","actualArrival":"","day":2}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({ "jsonIn": NtesCrypto::build(payload) }),
    );

    // Railyatri has no mock route, so a 200 with data_source NTES proves the
    // gov source was used first and no fallback happened.
    let (status, body) = app.get("/rail-api/live-status?train=12951").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["train_number"], "12951");
    assert_eq!(body["data_source"], "NTES");

    let stations = body["stations"].as_array().unwrap();
    assert_eq!(stations.len(), 3);
    assert_eq!(stations[0]["status"], "departed");
    assert_eq!(stations[1]["status"], "expected");
    assert_eq!(stations[2]["status"], "scheduled");

    // NTES real per-stop actuals are surfaced verbatim; missing ones stay empty.
    assert_eq!(stations[1]["actual_arrival"], "18:15");
    assert_eq!(stations[2]["actual_arrival"], "");
    assert_eq!(
        stations[0]["delay_minutes"], 0,
        "delay must never be invented"
    );

    assert!(body["current_location_info"]
        .as_str()
        .unwrap()
        .contains("BORIVALI"));
}
