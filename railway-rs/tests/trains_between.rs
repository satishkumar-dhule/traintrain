mod common;

use serde_json::json;

use common::TestApp;
use railway_rs::core::ntes::NtesCrypto;

#[tokio::test]
async fn missing_or_empty_params_are_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/ntes/trains-between",
        "/rail-api/ntes/trains-between?src=MMCT",
        "/rail-api/ntes/trains-between?dst=NDLS",
        "/rail-api/ntes/trains-between?src=&dst=NDLS",
        "/rail-api/ntes/trains-between?src=MMCT&dst=",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn malformed_codes_are_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/ntes/trains-between?src=MM&dst=NDLS",
        "/rail-api/ntes/trains-between?src=MMCT&dst=ND",
        "/rail-api/ntes/trains-between?src=MM!T&dst=NDLS",
        "/rail-api/ntes/trains-between?src=MMCTT&dst=NDLS",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn same_source_destination_is_bad_request() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=NDLS&dst=ndls")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Source and destination must differ.");
}

#[tokio::test]
async fn unknown_station_is_bad_request() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=NDXX&dst=NDLS")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Station NDXX not found.");
    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=NDLS&dst=ZZZZ")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Station ZZZZ not found.");
}

#[tokio::test]
async fn trains_between_returns_normalized_trains() {
    let app = TestApp::spawn().await;
    let payload = r#"{"trainBtwStationList":[{"trainNo":"12951","trainName":"MUMBAI RAJDHANI","depTime":"17:40","arrTime":"08:32","runOnMon":true,"runOnTue":true,"runOnWed":true,"runOnThu":true,"runOnFri":true,"runOnSat":true,"runOnSun":true},{"trainNo":"12954","trainName":"AK GOLD EXP","depTime":"20:05","arrTime":"10:10","runOnMon":true,"runOnTue":false,"runOnWed":true,"runOnThu":false,"runOnFri":true,"runOnSat":false,"runOnSun":false}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({ "jsonIn": NtesCrypto::build(payload) }),
    );

    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["src"], "MMCT");
    assert_eq!(body["dst"], "NDLS");

    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);

    assert_eq!(trains[0]["number"], "12951");
    assert_eq!(trains[0]["name"], "MUMBAI RAJDHANI");
    assert_eq!(trains[0]["departure_time"], "17:40");
    assert_eq!(trains[0]["arrival_time"], "08:32");
    assert_eq!(
        trains[0]["runs_on"],
        json!([true, true, true, true, true, true, true])
    );

    assert_eq!(trains[1]["number"], "12954");
    assert_eq!(trains[1]["name"], "AK GOLD EXP");
    assert_eq!(trains[1]["departure_time"], "20:05");
    assert_eq!(trains[1]["arrival_time"], "10:10");
    assert_eq!(
        trains[1]["runs_on"],
        json!([true, false, true, false, true, false, false])
    );
    assert_eq!(trains[1]["runs_on"][1], false);

    assert_eq!(body["data_source"], "NTES");
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}
