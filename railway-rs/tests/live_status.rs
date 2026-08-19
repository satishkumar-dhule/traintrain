mod common;

use axum::http::StatusCode;

use common::TestApp;

fn live_fixture() -> String {
    std::fs::read_to_string("testdata/ry_live_12951.html").unwrap()
}

/// Real NTES "Spot Your Train" popup captured for 12055 (active run 14-Aug).
fn ntes_spot_train_fixture() -> String {
    std::fs::read_to_string("testdata/ntes_spot_train_12055.html").unwrap()
}

/// The nav-shell NTES serves for an unknown train number (no run instances).
fn ntes_spot_train_unknown_fixture() -> String {
    std::fs::read_to_string("testdata/ntes_spot_train_unknown.html").unwrap()
}

fn mock_12951(app: &TestApp) {
    app.mock("railyatri")
        .route_html("/live-train-status/12951", live_fixture());
}

fn mock_12055_spot_train(app: &TestApp) {
    let m = &app.mocks["ntes"];
    m.route_html("/mntes/", "<html><head><title>NTES</title></head></html>");
    m.route_html(
        "/mntes/GetCSRFToken",
        "<input type='hidden' name='csrfToken' value='tok123'>",
    );
    m.route_html("/mntes/tr", ntes_spot_train_fixture());
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
    app.mocks["ntes"].route_html("/mntes/tr", ntes_spot_train_unknown_fixture());
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
async fn past_date_is_rejected_on_fallback_path() {
    let app = TestApp::spawn().await;
    mock_12951(&app);

    let (status, body) = app
        .get("/rail-api/live-status?train=12951&date=2020-01-01")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("today"));
}

#[tokio::test]
async fn past_date_is_rejected_on_ntes_path() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);

    let (status, body) = app
        .get("/rail-api/live-status?train=12055&date=2020-01-01")
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("today"));
}

#[tokio::test]
async fn ntes_is_primary_source_when_reachable() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);

    // Railyatri has no mock route, so a 200 with data_source NTES proves the
    // gov source was used first and no fallback happened.
    let (status, body) = app.get("/rail-api/live-status?train=12055").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["train_number"], "12055");
    assert_eq!(body["train_name"], "DDN JANSHTBDI");
    assert_eq!(body["data_source"], "NTES");
    assert_eq!(body["train_start_date"], "14-Aug-2026");

    // All run dates NTES reports for the train are surfaced, like the NTES
    // "Spot Train (Live Status)" page shows its "Train Instances". Per-run
    // timelines are resolved by the `?date=` switch (see
    // `ntes_run_instance_can_be_switched_by_date`).
    let instances = body["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 5);
    assert_eq!(instances[1]["start_date"], "14-Aug-2026");
    assert!(instances[1]["position"]
        .as_str()
        .unwrap()
        .contains("Departed from GHAZIABAD(GZB)"));
    assert_eq!(instances[0]["start_date"], "15-Aug-2026");

    let location = body["current_location_info"].as_str().unwrap();
    assert!(
        location.contains("MEERUT CITY"),
        "next station drives the location text: {location}"
    );

    let stations = body["stations"].as_array().unwrap();
    assert_eq!(stations.len(), 9);
    assert_eq!(stations[0]["status"], "departed");
    assert_eq!(stations[1]["status"], "departed");
    assert_eq!(stations[2]["status"], "expected", "MEERUT CITY is next");
    assert_eq!(stations[2]["code"], "MTC");
    assert_eq!(stations[8]["status"], "scheduled");

    // NTES real per-stop actuals are surfaced verbatim and drive honest delay.
    assert_eq!(stations[1]["code"], "GZB");
    assert_eq!(stations[1]["actual_arrival"], "15:56");
    assert_eq!(stations[1]["delay_minutes"], 3, "badge delay from NTES");
    assert_eq!(
        stations[2]["actual_arrival"], "",
        "not reached -> no actual"
    );
}

#[tokio::test]
async fn ntes_run_instance_can_be_switched_by_date() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);

    // Switch to the completed 13-Aug run: its own real arrivals surface and
    // the train is reported at destination.
    let (status, body) = app
        .get("/rail-api/live-status?train=12055&date=2026-08-13")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data_source"], "NTES");
    assert_eq!(body["train_start_date"], "13-Aug-2026");
    assert!(
        body["current_location_info"]
            .as_str()
            .unwrap()
            .contains("Arrived at DEHRADOON"),
        "completed run reports arrival: {}",
        body["current_location_info"].as_str().unwrap()
    );
    let stations = body["stations"].as_array().unwrap();
    assert_eq!(stations.len(), 9);
    assert!(
        stations.iter().all(|s| s["status"] == "departed"),
        "every stop of a completed run is departed"
    );
    assert_eq!(stations[8]["actual_arrival"], "21:23");
    assert_eq!(stations[8]["delay_minutes"], 18);

    // Switch to the not-yet-started 15-Aug run: still at origin, everything
    // scheduled and no actuals invented.
    let (status, body) = app
        .get("/rail-api/live-status?train=12055&date=2026-08-15")
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["train_start_date"], "15-Aug-2026");
    assert!(body["current_location_info"]
        .as_str()
        .unwrap()
        .contains("Train at NEW DELHI (origin)."));
    let stations = body["stations"].as_array().unwrap();
    assert_eq!(
        stations[0]["status"], "expected",
        "origin waits for departure"
    );
    assert!(stations.iter().skip(1).all(|s| s["status"] == "scheduled"));
    assert!(stations.iter().all(|s| s["actual_arrival"] == ""));
}

#[tokio::test]
async fn ntes_web_form_is_posted_with_query_and_csrf() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);
    app.mocks["ntes"].clear_calls();

    let (status, _body) = app.get("/rail-api/live-status?train=12055").await;
    assert_eq!(status, StatusCode::OK);

    let calls = app.mocks["ntes"].calls();
    let paths: Vec<&str> = calls.iter().map(|(p, _)| p.as_str()).collect();
    // The old mobile API must not be consulted for live status any more.
    assert!(
        !paths.iter().any(|p| p.contains("AppServAnd")),
        "mobile API must not be used: {paths:?}"
    );
    let tr_call = calls
        .iter()
        .find(|(p, _)| p == "/mntes/tr")
        .expect("spot-train web form must be posted");
    assert!(tr_call.0.starts_with("/mntes/tr"));
    assert!(
        tr_call.1.contains("lan=en"),
        "form body must carry the language: {}",
        tr_call.1
    );
    assert!(
        tr_call.1.contains("csrfToken=tok123"),
        "form body must carry the CSRF token: {}",
        tr_call.1
    );
}

#[tokio::test]
async fn ntes_failure_falls_back_to_railyatri() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].route_error("/mntes/tr", StatusCode::INTERNAL_SERVER_ERROR);
    mock_12951(&app);

    let (status, body) = app.get("/rail-api/live-status?train=12951").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data_source"], "Railyatri");
    assert_eq!(body["train_number"], "12951");
}

#[tokio::test]
async fn ntes_instances_carry_their_own_stops() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);

    let (_status, body) = app.get("/rail-api/live-status?train=12055").await;
    let instances = body["instances"].as_array().unwrap();
    assert!(
        instances.len() >= 2,
        "fixture must have at least two instances"
    );

    // Every instance reported by NTES must carry its own `stops` array so the
    // frontend can render tabs client-side without re-fetching.
    for (i, inst) in instances.iter().enumerate() {
        let stops = inst.get("stops").and_then(|v| v.as_array());
        assert!(
            stops.is_some_and(|s| !s.is_empty()),
            "instance[{i}] ({}) must have non-empty stops",
            inst["start_date"].as_str().unwrap_or("?"),
        );
        let first = &stops.unwrap()[0];
        assert!(
            first.get("name").and_then(|v| v.as_str()).is_some(),
            "each stop must have a name"
        );
        assert!(
            first.get("status").and_then(|v| v.as_str()).is_some(),
            "each stop must have a status"
        );
    }
}

#[tokio::test]
async fn both_sources_down_is_502_naming_each() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].route_error("/mntes/tr", StatusCode::INTERNAL_SERVER_ERROR);
    app.mock("railyatri").route_error(
        "/live-train-status/77777",
        StatusCode::INTERNAL_SERVER_ERROR,
    );

    let (status, body) = app.get("/rail-api/live-status?train=77777").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let error = body["error"].as_str().unwrap();
    assert!(error.contains("NTES"), "error names NTES: {error}");
    assert!(
        error.contains("Railyatri"),
        "error names Railyatri: {error}"
    );
}
