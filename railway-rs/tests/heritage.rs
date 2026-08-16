mod common;

use common::TestApp;

/// Two trains as the NTES heritage page renders them (summary header, run-days
/// text, and per-train times/station/duration cells).
const HT_HTML: &str = r#"<table class="table table-condensed table-bordered" style="padding: 10dp;">
<thead>
<tr><td colspan="9" align="left" style="padding-left: 20px;"><font class="bluehead"><b>43 All Heritage Trains</b></font></td></tr>
</thead>
<tbody>
<tr class=" w3-round" style="margin-left: 5dp;margin-right: 5dp;">
<td style="width: 30px;" align="center">1.</td>
<td colspan=3 style="border-radius: 25px 25px 25px 25px;margin-top: 10px;padding-bottom: 0px;border-bottom:5px solid #eee;"><span style="padding-left: 10px;"><b>52457</b>&nbsp;&nbsp;KLK SML EXP</span><br><span style="padding-left: 10px;">Daily | Passenger</span>
<div style="float: right;padding:4px;border:0px;margin-top:-20px;margin-right:10px;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" style="background: #eee;cursor: pointer;" onclick="showTrainServiceSchedule('52457','15-Aug-2026',document.getElementsByName('frmTBSH')[0]);" /></div>
<div style="width: 100%;height:1px;background-color:#E9ECEE;"></div>
<table style="width: 100%;margin: 0px;padding-left:10px;padding-right:10px;">
<tr style="padding: 0px;">
<td width="35%" style="padding-left: 10px;"><b>03:30</b><br>KALKA<br><b>KLK</b></td>
<td align="center" width="30%">--05:20 Hrs.--</td>
<td align="right" width="35%" style="padding-right: 10px;"><b>08:50</b><br>SHIMLA<br><b>SML</b>
</td>
</tr>
</table>
</td>
</tr>
<tr class=" w3-round" style="margin-left: 5dp;margin-right: 5dp;">
<td style="width: 30px;" align="center">2.</td>
<td colspan=3 style="border-radius: 25px 25px 25px 25px;margin-top: 10px;padding-bottom: 0px;border-bottom:5px solid #eee;"><span style="padding-left: 10px;"><b>52451</b>&nbsp;&nbsp;SHIVALK DLX EXP</span><br><span style="padding-left: 10px;">Daily | Passenger</span>
<div style="float: right;padding:4px;border:0px;margin-top:-20px;margin-right:10px;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" style="background: #eee;cursor: pointer;" onclick="showTrainServiceSchedule('52451','15-Aug-2026',document.getElementsByName('frmTBSH')[0]);" /></div>
<div style="width: 100%;height:1px;background-color:#E9ECEE;"></div>
<table style="width: 100%;margin: 0px;padding-left:10px;padding-right:10px;">
<tr style="padding: 0px;">
<td width="35%" style="padding-left: 10px;"><b>05:45</b><br>KALKA<br><b>KLK</b></td>
<td align="center" width="30%">--04:55 Hrs.--</td>
<td align="right" width="35%" style="padding-right: 10px;"><b>10:40</b><br>SHIMLA<br><b>SML</b>
</td>
</tr>
</table>
</td>
</tr>
</tbody>
</table>"#;

#[tokio::test]
async fn invalid_selection_is_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/ntes/heritage?selection=6",
        "/rail-api/ntes/heritage?selection=abc",
        "/rail-api/ntes/heritage?selection=-1",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn heritage_returns_normalized_trains() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(HT_HTML);

    let (status, body) = app.get("/rail-api/ntes/heritage").await;
    assert_eq!(status, 200);
    assert_eq!(body["selection"], "All Heritage Trains");
    assert_eq!(body["total"], 43);

    let trains = body["trains"].as_array().unwrap();
    assert_eq!(trains.len(), 2);

    assert_eq!(trains[0]["number"], "52457");
    assert_eq!(trains[0]["name"], "KLK SML EXP");
    assert_eq!(trains[0]["runs"], "Daily");
    assert_eq!(trains[0]["train_type"], "Passenger");
    assert_eq!(trains[0]["source_time"], "03:30");
    assert_eq!(trains[0]["source_station"], "KALKA");
    assert_eq!(trains[0]["source_code"], "KLK");
    assert_eq!(trains[0]["duration"], "05:20");
    assert_eq!(trains[0]["dest_time"], "08:50");
    assert_eq!(trains[0]["dest_station"], "SHIMLA");
    assert_eq!(trains[0]["dest_code"], "SML");

    assert_eq!(trains[1]["duration"], "04:55");
    assert_eq!(trains[1]["dest_time"], "10:40");

    assert_eq!(body["data_source"], "NTES");

    let (status, _) = app.get("/rail-api/ntes/heritage?selection=1").await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/heritage").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}
