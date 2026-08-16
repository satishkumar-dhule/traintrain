mod common;

use serde_json::json;

use common::TestApp;

const AVAIL_PATH: &str = "/eticketing/protected/mapps1/altAvlEnq/TC";

fn trains_payload() -> serde_json::Value {
    json!({
        "trainBtwnStnsList": [
            {
                "trainNumber": "12951",
                "trainName": "MUMBAI RAJDHANI",
                "fromStnCode": "MMCT",
                "fromStnName": "MUMBAI CENTRAL",
                "toStnCode": "NDLS",
                "toStnName": "NEW DELHI",
                "departureTime": "17:40",
                "arrivalTime": "08:32",
                "duration": "14:52",
                "distance": "1384",
                "avlClasses": ["3A", "2A"],
                "trainType": "SUF",
                "runDays": ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
            },
            {
                "trainNumber": "12009",
                "trainName": "SHATABDI EXP",
                "fromStnCode": "MMCT",
                "fromStnName": "MUMBAI CENTRAL",
                "toStnCode": "NDLS",
                "toStnName": "NEW DELHI",
                "departureTime": "05:40",
                "arrivalTime": "21:55",
                "duration": "16:15",
                "distance": "1384",
                "avlClasses": ["CC", "EC"],
                "trainType": "SHT",
                "runDays": ["MON", "WED", "FRI"]
            }
        ]
    })
}

#[tokio::test]
async fn missing_or_invalid_params_are_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/irctc/availability",
        "/rail-api/irctc/availability?src=MMCT",
        "/rail-api/irctc/availability?dst=NDLS",
        "/rail-api/irctc/availability?src=MM&dst=NDLS",
        "/rail-api/irctc/availability?src=MMCT&dst=ND",
        "/rail-api/irctc/availability?src=NDXX&dst=NDLS",
        "/rail-api/irctc/availability?src=MMCT&dst=MMCT",
        "/rail-api/irctc/availability?src=MMCT&dst=NDLS&date=not-a-date",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn availability_returns_normalized_trains() {
    let app = TestApp::spawn().await;
    app.mocks["irctc"].route_json(AVAIL_PATH, trains_payload());

    let (status, body) = app
        .get("/rail-api/irctc/availability?src=MMCT&dst=NDLS&date=2026-08-20")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["src"], "MMCT");
    assert_eq!(body["dst"], "NDLS");
    assert_eq!(body["date"], "2026-08-20");
    assert_eq!(body["data_source"], "IRCTC");
    assert!(body["notice"]
        .as_str()
        .unwrap_or_default()
        .contains("IRCTC"));

    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);

    assert_eq!(trains[0]["number"], "12951");
    assert_eq!(trains[0]["name"], "MUMBAI RAJDHANI");
    assert_eq!(trains[0]["from_code"], "MMCT");
    assert_eq!(trains[0]["from_name"], "MUMBAI CENTRAL");
    assert_eq!(trains[0]["to_code"], "NDLS");
    assert_eq!(trains[0]["departure_time"], "17:40");
    assert_eq!(trains[0]["arrival_time"], "08:32");
    assert_eq!(trains[0]["duration"], "14:52");
    assert_eq!(trains[0]["distance"], "1384");
    assert_eq!(trains[0]["classes"], json!(["3A", "2A"]));
    assert_eq!(trains[0]["train_type"], "SUF");
    assert_eq!(
        trains[0]["runs_on"],
        json!([true, true, true, true, true, true, true])
    );

    assert_eq!(trains[1]["number"], "12009");
    assert_eq!(
        trains[1]["runs_on"],
        json!([true, false, true, false, true, false, false])
    );
}

#[tokio::test]
async fn missing_date_defaults_to_today() {
    let app = TestApp::spawn().await;
    app.mocks["irctc"].route_json(AVAIL_PATH, trains_payload());

    let (status, body) = app
        .get("/rail-api/irctc/availability?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(status, 200);
    let date = body["date"].as_str().unwrap_or_default();
    assert!(
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok(),
        "date should be a YYYY-MM-DD today, got {date}"
    );
    assert_eq!(body["trains"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn accepts_compact_and_dmy_dates() {
    let app = TestApp::spawn().await;
    app.mocks["irctc"].route_json(AVAIL_PATH, trains_payload());

    for date in ["20260820", "20-08-2026", "20/08/2026"] {
        let (status, body) = app
            .get(&format!(
                "/rail-api/irctc/availability?src=MMCT&dst=NDLS&date={date}"
            ))
            .await;
        assert_eq!(status, 200, "date {date} should be accepted");
        assert_eq!(
            body["date"], "2026-08-20",
            "date {date} should echo normalized"
        );
    }
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/irctc/availability?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}
