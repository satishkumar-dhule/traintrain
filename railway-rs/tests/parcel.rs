mod common;

use common::TestApp;

/// Two parcel special trains as the NTES `TrainRunning`/`splTrnDtl` web form
/// renders them (train button + name, route, validity, days of run, and the
/// source/dest code-time legs).
const PS_HTML: &str = r#"<table class="table table-bordered table-condensed table-striped" style="text-align: center;">
<tr><th style="text-align: center;" colspan="6">All Parcel Special Trains</th></tr>
<tr class="active">
<td align="center" valign="middle" style="width: 60px;" nowrap><b>1</b></td>
<td style="text-align: left;indent:8px; margin-top:5px;"><button type="button" class="custom-btn" style="height: 30px;padding-left: 10px;padding-right: 10px;padding-bottom: 5px;padding-top: 5px;" onClick="javascript:onTrainInputByFindP('00111','15-Aug-2026')"><b>00111</b></button> &nbsp;<b> BIRD-SGTY RAPID CARGO </b>&nbsp;&nbsp; <span class="w3-round w3-blue w3-tiny w3-round" style="padding:2px 5px;font-size:8pt;cursor: pointer;" onclick="javascript:onTrainInputByFindP('00111','15-Aug-2026')">See Train Status >></span>
<div style="float: right;padding:4px;border:1px solid #E9ECEE;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" onclick="showTrainServiceScheduleSpot('00111','15-Aug-2026',845513,document.frmTBS);" style="cursor:pointer;background: #eee;" />
</div>
<br/>BHIVANDI ROAD - SANKRAIL GOODS TERMINAL
<br/> Validity : <b>25-Jul-2026</b> To <b>31-Dec-2099</b>
<div style="width: 100%;height:1px;background-color:#E9ECEE;"></div>
Days of Run : <b>Sat</b>
<br/>
<b>BIRD - 22:30</b>
|<b>SGTY - 15:15</b>
|Travel Time:&nbsp;<b>40:45 Hrs.</b>
<br/></td>
</tr>
<tr class="active">
<td align="center" valign="middle" style="width: 60px;" nowrap><b>2</b></td>
<td style="text-align: left;indent:8px; margin-top:5px;"><button type="button" class="custom-btn" style="height: 30px;padding-left: 10px;padding-right: 10px;padding-bottom: 5px;padding-top: 5px;" onClick="javascript:onTrainInputByFindP('00112','15-Aug-2026')"><b>00112</b></button> &nbsp;<b> SGTY-AJNI RAPID PARCEL </b>&nbsp;&nbsp; <span class="w3-round w3-blue w3-tiny w3-round" style="padding:2px 5px;font-size:8pt;cursor: pointer;" onclick="javascript:onTrainInputByFindP('00112','15-Aug-2026')">See Train Status >></span>
<div style="float: right;padding:4px;border:1px solid #E9ECEE;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" onclick="showTrainServiceScheduleSpot('00112','15-Aug-2026',845532,document.frmTBS);" style="cursor:pointer;background: #eee;" />
</div>
<br/>SANKRAIL GOODS TERMINAL - AJNI
<br/> Validity : <b>28-Jul-2026</b> To <b>31-Dec-2099</b>
<div style="width: 100%;height:1px;background-color:#E9ECEE;"></div>
Days of Run : <b>Tue</b>
<br/>
<b>SGTY - 19:30</b>
|<b>AJNI - 22:20</b>
|Travel Time:&nbsp;<b>26:50 Hrs.</b>
<br/></td>
</tr>
</table>"#;

#[tokio::test]
async fn parcel_returns_normalized_trains() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(PS_HTML);

    let (status, body) = app.get("/rail-api/ntes/parcel").await;
    assert_eq!(status, 200);

    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);

    assert_eq!(trains[0]["number"], "00111");
    assert_eq!(trains[0]["name"], "BIRD-SGTY RAPID CARGO");
    assert_eq!(
        trains[0]["route"],
        "BHIVANDI ROAD - SANKRAIL GOODS TERMINAL"
    );
    assert_eq!(trains[0]["validity_from"], "25-Jul-2026");
    assert_eq!(trains[0]["validity_to"], "31-Dec-2099");
    assert_eq!(trains[0]["days_of_run"], "Sat");
    assert_eq!(trains[0]["source_code"], "BIRD");
    assert_eq!(trains[0]["source_time"], "22:30");
    assert_eq!(trains[0]["dest_code"], "SGTY");
    assert_eq!(trains[0]["dest_time"], "15:15");
    assert_eq!(trains[0]["travel_time"], "40:45");

    assert_eq!(trains[1]["number"], "00112");
    assert_eq!(trains[1]["name"], "SGTY-AJNI RAPID PARCEL");
    assert_eq!(trains[1]["days_of_run"], "Tue");
    assert_eq!(trains[1]["travel_time"], "26:50");

    assert_eq!(body["data_source"], "NTES");
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/parcel").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}

#[tokio::test]
async fn unknown_query_params_are_ignored() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(PS_HTML);

    let (status, body) = app.get("/rail-api/ntes/parcel?foo=bar").await;
    assert_eq!(status, 200);
    assert_eq!(body["trains"].as_array().unwrap().len(), 2);
}
