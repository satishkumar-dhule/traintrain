mod common;

use common::TestApp;

/// Two trains as the NTES station-timetable (TrainsAtStation) web form renders
/// them: a summary caption with the total + station, then one row per train
/// with route, type/classes and arrival/departure times (the run-days cell is
/// rendered as `- Fri -` / `- Daily -`).
const TAS_HTML: &str = r##"<table class="table table-condensed "  id="myTable">
<thead>
<tr><th colspan="9" align="left"><font size="2" color="#006AD5" face="verdana"><b>326  Trains scheduled at NDLS - NEW DELHI</b></font></th></tr>
</thead>
<tbody>
<tr class=" w3-round">
<td colspan=3 style="border-radius: 20px 20px 20px 20px;margin-top: 20px;padding-bottom: 0px;border-bottom:5px solid #eee;border-left:5px solid #eee;border-right:5px solid #eee;">
<span ><b>22403</b>&nbsp;&nbsp;PDY NDLS SF EXP</span>
<br><span >Pondicherry (PDY) - New Delhi (NDLS)</span>
<br><span >Superfast | 1A,2A,3A,SL,GEN,PWD</span>
<div style="float: right;padding:5px;border:0px;margin-top:-30px;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" onclick="showTrainServiceSchedule('22403','15-Aug-2026',document.getElementsByName('frmTAS')[0]);" style="cursor:pointer;background: #eee;"/></div>
<div style="display: flex; justify-content: space-between; align-items: center; width: 100%; border-top: 1px solid #eee; text-align: center; padding: 5px 2px;">
<span style="text-align: left;width: 25%;">Arr.	: <b>00:20</b></span>
<div style="text-align: center; width: 50%;">- Fri -</div>
<span style="text-align: right; width: 25%;">Dep.: <b>DSTN</b></span>
</div>
</td>
</tr>
<tr class=" w3-round">
<td colspan=3 style="border-radius: 20px 20px 20px 20px;margin-top: 20px;padding-bottom: 0px;border-bottom:5px solid #eee;border-left:5px solid #eee;border-right:5px solid #eee;">
<span ><b>64422</b>&nbsp;&nbsp;NDLS-GZB EMU</span>
<br><span >New Delhi (NDLS) - Ghaziabad (GZB)</span>
<br><span >Emu | GEN</span>
<div style="float: right;padding:5px;border:0px;margin-top:-30px;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" onclick="showTrainServiceSchedule('64422','15-Aug-2026',document.getElementsByName('frmTAS')[0]);" style="cursor:pointer;background: #eee;"/></div>
<div style="display: flex; justify-content: space-between; align-items: center; width: 100%; border-top: 1px solid #eee; text-align: center; padding: 5px 2px;">
<span style="text-align: left;width: 25%;">Arr.	: <b>SRC</b></span>
<div style="text-align: center; width: 50%;">- Daily -</div>
<span style="text-align: right; width: 25%;">Dep.: <b>00:10</b></span>
</div>
</td>
</tr>
</tbody>
</table>"##;

#[tokio::test]
async fn missing_or_empty_params_are_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/ntes/station-timetable",
        "/rail-api/ntes/station-timetable?station=",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn invalid_station_is_bad_request() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/station-timetable?station=ABCDE")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Invalid station code: ABCDE");

    let (status, body) = app
        .get("/rail-api/ntes/station-timetable?station=ZZZZ")
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "Station ZZZZ not found.");
}

#[tokio::test]
async fn station_timetable_returns_normalized_trains() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(TAS_HTML);

    let (status, body) = app
        .get("/rail-api/ntes/station-timetable?station=NDLS")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["station"], "NDLS");
    assert_eq!(body["station_name"], "NEW DELHI");
    assert_eq!(body["total"], 326);

    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);

    assert_eq!(trains[0]["number"], "22403");
    assert_eq!(trains[0]["name"], "PDY NDLS SF EXP");
    assert_eq!(trains[0]["route"], "Pondicherry (PDY) - New Delhi (NDLS)");
    assert_eq!(trains[0]["train_type"], "Superfast");
    assert_eq!(trains[0]["classes"], "1A,2A,3A,SL,GEN,PWD");
    assert_eq!(trains[0]["arrival"], "00:20");
    assert_eq!(trains[0]["departure"], "DSTN");
    assert_eq!(trains[0]["days"], "Fri");

    assert_eq!(trains[1]["number"], "64422");
    assert_eq!(trains[1]["name"], "NDLS-GZB EMU");
    assert_eq!(trains[1]["train_type"], "Emu");
    assert_eq!(trains[1]["classes"], "GEN");
    assert_eq!(trains[1]["arrival"], "SRC");
    assert_eq!(trains[1]["departure"], "00:10");
    assert_eq!(trains[1]["days"], "Daily");

    assert_eq!(body["data_source"], "NTES");
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/station-timetable?station=NDLS")
        .await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}
