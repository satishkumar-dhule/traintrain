mod common;

use common::TestApp;

#[tokio::test]
async fn healthz_returns_ok() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/healthz").await;
    assert_eq!(status, 200);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "railway-rs");
}

#[tokio::test]
async fn api_healthz_returns_ok() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/api/healthz").await;
    assert_eq!(status, 200);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn source_status_reports_live_sources() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/source-status").await;
    assert_eq!(status, 200);
    assert_eq!(body["live_enabled"], true);
    assert_eq!(body["mode"], "live");
    let sources = body["sources"].as_array().unwrap();
    assert!(sources.iter().any(|s| s["name"] == "Railyatri"));
    assert!(sources.iter().any(|s| s["name"] == "NTES"));
}

#[tokio::test]
async fn unknown_route_falls_back_to_static() {
    let app = TestApp::spawn().await;
    let (status, _) = app.get_raw("/rail-api/nope").await;
    // unmatched api route -> 404 (no fallback index.html for /rail-api paths is
    // configured; ServeDir serves files or index.html fallback for asset paths)
    assert_eq!(status, 404);
}
