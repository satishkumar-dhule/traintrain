mod common;

use common::TestApp;

const EXCP_HTML: &str = r#"<table>
<tr><th colspan="4">Exceptional Trains</th></tr>
<tr><td>02951</td><td>MUMBAI RAJDHANI SPL</td><td>2026-08-13</td><td>Track maintenance</td></tr>
</table>"#;

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
async fn html_table_maps_to_trains() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(EXCP_HTML);
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
async fn shell_page_without_exception_table_is_honest_source_unavailable() {
    // The web client only trusts a page that carries a parseable exception
    // table; a nav shell (or any HTML fragment without train rows) is not
    // guessed at - the client surfaces 502.
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web("<table><tr><th>No data</th></tr></table>");
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
