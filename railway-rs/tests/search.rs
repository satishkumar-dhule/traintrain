mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;

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

/// CoRover is the primary origin for station search: when Ask DISHA answers,
/// its rows win verbatim (lat/lng mapping proves the corover provenance -
/// the local NDLS dataset row would carry different coordinates).
#[tokio::test]
async fn corover_is_primary_for_station_search() {
    let app = TestApp::spawn().await;
    app.mocks["corover"].route_json(
        "/dishaAPI/bot/searchStation/vashi",
        json!([
            {
                "name": "VASHI",
                "code": "VSH",
                "name_hi": "वाशी",
                "district": "Thane",
                "state": "Maharashtra",
                "trainCount": "42",
                "latitude": 19.077,
                "longitude": 72.999,
                "address": "Vashi, Navi Mumbai"
            },
            { "name": "SANPADA", "code": "SNPD" }
        ]),
    );

    let (status, body) = app.get("/rail-api/search/stations?q=vashi").await;
    assert_eq!(status, 200);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2, "corover rows served as-is: {body}");
    assert_eq!(arr[0]["code"], "VSH");
    assert_eq!(arr[0]["name"], "VASHI");
    assert_eq!(arr[0]["lat"], 19.077, "upstream latitude -> wire lat");
    assert_eq!(arr[0]["lng"], 72.999);
    assert_eq!(arr[0]["train_count"], "42");
    assert_eq!(
        arr[1],
        json!({"code": "SNPD", "name": "SANPADA"}),
        "absent optionals stay omitted on the wire"
    );
}

/// A failing CoRover upstream silently degrades to the pre-warmed local
/// dataset (same tiered ranking authority), still answering 200.
#[tokio::test]
async fn corover_failure_falls_back_to_local_dataset() {
    let app = TestApp::spawn().await;
    app.mocks["corover"].route_error(
        "/dishaAPI/bot/searchStation/",
        StatusCode::INTERNAL_SERVER_ERROR,
    );

    let (status, body) = app.get("/rail-api/search/stations?q=Varanasi").await;
    assert_eq!(status, 200);
    let codes: Vec<&str> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, vec!["BSB", "BCY", "BSBY"], "local authority wins");
}

/// An empty CoRover answer also falls through to the local dataset instead
/// of caching an empty result.
#[tokio::test]
async fn empty_corover_answer_falls_back_to_local_dataset() {
    let app = TestApp::spawn().await;
    app.mocks["corover"].route_json("/dishaAPI/bot/searchStation/new%20delhi", json!([]));

    let (status, body) = app.get("/rail-api/search/stations?q=new%20delhi").await;
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
