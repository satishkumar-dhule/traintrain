mod common;

use serde_json::json;

use common::TestApp;

const AVAIL_PATH: &str = "/eticketing/protected/mapps1/altAvlEnq/TC";
const PAYTM_SEARCH_PATH: &str = "/api/trains/v5/search";

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

/// Mirrors the real travel.paytm.com search response (captured live,
/// trimmed to the fields the normalizer reads).
fn paytm_payload() -> serde_json::Value {
    json!({
        "error": null,
        "status": {"result": "success", "message": {"title": "Successful"}},
        "code": 200,
        "body": {
            "trains": [
                {
                    "departure": "2026-10-20T10:00:00+00:00",
                    "arrival": "2026-10-21T14:10:00+00:00",
                    "trainName": "GOA SMPRK K",
                    "trainNumber": "12449",
                    "source": "MAO",
                    "destination": "NDLS",
                    "source_name": "Madgaon",
                    "destination_name": "New Delhi",
                    "duration": "28:10",
                    "classes": ["SL", "3E", "3A", "2A", "1A"],
                    "train_type": "o",
                    "runs_on": {"text": "Runs on Tue, Wed"},
                    "availability": [
                        {
                            "code": "SL",
                            "name": "Sleeper Class",
                            "non_formatted_status": "GNWL82/WL59",
                            "status": "GNWL82/WL59",
                            "status_shortform": "WL 59",
                            "available_flag": "false",
                            "fare": 875,
                            "quota": "GN",
                            "pnr_prediction": {"value": 95}
                        },
                        {
                            "code": "3A",
                            "name": "AC 3 Tier",
                            "status": "AVAILABLE 0022",
                            "available_flag": true,
                            "fare": 2195,
                            "quota": "GN"
                        }
                    ]
                },
                {
                    "departure": "2026-10-20T07:20:00+00:00",
                    "arrival": "2026-10-20T13:45:00+00:00",
                    "trainName": "KONKAN KANYA EXP",
                    "trainNumber": "10111",
                    "source": "MAO",
                    "destination": "NDLS",
                    "source_name": "Madgaon",
                    "destination_name": "New Delhi",
                    "duration": "30:25",
                    "classes": ["1A", "2A", "3A"],
                    "train_type": "o",
                    "runs_on": {"text": "Runs on Mon, Tue, Wed, Thu, Fri, Sat, Sun"},
                    "availability": [
                        {
                            "code": "3A",
                            "name": "AC 3 Tier",
                            "status": "RAC 12",
                            "available_flag": false,
                            "fare": 1890
                        }
                    ]
                }
            ]
        },
        "meta": {"smartFilterTrainType": {"o": "Other Trains"}}
    })
}

#[tokio::test]
async fn missing_or_invalid_params_are_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/availability",
        "/rail-api/availability?src=MMCT",
        "/rail-api/availability?dst=NDLS",
        "/rail-api/availability?src=MMMMM&dst=NDLS",
        "/rail-api/availability?src=MMCT&dst=NDDDD",
        "/rail-api/availability?src=NDXX&dst=NDLS",
        "/rail-api/availability?src=MMCT&dst=MMCT",
        "/rail-api/availability?src=MMCT&dst=NDLS&date=not-a-date",
        "/rail-api/availability?src=MMCT&dst=NDLS&source=irctc123",
        // legacy alias must validate identically
        "/rail-api/irctc/availability?src=MMCT&dst=MMCT",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn paytm_returns_trains_with_class_status() {
    let app = TestApp::spawn().await;
    app.mocks["paytm"].route_json(PAYTM_SEARCH_PATH, paytm_payload());

    let (status, body) = app
        .get("/rail-api/availability?src=MAO&dst=NDLS&date=2026-10-20")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["src"], "MAO");
    assert_eq!(body["dst"], "NDLS");
    assert_eq!(body["date"], "2026-10-20");
    assert_eq!(body["data_source"], "Paytm");
    assert!(body["notice"]
        .as_str()
        .unwrap_or_default()
        .contains("Paytm"));

    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);

    let t = &trains[0];
    assert_eq!(t["number"], "12449");
    assert_eq!(t["name"], "GOA SMPRK K");
    assert_eq!(t["from_code"], "MAO");
    assert_eq!(t["from_name"], "Madgaon");
    assert_eq!(t["to_code"], "NDLS");
    assert_eq!(t["to_name"], "New Delhi");
    assert_eq!(t["departure_time"], "10:00");
    assert_eq!(t["arrival_time"], "14:10");
    assert_eq!(t["duration"], "28:10");
    assert_eq!(t["classes"], json!(["SL", "3E", "3A", "2A", "1A"]));
    assert_eq!(
        t["runs_on"],
        json!([false, true, true, false, false, false, false])
    );

    let avl = t["availability"].as_array().unwrap();
    assert_eq!(avl.len(), 2);
    assert_eq!(avl[0]["class"], "SL");
    assert_eq!(avl[0]["class_name"], "Sleeper Class");
    assert_eq!(avl[0]["status"], "GNWL82/WL59");
    assert_eq!(avl[0]["available"], json!(false));
    assert_eq!(avl[0]["fare"], json!(875));
    assert_eq!(avl[0]["quota"], "GN");
    assert_eq!(avl[0]["prediction"], json!(95));
    assert_eq!(avl[1]["class"], "3A");
    assert_eq!(avl[1]["status"], "AVAILABLE 0022");
    assert_eq!(avl[1]["available"], json!(true));

    // Daily train parses to all-true run days.
    assert_eq!(
        trains[1]["runs_on"],
        json!([true, true, true, true, true, true, true])
    );
    assert_eq!(trains[1]["availability"][0]["status"], "RAC 12");

    // The upstream call must have hit the Paytm search endpoint.
    let calls = app.mock("paytm").calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].0.starts_with(PAYTM_SEARCH_PATH));
}

#[tokio::test]
async fn explicit_paytm_source_failure_is_honest_502() {
    let app = TestApp::spawn().await;
    app.mocks["paytm"].route_error(PAYTM_SEARCH_PATH, axum::http::StatusCode::BAD_REQUEST);

    let (status, body) = app
        .get("/rail-api/availability?src=MAO&dst=NDLS&source=paytm")
        .await;
    assert_eq!(status, 502);
    assert!(body["error"].as_str().unwrap_or_default().contains("Paytm"));
    assert!(
        app.mock("irctc").calls().is_empty(),
        "explicit source must not fall back"
    );
}

#[tokio::test]
async fn auto_falls_back_to_irctc_when_paytm_fails() {
    let app = TestApp::spawn().await;
    app.mocks["paytm"].route_error(
        PAYTM_SEARCH_PATH,
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
    );
    app.mocks["irctc"].route_json(AVAIL_PATH, trains_payload());

    let (status, body) = app
        .get("/rail-api/availability?src=MMCT&dst=NDLS&date=2026-08-20")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["data_source"], "IRCTC");
    assert_eq!(body["trains"].as_array().unwrap().len(), 2);
    assert_eq!(app.mock("paytm").calls().len(), 1);
    // The IRCTC client also fires one session-bootstrap GET before the real
    // availability POST.
    assert!(app
        .mock("irctc")
        .calls()
        .iter()
        .any(|(path, _)| path.starts_with(AVAIL_PATH)));
}

#[tokio::test]
async fn explicit_irctc_skips_paytm_and_normalizes() {
    let app = TestApp::spawn().await;
    app.mocks["irctc"].route_json(AVAIL_PATH, trains_payload());

    let (status, body) = app
        .get("/rail-api/irctc/availability?src=MMCT&dst=NDLS&date=2026-08-20&source=irctc")
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
    assert!(
        trains[0].get("availability").is_none(),
        "IRCTC has no per-class status; field must be omitted"
    );
    assert_eq!(
        trains[1]["runs_on"],
        json!([true, false, true, false, true, false, false])
    );

    assert!(app.mock("paytm").calls().is_empty());
}

#[tokio::test]
async fn missing_date_defaults_to_today() {
    let app = TestApp::spawn().await;
    app.mocks["paytm"].route_json(PAYTM_SEARCH_PATH, paytm_payload());

    let (status, body) = app.get("/rail-api/availability?src=MAO&dst=NDLS").await;
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
    app.mocks["paytm"].route_json(PAYTM_SEARCH_PATH, paytm_payload());

    for date in ["20261020", "20-10-2026", "20/10/2026"] {
        let (status, body) = app
            .get(&format!(
                "/rail-api/availability?src=MAO&dst=NDLS&date={date}"
            ))
            .await;
        assert_eq!(status, 200, "date {date} should be accepted");
        assert_eq!(
            body["date"], "2026-10-20",
            "date {date} should echo normalized"
        );
    }
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/availability?src=MMCT&dst=NDLS").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}

/// Mirrors the live HYB→AL incident: Paytm answers HTTP 451 "There are no
/// direct trains running between these two stations…". The API must return
/// a clean 404 with a user-facing message — no upstream URLs, no merged
/// IRCTC outage blob — and must not bother the fallback source.
#[tokio::test]
async fn paytm_no_direct_trains_is_clean_404_without_fallback() {
    let app = TestApp::spawn().await;
    app.mocks["paytm"].route_raw_seq(
        PAYTM_SEARCH_PATH,
        vec![(
            axum::http::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            "application/json",
            json!({
                "status": {
                    "result": "failure",
                    "message": {"message": "There are no direct trains running between these two stations for your travel date. Please try an alternative route or a different date."}
                }
            })
            .to_string(),
        )],
    );

    let (status, body) = app
        .get("/rail-api/availability?src=HYB&dst=AL&date=2026-08-29")
        .await;
    assert_eq!(status, 404);
    let msg = body["error"].as_str().unwrap_or_default();
    assert!(
        msg.contains("No direct trains"),
        "clean message, got: {msg}"
    );
    assert!(!msg.contains("http"), "no URL noise, got: {msg}");
    assert!(
        !msg.contains("IRCTC"),
        "fallback outage details must not leak into the answer: {msg}"
    );
    assert!(
        app.mock("irctc").calls().is_empty(),
        "definitive no-trains must not trigger the IRCTC fallback"
    );
}
