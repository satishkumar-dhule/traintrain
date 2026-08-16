mod common;

use serde_json::json;

use common::TestApp;

const COMPOSITION_PATH: &str = "/online-charts/api/trainComposition";

fn composition_payload() -> serde_json::Value {
    json!({
        "trainData": {
            "trainNumber": "12951",
            "trainName": "MUMBAI RAJDHANI",
            "coachList": [
                {
                    "coachCode": "B1",
                    "classCode": "3A",
                    "berthList": [
                        {"berthNo": 1, "status": "vacant"},
                        {"berthNo": 2, "status": "occupied"},
                        {"berthNo": 3, "status": "not_reserved"}
                    ]
                },
                {"coachCode": "B2", "classCode": "3A", "berthList": []}
            ]
        }
    })
}

#[tokio::test]
async fn missing_or_invalid_params_are_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/irctc/chart",
        "/rail-api/irctc/chart?train=abc",
        "/rail-api/irctc/chart?train=123456789",
        "/rail-api/irctc/chart?train=12951&date=not-a-date",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn chart_returns_normalized_coaches() {
    let app = TestApp::spawn().await;
    app.mocks["irctc"].route_json(COMPOSITION_PATH, composition_payload());

    let (status, body) = app
        .get("/rail-api/irctc/chart?train=12951&date=2026-08-20&station=MMCT")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["train_number"], "12951");
    assert_eq!(body["train_name"], "MUMBAI RAJDHANI");
    assert_eq!(body["journey_date"], "2026-08-20");
    assert_eq!(body["boarding_station"], "MMCT");
    assert_eq!(body["data_source"], "IRCTC");
    assert!(body["notice"]
        .as_str()
        .unwrap_or_default()
        .contains("IRCTC"));

    let coaches = body["coaches"].as_array().unwrap();
    assert_eq!(coaches.len(), 2);
    assert_eq!(coaches[0]["code"], "B1");
    assert_eq!(coaches[0]["class_code"], "3A");
    let berths = coaches[0]["berths"].as_array().unwrap();
    assert_eq!(berths.len(), 3);
    assert_eq!(berths[0]["number"], 1);
    assert_eq!(berths[0]["status"], "vacant");
    assert_eq!(berths[1]["status"], "occupied");
    assert_eq!(berths[2]["status"], "not_reserved");
    assert_eq!(coaches[1]["berths"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn missing_date_and_station_are_optional() {
    let app = TestApp::spawn().await;
    app.mocks["irctc"].route_json(COMPOSITION_PATH, composition_payload());

    let (status, body) = app.get("/rail-api/irctc/chart?train=12951").await;
    assert_eq!(status, 200);
    assert!(
        chrono::NaiveDate::parse_from_str(
            body["journey_date"].as_str().unwrap_or_default(),
            "%Y-%m-%d"
        )
        .is_ok(),
        "journey_date should default to today (YYYY-MM-DD)"
    );
    assert_eq!(body["boarding_station"], serde_json::Value::Null);
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/irctc/chart?train=12951&date=2026-08-20&station=MMCT")
        .await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}
