mod common;

use axum::http::StatusCode;

#[tokio::test]
async fn observability_returns_real_metrics() {
    let app = common::TestApp::spawn().await;

    let (status, body) = app.get("/rail-api/observability").await;
    assert_eq!(status, StatusCode::OK);

    let cpu = body["cpu_usage"].as_f64().unwrap();
    let total = body["requests_total"].as_u64().unwrap();

    assert!(cpu >= 0.0);
    assert!(
        total >= 1,
        "metrics middleware must count the observability request itself"
    );

    let origins = body["origins"].as_array().unwrap();
    assert_eq!(origins.len(), 3);
    let names: Vec<&str> = origins
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Railyatri", "etrain", "NTES"]);
    for origin in origins {
        assert!(origin["latency"].as_u64().is_some());
        assert!(origin["status"].as_str().is_some());
    }
}

#[tokio::test]
async fn observability_top_paths_are_pairs() {
    let app = common::TestApp::spawn().await;

    let (status, body) = app.get("/rail-api/observability").await;
    assert_eq!(status, StatusCode::OK);

    let top = body["top_paths"].as_array().unwrap();
    for pair in top {
        assert_eq!(pair.as_array().unwrap().len(), 2);
        assert!(pair[0].as_str().is_some());
        assert!(pair[1].as_u64().is_some());
    }
}
