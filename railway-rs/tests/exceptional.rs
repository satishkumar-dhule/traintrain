mod common;

use common::TestApp;

#[tokio::test]
async fn bad_type_is_rejected() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/exceptional?type=foo").await;
    assert_eq!(status, 400);
    assert_eq!(
        body["error"],
        "type must be one of: cancelled, rescheduled, diverted"
    );
}

#[tokio::test]
async fn missing_type_is_rejected() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/exceptional").await;
    assert_eq!(status, 400);
    assert_eq!(
        body["error"],
        "type must be one of: cancelled, rescheduled, diverted"
    );
}

#[tokio::test]
async fn json_list_maps_to_trains() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].route_json(
        "/q",
        serde_json::json!({"list": [{"trainNo":"02951","trainName":"MUMBAI RAJDHANI SPL","date":"2026-08-13","reason":"Track maintenance"}]}),
    );
    let (status, body) = app.get("/rail-api/ntes/exceptional?type=cancelled").await;
    assert_eq!(status, 200);
    assert_eq!(body["type"], "cancelled");
    assert_eq!(body["trains"].as_array().unwrap().len(), 1);
    assert_eq!(body["trains"][0]["number"], "02951");
    assert_eq!(body["trains"][0]["name"], "MUMBAI RAJDHANI SPL");
    assert_eq!(body["trains"][0]["date"], "2026-08-13");
    assert_eq!(body["trains"][0]["reason"], "Track maintenance");
    assert_eq!(body["data_source"], "NTES");
}

#[tokio::test]
async fn non_json_html_is_honest_source_unavailable() {
    // The `/q` contract is JSON-only: `NtesWebClient::post_form` refuses a
    // non-JSON body with `AppError::SourceUnavailable`, so an HTML fragment is
    // not guessed at - the client surfaces 502.
    let app = TestApp::spawn().await;
    app.mocks["ntes"].route_html(
        "/q",
        "<table><tr><td>02952</td><td>SPL EXP</td><td>2026-08-13</td><td>line block</td></tr></table>",
    );
    let (status, body) = app.get("/rail-api/ntes/exceptional?type=cancelled").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/exceptional?type=diverted").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}
