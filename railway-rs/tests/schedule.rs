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

    // No corover / ntes mock routes: both fail upstream and the Railyatri
    // fixture wins as the final fallback of the chain.
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
async fn ntes_fallback_serves_when_corover_unreachable() {
    let app = TestApp::spawn().await;
    let payload = r#"{"trainNo":"12951","trainName":"MUMBAI RAJDHANI","trainScheduleList":[{"stationCode":"MMCT","stationName":"MUMBAI CENTRAL","arrivalTime":"--","departureTime":"17:40","day":1,"stopNumber":1},{"stationCode":"NDLS","stationName":"NEW DELHI","arrivalTime":"08:32","departureTime":"--","day":2,"stopNumber":2}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({ "jsonIn": NtesCrypto::build(payload) }),
    );

    // CoRover has no mock route (primary fails upstream), so a 200 with
    // data_source NTES proves the first fallback answered.
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
async fn corover_and_ntes_failures_fall_back_to_railyatri() {
    let app = TestApp::spawn().await;
    let fixture = std::fs::read_to_string("testdata/ry_schedule_12951.html").unwrap();
    app.mocks["railyatri"].route_html("/time-table/12951", fixture);

    // No corover / ntes mock routes -> both fail -> Railyatri wins.
    let (status, body) = app.get("/rail-api/schedule?train=12951").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data_source"], "Railyatri");
    assert!(body["stops"].as_array().is_some_and(|s| s.len() > 100));
}

#[tokio::test]
async fn all_sources_down_is_bad_gateway_mentioning_all() {
    let app = TestApp::spawn().await;
    app.mocks["railyatri"].route_error("/time-table/1", StatusCode::INTERNAL_SERVER_ERROR);
    app.mocks["corover"].route_error(
        "/dishaAPI/bot/trnscheduleEnq/1",
        StatusCode::INTERNAL_SERVER_ERROR,
    );

    let (status, body) = app.get("/rail-api/schedule?train=1").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let err = body["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("CoRover"),
        "error should mention CoRover: {err}"
    );
    assert!(err.contains("NTES"), "error should mention NTES: {err}");
    assert!(
        err.contains("Railyatri"),
        "error should mention Railyatri: {err}"
    );
}

#[tokio::test]
async fn corover_is_primary_source_with_distance_and_day() {
    let app = TestApp::spawn().await;

    // Only the Ask DISHA `trnscheduleEnq` route is registered: as primary it
    // must win outright, normalizing into the same wire shape (real captured
    // upstream body from the live fixture).
    let fixture = std::fs::read_to_string("testdata/askdisha/schedule_12951.json").unwrap();
    let payload: serde_json::Value = serde_json::from_str(&fixture).unwrap();
    app.mocks["corover"].route_json("/dishaAPI/bot/trnscheduleEnq/12951", payload);

    let (status, body) = app.get("/rail-api/schedule?train=12951").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data_source"], "CoRover");
    assert_eq!(body["source"], "CoRover");
    assert_eq!(body["train_number"], "12951");
    // Run-day flags surface in the Railyatri spelling.
    let run_days = body["running_days"].as_array().unwrap();
    assert_eq!(run_days.len(), 7);
    assert!(run_days.iter().any(|d| d == "MON"));

    let stops = body["stops"].as_array().unwrap();
    assert_eq!(stops.len(), 8);
    assert_eq!(stops[0]["code"], "MMCT");
    assert_eq!(stops[0]["distance_km"], 0.0);
    assert_eq!(stops[0]["day"], 1);
    assert_eq!(stops[4]["distance_km"], 653.0);
    assert_eq!(stops[4]["day"], 2);

    // Primary means primary: no fallback source was contacted at all.
    assert!(
        app.mocks["ntes"].calls().is_empty(),
        "NTES must not be called when CoRover answers"
    );
    assert!(
        app.mocks["railyatri"].calls().is_empty(),
        "Railyatri must not be called when CoRover answers"
    );

    // Cache key is shared with the other sources.
    assert!(app.state.cache.get("schedule:12951").is_some());
}

#[tokio::test]
async fn stale_upstream_route_falls_through_to_index_matching_source() {
    let app = TestApp::spawn().await;

    // CoRover still serves 77608 under its pre-renumbering identity: a
    // MEDCHAL-SECUNDERABAD DEMU whose route never touches AKOT/AK - exactly
    // what the timetable index ("AKOT-AKOLA PASSENGER") rules out.
    let corover_payload = json!({
        "trainNumber": "77608",
        "trainName": "MEDCHAL - SECUNDERABAD DEMU",
        "stationFrom": "MEDCHAL",
        "stationTo": "SECUNDERABAD JN",
        "trainRunsOnMon": true,
        "trainRunsOnTue": true,
        "trainRunsOnWed": true,
        "trainRunsOnThu": true,
        "trainRunsOnFri": true,
        "trainRunsOnSat": true,
        "trainRunsOnSun": false,
        "errorMessage": null,
        "serverId": "test",
        "timeStamp": "2026-08-24T00:00:00Z",
        "stationList": [
            {"stationCode":"MED","stationName":"MEDCHAL","arrivalTime":"--","departureTime":"11:35","routeNumber":"1","haltTime":"--","distance":"0","dayCount":"1","stnSerialNumber":"1"},
            {"stationCode":"SC","stationName":"SECUNDERABAD JN","arrivalTime":"12:35","departureTime":"--","routeNumber":"1","haltTime":"--","distance":"221","dayCount":"1","stnSerialNumber":"2"}
        ]
    });
    app.mocks["corover"].route_json("/dishaAPI/bot/trnscheduleEnq/77608", corover_payload);

    // NTES knows the real AKOT-AKOLA shuttle.
    let ntes_payload = r#"{"trainNo":"77608","trainName":"AKOT-AKOLA PASSENGER","trainScheduleList":[{"stationCode":"AKOT","stationName":"AKOT","arrivalTime":"--","departureTime":"09:00","day":1,"stopNumber":1},{"stationCode":"AK","stationName":"AKOLA JN","arrivalTime":"10:10","departureTime":"--","day":1,"stopNumber":2}]}"#;
    app.mocks["ntes"].route_json(
        "/crisns/AppServAnd",
        json!({ "jsonIn": NtesCrypto::build(ntes_payload) }),
    );

    let (status, body) = app.get("/rail-api/schedule?train=77608").await;
    assert_eq!(status, StatusCode::OK);
    // The conflicting primary was rejected; the index-matching source wins.
    assert_eq!(body["data_source"], "NTES");
    assert_eq!(body["train_name"], "AKOT-AKOLA PASSENGER");
    let stops = body["stops"].as_array().unwrap();
    assert_eq!(stops[0]["code"], "AKOT");
    assert_eq!(stops.last().unwrap()["code"], "AK");
    // Proves the primary was genuinely consulted before being discarded.
    assert_eq!(app.mocks["corover"].calls().len(), 1);
}
