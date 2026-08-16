mod common;

use axum::http::StatusCode;

use common::TestApp;

const AVAIL_PATH: &str = "/eticketing/protected/mapps1/altAvlEnq/TC";

/// One train as the NTES trains-between web form renders it.
const TBS_HTML: &str = r#"<table>
<tr class="w3-round">
  <td colspan=3>
    <span><b>12951</b>&nbsp;&nbsp;MUMBAI RAJDHANI</span><br>
    <span>Daily | Superfast</span>
    <span class="w3-round w3-blue" onclick="onTrainStatus('12951',document.getElementsByName('frmTBS')[0],'')">See Train Status >></span>
    <span style="text-align: left;width: 25%;"><b>17:40</b><br>Mumbai Central<br>MMCT</span>
    <span style="text-align: right; width: 25%;"><b>08:32</b><br>New Delhi<br><b>NDLS</b></span>
  </td>
</tr>
</table>"#;

#[tokio::test]
async fn trains_between_falls_back_to_irctc_when_ntes_down() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].route_error("/mntes/q", StatusCode::INTERNAL_SERVER_ERROR);
    app.mocks["irctc"].route_json(
        AVAIL_PATH,
        serde_json::json!({
            "trainBtwnStnsList": [
                {
                    "trainNumber": "12951",
                    "trainName": "MUMBAI RAJDHANI",
                    "departureTime": "17:40",
                    "arrivalTime": "08:32",
                    "runDays": ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
                },
                {
                    "trainNumber": "12009",
                    "trainName": "SHATABDI EXP",
                    "departureTime": "05:40",
                    "arrivalTime": "21:55",
                    "runDays": ["MON", "WED", "FRI"]
                }
            ]
        }),
    );

    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["data_source"], "IRCTC");

    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);
    assert_eq!(trains[0]["number"], "12951");
    assert_eq!(trains[0]["name"], "MUMBAI RAJDHANI");
    assert_eq!(trains[0]["departure_time"], "17:40");
    assert_eq!(trains[0]["arrival_time"], "08:32");
    assert_eq!(
        trains[0]["runs_on"],
        serde_json::json!([true, true, true, true, true, true, true])
    );
    assert_eq!(
        trains[1]["runs_on"],
        serde_json::json!([true, false, true, false, true, false, false])
    );
}

#[tokio::test]
async fn ntes_still_wins_when_both_sources_are_up() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(TBS_HTML);
    // IRCTC mock returns a 502-worthy shape if it were ever consulted.
    app.mocks["irctc"].route_json(AVAIL_PATH, serde_json::json!({ "trainBtwnStnsList": [] }));

    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["data_source"], "NTES");
    assert_eq!(body["trains"][0]["number"], "12951");
}

#[tokio::test]
async fn trains_between_is_502_when_every_source_is_down() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].route_error("/mntes/q", StatusCode::INTERNAL_SERVER_ERROR);
    // IRCTC mock has no route for altAvlEnq -> mock 404 -> source unavailable.

    let (status, body) = app
        .get("/rail-api/ntes/trains-between?src=MMCT&dst=NDLS")
        .await;
    assert_eq!(status, 502);
    let err = body["error"].as_str().unwrap_or_default();
    assert!(err.contains("NTES"), "error should mention NTES: {err}");
    assert!(err.contains("IRCTC"), "error should mention IRCTC: {err}");
}
