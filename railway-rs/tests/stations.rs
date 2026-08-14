mod common;

use common::TestApp;

#[tokio::test]
async fn stations_search_by_code() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/stations?q=NDLS").await;
    assert_eq!(status, 200);
    let arr = body.as_array().unwrap();
    let ndls = arr
        .iter()
        .find(|s| s["code"] == "NDLS")
        .expect("NDLS in results");
    assert!(!ndls["name"].as_str().unwrap_or_default().is_empty());
}

#[tokio::test]
async fn stations_empty_query_returns_empty() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/stations?q=").await;
    assert_eq!(status, 200);
    assert_eq!(body, serde_json::json!([]));
}

#[tokio::test]
async fn stations_no_match_returns_empty() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/stations?q=zzzznotfound").await;
    assert_eq!(status, 200);
    assert_eq!(body, serde_json::json!([]));
}
