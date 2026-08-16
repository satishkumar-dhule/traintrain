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
    assert_eq!(origins.len(), 4);
    let names: Vec<&str> = origins
        .iter()
        .map(|o| o["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Railyatri", "etrain", "NTES", "IRCTC"]);
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

#[tokio::test]
async fn observability_includes_status_codes_cache_and_series() {
    let app = common::TestApp::spawn().await;

    // A prior request so the status distribution has data by the time we read it
    // (a request's own status is recorded only after its response is generated).
    app.get("/rail-api/stations?q=NDLS").await;

    let (status, body) = app.get("/rail-api/observability").await;
    assert_eq!(status, StatusCode::OK);

    // HTTP status distribution - the stations request above is a 200.
    let codes = body["status_codes"].as_array().unwrap();
    assert!(codes
        .iter()
        .any(|c| c["code"] == 200 && c["count"].as_u64().unwrap() >= 1));

    // Cache stats shape (counters may be 0 in tests - the shape must hold).
    let cache = &body["cache"];
    assert!(cache["hits"].as_u64().is_some());
    assert!(cache["misses"].as_u64().is_some());
    assert!(cache["hit_rate"].as_f64().is_some());
    assert!(cache["entries"].as_u64().is_some());

    // Time-series shape: column arrays, all aligned with `times`.
    let series = &body["series"];
    assert!(series["times"].is_array());
    assert!(series["rps"].is_array());
    assert!(series["latency_ms"].is_array());
    assert!(series["mem_mb"].is_array());
    assert!(series["cpu_frac"].is_array());
    assert!(series["in_flight"].is_array());
    assert!(series["sources"].is_array());

    // Recent logs are a JSON array of structured records.
    assert!(body["logs"].is_array());
}

#[tokio::test]
async fn metrics_endpoint_serves_prometheus_text_format() {
    let app = common::TestApp::spawn().await;

    // A prior request so the exporter has recorded a completed request by the
    // time we scrape (a request's own status is recorded only after its
    // response is generated).
    app.get("/healthz").await;

    let (status, text) = app.get_raw("/metrics").await;
    assert_eq!(status, StatusCode::OK);

    assert!(text.contains("railway_http_requests_total"));
    assert!(text.contains("railway_http_duration_seconds_bucket"));
    assert!(text.contains("railway_process_uptime_seconds"));
    assert!(text.contains("railway_http_in_flight"));
    assert!(text.contains("railway_cache_entries"));
    assert!(text.contains("railway_http_requests_per_second"));
    assert!(text.contains(
        "railway_http_requests_total{method=\"GET\",path=\"/healthz\",status=\"200\"} 1"
    ));
}

#[tokio::test]
async fn logs_endpoint_returns_structured_records() {
    let app = common::TestApp::spawn().await;

    let (status, body) = app.get("/rail-api/logs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["logs"].is_array());

    let (status, body) = app.get("/rail-api/logs?limit=5&level=error").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["logs"].is_array());
    assert!(body["limit"] == 5);
}

#[tokio::test]
async fn logs_endpoint_caps_limit() {
    let app = common::TestApp::spawn().await;

    let (status, body) = app.get("/rail-api/logs?limit=100000").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["limit"] == 500);
}
