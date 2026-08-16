mod common;

use common::TestApp;

#[tokio::test]
async fn train_search_by_number() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/search/trains?q=12951").await;
    assert_eq!(status, 200);
    let trains = body.as_array().unwrap();
    assert!(trains.iter().any(|t| t["number"] == "12951"));
}

#[tokio::test]
async fn train_search_by_name_is_case_insensitive() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/search/trains?q=MUMBAI%20RAJDHANI").await;
    assert_eq!(status, 200);
    let trains = body.as_array().unwrap();
    assert!(trains.iter().any(|t| t["name"]
        .as_str()
        .unwrap()
        .to_uppercase()
        .contains("RAJDHANI")));
}

#[tokio::test]
async fn station_search_by_code() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/search/stations?q=NDLS").await;
    assert_eq!(status, 200);
    let stations = body.as_array().unwrap();
    assert!(stations.iter().any(|s| s["code"] == "NDLS"));
}

#[tokio::test]
async fn empty_query_returns_empty_array() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/search/trains",
        "/rail-api/search/trains?q=",
        "/rail-api/search/stations",
        "/rail-api/search/suggest",
        "/rail-api/search/suggest?q=",
    ] {
        let (status, body) = app.get(path).await;
        assert_eq!(status, 200);
        assert_eq!(body.as_array().unwrap().len(), 0);
    }
}

#[tokio::test]
async fn suggest_returns_station_and_train_hits() {
    let app = TestApp::spawn().await;

    let (status, body) = app.get("/rail-api/search/suggest?q=12951").await;
    assert_eq!(status, 200);
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty(), "expected suggestions for 12951");
    assert!(arr
        .iter()
        .any(|s| s["type"] == "train" && s["number"] == "12951"));

    let (status, body) = app.get("/rail-api/search/suggest?q=NDLS").await;
    assert_eq!(status, 200);
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty(), "expected suggestions for NDLS");
    assert!(arr
        .iter()
        .any(|s| s["type"] == "station" && s["code"] == "NDLS"));
}

#[tokio::test]
async fn suggest_matches_train_name_and_number_ranked() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/search/suggest?q=MUMBAI%20RAJDHANI")
        .await;
    assert_eq!(status, 200);
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty(), "expected suggestions for MUMBAI RAJDHANI");
    let top = &arr[0];
    assert!(
        top["type"] == "train",
        "train hit should outrank partial stations"
    );
    assert!(top["name"]
        .as_str()
        .unwrap()
        .to_uppercase()
        .contains("RAJDHANI"));
}
