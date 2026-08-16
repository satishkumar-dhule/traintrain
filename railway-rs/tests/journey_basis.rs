mod common;

use axum::http::StatusCode;

use common::TestApp;

/// FindStationList response for 12055: a `trainNo` input plus the `jStation`
/// select whose options carry the `arrDays#depDays` title and the
/// `CODE#dayChange#seq` value the service round-trips into ShowRunCStn.
/// The placeholder option (no title) is skipped by the parser.
const STATIONLIST_HTML: &str = r##"<html><body>
<form name="frmTRN" method="post">
  <input type="hidden" name="lan" id="lan" value="en"/>
  <input type="text" name="trainNo" id="trainNo" size="6" maxlength="15" value="12055" onkeyup="onTrainFindInput('K')">
  <select  name="jStation" class="form-control"  id="jStation" onchange="onJourneyStationInput();" style="width:100%">
    <option value="">---Select---</option>
    <option title="DAILY#DAILY" value="NDLS#false#1" >NEW DELHI - NDLS</option>
    <option title="MON,WED#TUE,THU" value="GZB#true#3" >GHAZIABAD - GZB</option>
    <option title="DAILY#DAILY" value="DDN#false#53" >DEHRADOON - DDN</option>
  </select>
</form>
</body></html>"##;

/// ShowRunCStn popup for the run nearest today. The pane id carries the
/// ShowRunCStn TRAILING "1" (`train17-aug-20261`) which the parser tolerates.
/// The run has departed NEW DELHI, so GZB is the next/expected stop and DDN is
/// still scheduled.
const SHOWRUNCSTN_HTML: &str = r##"<html><body>
<div class="w3-panel w3-round w3-blue" style="margin-left: 20px;margin-right:20px;"><h3>12055 DDN JANSHTBDI</h3></div>
<div class="tab-content clearfix" style="margin-left: 10px;margin-right:10px;border: 2px solid #eee;border-radius:5px;">
  <div class="tab-pane active" id="train17-aug-20261" style="margin-left: 10px;margin-right:10px;">
    <h4 style="text-align: left; margin-left: 10px;margin-right:10px;">Journey Date :<b>17-Aug (15:20)</b></h4>
    <div style="text-align: left;width: 100%;margin-left: 10px;margin-right:10px;">
      <h6 class ="text-primary"><b>Departed from NEW DELHI(NDLS) at 15:20 17-Aug</b></h6>
    </div>
    <div class=" w3-card-2 w3-sand" style="width:100%;">
      <div class="w3-container" style="float:left;width:25%;text-align:right;">
        <font size="1"><span>&nbsp;</span><br><span>&nbsp;</span></font>
      </div>
      <div class="w3-container" style="float:right;width:70%;padding-left:0px;padding-right:0px;line-height:40px;">
        <div><font size="2" color="green"><b>Departed from NEW DELHI(NDLS) at 15:20 17-Aug</b></font></div>
      </div>
    </div>
    <div class=" w3-card-2" style="width:100%;">
      <div class="w3-container" style="float:left;width:100px;text-align:right;">
        <b><font size="1"></font></b><br>
        <font size="1" color="green" ><b></b><br><span class="w3-round w3-green" style="padding: 1px 4px;">On Time</span></font><br>
        <font size="2"><b>&nbsp;SRC&nbsp;&nbsp;</b></font>
      </div>
      <div class="w3-container" style="float:left;width:100px;text-align:center;">
        <div class="w3-bar-block" style="width:100%; background-image:url('track_gray.png');"><i class="fa fa-circle" style="color:teal;"></i></div>
      </div>
      <div class="w3-container" style="float:right;flex:1;padding-left:0px;padding-right:0px;display:flex;">
        <div class="w3-container" style="float:left;flex:1;">
          <span><font size="1"><b>NEW DELHI</b><br>
          <div class="w3-container" style="flex:1;padding:0px;display:inline-block;width:100%;text-align: center;">
            <div style="float:left;padding: 0px;"><b>NDLS <span class="w3-round w3-orange" style="padding: 1px 4px;">PF 9*</span></b></div>
            <div style="float:right;padding: 0px;"><b>0</b> KMs</div>
            <button class="btn" type="button">Coach Position</button>
          </div>
        </div>
      </div>
      <div class="w3-container" style="float:right;text-align:right;">
        <span><b><font size="1" >15:20 17-Aug</font></b></span><br>
        <span><font size="1" color="green" ><b>15:20 17-Aug</b><br><span class="w3-round w3-green" style="padding: 1px 4px;">On Time</span></font></span>
      </div>
    </div>
    <div class=" w3-card-2" style="width:100%;">
      <div class="w3-container" style="float:left;width:100px;text-align:right;">
        <b><font size="1">15:53 17-Aug</font></b><br>
        <font size="1" color="green" ><b></b><br><span class="w3-round w3-green" style="padding: 1px 4px;">On Time</span></font><br>
      </div>
      <div class="w3-container" style="float:left;width:100px;text-align:center;">
        <div class="w3-bar-block" style="width:100%; background-image:url('track_gray.png');"><i class="fa fa-circle" style="color:teal;"></i></div>
      </div>
      <div class="w3-container" style="float:right;flex:1;padding-left:0px;padding-right:0px;display:flex;">
        <div class="w3-container" style="float:left;flex:1;">
          <span><font size="1"><b>GHAZIABAD</b><br>
          <div class="w3-container" style="flex:1;padding:0px;display:inline-block;width:100%;text-align: center;">
            <div style="float:left;padding: 0px;"><b>GZB <span class="w3-round w3-orange" style="padding: 1px 4px;">PF 1*</span></b></div>
            <div style="float:right;padding: 0px;"><b>26</b> KMs</div>
            <button class="btn" type="button">Coach Position</button>
          </div>
        </div>
      </div>
      <div class="w3-container" style="float:right;text-align:right;">
        <span><b><font size="1" >15:55 17-Aug</font></b></span><br>
        <span><font size="1" color="green" ><b></b><br><span class="w3-round w3-green" style="padding: 1px 4px;">On Time</span></font></span>
      </div>
    </div>
    <div class=" w3-card-2" style="width:100%;">
      <div class="w3-container" style="float:left;width:100px;text-align:right;">
        <b><font size="1">21:05 17-Aug</font></b><br>
        <font size="1" color="green" ><b></b><br><span class="w3-round w3-green" style="padding: 1px 4px;">On Time</span></font><br>
      </div>
      <div class="w3-container" style="float:left;width:100px;text-align:center;">
        <div class="w3-bar-block" style="width:100%; background-image:url('track_gray.png');"><i class="fa fa-circle" style="color:teal;"></i></div>
      </div>
      <div class="w3-container" style="float:right;flex:1;padding-left:0px;padding-right:0px;display:flex;">
        <div class="w3-container" style="float:left;flex:1;">
          <span><font size="1"><b>DEHRADOON</b><br>
          <div class="w3-container" style="flex:1;padding:0px;display:inline-block;width:100%;text-align: center;">
            <div style="float:left;padding: 0px;"><b>DDN <span class="w3-round w3-orange" style="padding: 1px 4px;">PF 3*</span></b></div>
            <div style="float:right;padding: 0px;"><b>324</b> KMs</div>
            <button class="btn" type="button">Coach Position</button>
          </div>
        </div>
      </div>
      <div class="w3-container" style="float:right;text-align:right;">
        <span><b><font size="1" ></font></b></span><br>
        <span><font size="1" color="green" ><b></b><br><span class="w3-round w3-green" style="padding: 1px 4px;">On Time</span></font></span>
        <b>&nbsp;DSTN&nbsp;&nbsp;</b>
      </div>
    </div>
  </div>
</div>
</body></html>"##;

fn mock_journey_flow(app: &TestApp) {
    app.mocks["ntes"].ntes_web(STATIONLIST_HTML);
    app.mocks["ntes"].route_html("/mntes/tr", SHOWRUNCSTN_HTML);
}

#[tokio::test]
async fn missing_or_invalid_train_is_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/ntes/journey-stations",
        "/rail-api/ntes/journey-stations?train=",
        "/rail-api/ntes/journey-stations?train=abc",
        "/rail-api/ntes/journey-stations?train=00000",
        "/rail-api/ntes/journey-stations?train=1234",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn journey_basis_requires_station() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/journey-basis?train=12055").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("station"));
}

#[tokio::test]
async fn journey_basis_station_not_on_route_is_bad_request() {
    let app = TestApp::spawn().await;
    mock_journey_flow(&app);

    // GAYA is a valid 4-char known station, but the mocked FindStationList only
    // offers NDLS / GZB / DDN, so the service must reject it before hitting
    // ShowRunCStn.
    let (status, body) = app
        .get("/rail-api/ntes/journey-basis?train=12055&station=GAYA")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(error.contains("GAYA"), "error names the station: {error}");
    assert!(
        error.contains("not on the route"),
        "error mentions the route: {error}"
    );
    let calls = app.mocks["ntes"].calls();
    assert!(
        !calls.iter().any(|(p, _)| p.starts_with("/mntes/tr")),
        "ShowRunCStn must not be posted for an off-route station: {calls:?}"
    );
}

#[tokio::test]
async fn journey_stations_returns_normalized_list() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(STATIONLIST_HTML);

    let (status, body) = app.get("/rail-api/ntes/journey-stations?train=12055").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["train_no"], "12055");
    assert_eq!(body["data_source"], "ntes");

    let stations = body["stations"].as_array().unwrap();
    assert_eq!(stations.len(), 3, "placeholder option must be skipped");

    assert_eq!(stations[0]["code"], "NDLS");
    assert_eq!(stations[0]["name"], "NEW DELHI");
    assert_eq!(stations[0]["seq"], 1);
    assert_eq!(stations[0]["day_change"], false);
    assert_eq!(stations[0]["arrival_days"], "DAILY");
    assert_eq!(stations[0]["departure_days"], "DAILY");

    assert_eq!(stations[1]["code"], "GZB");
    assert_eq!(stations[1]["seq"], 3);
    assert_eq!(stations[1]["day_change"], true);
    assert_eq!(stations[1]["arrival_days"], "MON,WED");
    assert_eq!(stations[1]["departure_days"], "TUE,THU");

    assert_eq!(stations[2]["code"], "DDN");
    assert_eq!(stations[2]["name"], "DEHRADOON");
    assert_eq!(stations[2]["day_change"], false);
}

#[tokio::test]
async fn journey_basis_returns_status_and_journey_station() {
    let app = TestApp::spawn().await;
    mock_journey_flow(&app);
    app.mocks["ntes"].clear_calls();

    let (status, body) = app
        .get("/rail-api/ntes/journey-basis?train=12055&station=NDLS")
        .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(body["train_number"], "12055");
    assert_eq!(body["train_name"], "DDN JANSHTBDI");
    assert_eq!(body["train_start_date"], "17-Aug-2026");
    assert_eq!(body["data_source"], "NTES");

    let location = body["current_location_info"].as_str().unwrap();
    assert!(
        location.contains("next station"),
        "location names the next station: {location}"
    );

    let stations = body["stations"].as_array().unwrap();
    assert_eq!(stations.len(), 3);
    let statuses: Vec<&str> = stations
        .iter()
        .map(|s| s["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["departed", "expected", "scheduled"]);
    assert_eq!(stations[1]["code"], "GZB", "GZB is the next/expected stop");
    assert_eq!(stations[0]["scheduled_arrival"], "15:20");
    assert_eq!(stations[0]["actual_arrival"], "15:20");
    assert_eq!(
        stations[1]["actual_arrival"], "",
        "not reached -> no actual"
    );

    let journey = &body["journey_station"];
    assert_eq!(journey["code"], "NDLS");
    assert_eq!(journey["name"], "NEW DELHI");
    assert_eq!(journey["seq"], 1);
    assert_eq!(journey["day_change"], false);
    assert_eq!(journey["arrival_days"], "DAILY");
    assert_eq!(journey["departure_days"], "DAILY");

    // The ShowRunCStn form must carry the full CODE#dayChange#seq select value,
    // with the '#' percent-encoded by the form-urlencoded body.
    let calls = app.mocks["ntes"].calls();
    let tr_call = calls
        .iter()
        .find(|(p, _)| p.starts_with("/mntes/tr"))
        .expect("ShowRunCStn form must be posted");
    assert!(tr_call.1.contains("trainNo=12055"), "body: {}", tr_call.1);
    assert!(
        tr_call.1.contains("jStation=NDLS%23false%231"),
        "jStation must carry the encoded CODE#dayChange#seq: {}",
        tr_call.1
    );
}

#[tokio::test]
async fn upstream_failure_is_source_unavailable() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(STATIONLIST_HTML);
    app.mocks["ntes"].route_error("/mntes/tr", StatusCode::INTERNAL_SERVER_ERROR);

    let (status, body) = app
        .get("/rail-api/ntes/journey-basis?train=12055&station=NDLS")
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let error = body["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("unavailable"),
        "error names availability: {error}"
    );
}
