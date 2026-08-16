mod common;

use common::TestApp;

/// Two trains as the NTES live-station web form renders them: an on-time
/// through train and a delayed one, plus a platform column.
const LS_HTML: &str = r#"<table>
<tr><th colspan="10">28 Trains departing from/arriving at <b>NDLS- NEW DELHI</b> in next 2 Hrs.</th></tr>
<tr><td nowrap style="width:20px;">1</td>
  <td align=left nowrap><b>12951</b>&nbsp;|<b> MUMBAI RAJDHANI</b><br>
    <span class="w3-round w3-blue w3-tiny" onclick="onTrainStatus('12951',document.getElementsByName('frmSTN')[0],'13-Aug-2026')">See Train Status >></span>
    &nbsp;
    <span class="w3-round w3-orange w3-tiny" onclick="showTrainServiceSchedule('12951','13-Aug-2026',document.getElementsByName('frmSTN')[0])">Train Schedule >></span>
  </td>
  <td nowrap width="130px">
    <font color="green">09:15</font><br>
    <span class="w3-round w3-green w3-tiny">On Time</span><br>
    <font size="1">&nbsp;09:15</font>
  </td>
  <td nowrap width="130px">
    <font color="green">09:15</font><br>
    <span class="w3-round w3-green w3-tiny">On Time</span><br>
    <font size="1">&nbsp;09:15</font>
  </td>
  <td width="80px"><b>1</b></td>
</tr>
<tr><td nowrap style="width:20px;">2</td>
  <td align=left nowrap><b>12301</b>&nbsp;|<b> RAJDHANI EXP</b><br>
    <span class="w3-round w3-blue w3-tiny" onclick="onTrainStatus('12301',document.getElementsByName('frmSTN')[0],'13-Aug-2026')">See Train Status >></span>
  </td>
  <td nowrap width="130px">
    <font color="red">10:30</font><br>
    <span class="w3-round w3-red w3-tiny">30 Mins.</span><br>
    <font size="1">&nbsp;10:00</font>
  </td>
  <td nowrap width="130px">
    <font color="red">10:30</font><br>
    <span class="w3-round w3-red w3-tiny">30 Mins.</span><br>
    <font size="1">&nbsp;10:00</font>
  </td>
  <td width="80px"><b>2</b></td>
</tr>
</table>"#;

#[tokio::test]
async fn bad_station_code_is_400() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=ABCDE&hours=2")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Invalid station code: ABCDE");
}

#[tokio::test]
async fn unknown_station_is_400() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=NDXX&hours=2")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Station NDXX not found.");
}

#[tokio::test]
async fn live_station_returns_mapped_trains() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(LS_HTML);

    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=NDLS&hours=2")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["station"], "NDLS");
    assert_eq!(body["hours"], 2);
    assert_eq!(body["data_source"], "NTES");
    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);
    assert_eq!(trains[0]["number"], "12951");
    assert_eq!(trains[0]["name"], "MUMBAI RAJDHANI");
    assert_eq!(trains[0]["sta"], "09:15");
    assert_eq!(trains[0]["eta"], "09:15");
    assert_eq!(trains[0]["platform"], "1");
    assert_eq!(trains[0]["delay_arr"], false);
    assert_eq!(trains[1]["delay_arr"], true);
}

#[tokio::test]
async fn unsupported_hour_window_is_bad_request() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(LS_HTML);

    for hours in ["1", "3", "5", "6", "7", "99"] {
        let (status, body) = app
            .get(&format!(
                "/rail-api/ntes/live-station?station=NDLS&hours={hours}"
            ))
            .await;
        assert_eq!(status, 400, "hours={hours} should be 400");
        assert_eq!(
            body["error"], "Live station window must be 2, 4, or 8 hours.",
            "hours={hours}"
        );
    }
}

#[tokio::test]
async fn no_mock_route_is_honest_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/live-station?station=NDLS&hours=2")
        .await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("Live source"));
}
