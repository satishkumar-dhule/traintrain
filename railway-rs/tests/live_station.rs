mod common;

use common::TestApp;
use railway_rs::core::ntes::NtesCrypto;
use serde_json::json;

#[tokio::test]
async fn bad_station_code_is_400() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=XYZ&hours=2")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Station code must be a 4-character code.");
}

#[tokio::test]
async fn unknown_station_is_400() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=NDXX&hours=2")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Station NDXX not found.");
}

#[tokio::test]
async fn live_station_returns_mapped_trains() {
    let app = TestApp::spawn().await;
    let payload = r#"{"trainList":[{"trainNo":"12951","trainName":"MUMBAI RAJDHANI","scheduledTime":"09:15","expectedTime":"09:15","platformNo":"1","delayArr":false},{"trainNo":"12301","trainName":"RAJDHANI EXP","scheduledTime":"10:00","expectedTime":"10:30","platformNo":"2","delayArr":true}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({"jsonIn": NtesCrypto::build(payload)}),
    );

    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=NDLS&hours=2")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["station"], "NDLS");
    assert_eq!(body["hours"], 2);
    assert_eq!(body["data_source"], "NTES");
    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);
    assert_eq!(trains[0]["number"], "12951");
    assert_eq!(trains[0]["name"], "MUMBAI RAJDHANI");
    assert_eq!(trains[0]["sta"], "09:15");
    assert_eq!(trains[0]["eta"], "09:15");
    assert_eq!(trains[0]["platform"], "1");
    assert_eq!(trains[0]["delay_arr"], false);
    assert_eq!(trains[1]["delay_arr"], true);
}

#[tokio::test]
async fn hours_are_clamped_into_range() {
    let app = TestApp::spawn().await;
    let payload = r#"{"trainList":[{"trainNo":"12951","trainName":"MUMBAI RAJDHANI","scheduledTime":"09:15","expectedTime":"09:15","platformNo":"1","delayArr":false}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({"jsonIn": NtesCrypto::build(payload)}),
    );

    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=NDLS&hours=99")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["station"], "NDLS");
    assert_eq!(body["hours"], 4);
}

#[tokio::test]
async fn no_mock_route_is_honest_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=NDLS&hours=2")
        .await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("Live source"));
}
