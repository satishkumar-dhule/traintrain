mod common;

use axum::http::StatusCode;
use serde_json::json;

use common::TestApp;

/// Route the four official IR enquiry endpoints on the `ir` mock.
fn route_ir_session(app: &TestApp) {
    app.mock("ir").route_html(
        "/enquiry/PNR/PnrEnquiry.html",
        "<html>IR PNR Enquiry</html>",
    );
    app.mock("ir")
        .route_json("/enquiry/CaptchaConfig", json!("1"));
    app.mock("ir")
        .route_html("/enquiry/captchaDraw.png", "<png>captcha</png>");
}

const PNR: &str = "8456789012";

fn pnr_url() -> String {
    format!("/rail-api/pnr?pnr={PNR}")
}

fn pnr_retry_url(session_id: &str, text: &str) -> String {
    format!(
        "/rail-api/pnr?pnr={PNR}&captcha_session={}&captcha_text={}&captcha_source=Indian%20Railways",
        urlencoding::encode(session_id),
        urlencoding::encode(text),
    )
}

#[tokio::test]
async fn invalid_pnr_rejected() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/pnr?pnr=123").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("10-digit"));
}

#[tokio::test]
async fn first_request_is_captcha_challenge() {
    let app = TestApp::spawn().await;
    route_ir_session(&app);
    let (status, body) = app.get(&pnr_url()).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(body["error"], "captcha_required");
    assert_eq!(body["source"], "Indian Railways");
    assert!(
        body["image"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"),
        "image must be a data URI: {body}"
    );
    assert!(!body["session_id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn valid_pnr_after_captcha_returns_normalized_response() {
    let app = TestApp::spawn().await;
    route_ir_session(&app);
    app.mock("ir").route_json(
        "/enquiry/CommonCaptcha",
        json!({
            "flag": "YES",
            "pnrNumber": PNR,
            "trainNumber": "12951",
            "trainName": "MUMBAI RAJDHANI",
            "dateOfJourney": "13-08-2026",
            "sourceStation": "MMCT - MUMBAI CENTRAL",
            "destinationStation": "NDLS - NEW DELHI",
            "reservationUpto": "NDLS - NEW DELHI",
            "boardingPoint": "MMCT",
            "journeyClass": "3A",
            "bookingFare": "2465",
            "chartStatus": "Chart Prepared",
            "isWL": "N",
            "informationMessage": ["", ""],
            "generatedTimeStamp": {"year": 2026, "month": 8, "day": 13, "hour": 10, "minute": 5, "second": 0},
            "passengerList": [{
                "passengerSerialNumber": 1,
                "bookingStatus": "CNF",
                "bookingCoachId": "B2",
                "bookingBerthNo": 47,
                "bookingBerthCode": "LB",
                "currentStatus": "CNF",
                "currentCoachId": "B2",
                "currentBerthNo": 47,
                "currentBerthCode": "LB",
                "passengerQuota": "GN"
            }]
        }),
    );

    let (status, body) = app.get(&pnr_url()).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    let session_id = body["session_id"].as_str().unwrap().to_string();

    let (status, body) = app.get(&pnr_retry_url(&session_id, "48201")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["pnr"], PNR);
    assert_eq!(body["train_number"], "12951");
    assert_eq!(body["train_name"], "MUMBAI RAJDHANI");
    assert_eq!(body["journey_date"], "2026-08-13");
    assert_eq!(body["from"]["code"], "MMCT");
    assert_eq!(body["from"]["name"], "MUMBAI CENTRAL");
    assert_eq!(body["to"]["code"], "NDLS");
    assert_eq!(body["to"]["name"], "NEW DELHI");
    assert_eq!(body["passengers"][0]["booking_status"], "CNF/B2/47/LB/GN");
    assert_eq!(body["passengers"][0]["current_status"], "CNF/B2/47");
    assert_eq!(body["passengers"][0]["coach"], "B2");
    assert_eq!(body["passengers"][0]["berth"], "47");
    assert_eq!(body["data_source"], "Indian Railways");
    assert_eq!(body["freshness"], "live");
    assert!(body["notice"].as_str().unwrap().contains("Chart Prepared"));
    assert_eq!(body["last_updated"], "2026-08-13T10:05:00+05:30");
}

#[tokio::test]
async fn wrong_captcha_reissues_fresh_challenge() {
    let app = TestApp::spawn().await;
    route_ir_session(&app);
    app.mock("ir").route_json(
        "/enquiry/CommonCaptcha",
        json!({"errorMessage": "Captcha not matched", "serverId": "appserver", "generatedTimeStamp": {"year": 2026, "month": 8, "day": 13, "hour": 10, "minute": 5, "second": 0}}),
    );

    let (status, body) = app.get(&pnr_url()).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    let first_session = body["session_id"].as_str().unwrap().to_string();

    let (status, body) = app.get(&pnr_retry_url(&first_session, "99999")).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(body["error"], "captcha_required");
    let second_session = body["session_id"].as_str().unwrap().to_string();
    assert_ne!(second_session, first_session);
}

#[tokio::test]
async fn invalid_pnr_after_captcha_is_404() {
    let app = TestApp::spawn().await;
    route_ir_session(&app);
    app.mock("ir").route_json(
        "/enquiry/CommonCaptcha",
        json!({"errorMessage": "PNR No. is not valid", "serverId": "appserver", "generatedTimeStamp": {"year": 2026, "month": 8, "day": 13, "hour": 10, "minute": 5, "second": 0}}),
    );

    let (status, body) = app.get(&pnr_url()).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    let session_id = body["session_id"].as_str().unwrap().to_string();

    let (status, body) = app.get(&pnr_retry_url(&session_id, "12345")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains(PNR));
}

#[tokio::test]
async fn expired_or_unknown_session_reissues_challenge() {
    let app = TestApp::spawn().await;
    route_ir_session(&app);
    let (status, body) = app.get(&pnr_retry_url("bogus-session", "12345")).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(body["error"], "captcha_required");
}

#[tokio::test]
async fn empty_captcha_text_is_400() {
    let app = TestApp::spawn().await;
    route_ir_session(&app);
    let (status, body) = app.get(&pnr_retry_url("bogus-session", "")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("captcha_text"));
}

#[tokio::test]
async fn unreachable_source_is_502() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get(&pnr_url()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body["error"].as_str().unwrap().contains("unavailable"));
}

#[tokio::test]
async fn unexpected_shape_is_502() {
    let app = TestApp::spawn().await;
    route_ir_session(&app);
    app.mock("ir")
        .route_json("/enquiry/CommonCaptcha", json!({}));

    let (status, body) = app.get(&pnr_url()).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    let session_id = body["session_id"].as_str().unwrap().to_string();

    let (status, body) = app.get(&pnr_retry_url(&session_id, "12345")).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body["error"].as_str().unwrap().contains("shape"));
}
