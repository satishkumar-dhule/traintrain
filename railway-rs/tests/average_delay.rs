mod common;

use common::TestApp;

/// Average-delay page as NTES renders it: a header block (train no + name,
/// days of run, type) followed by the delay table.
const AD_HTML: &str = r#"<table class="table table-bordered table-condensed table-striped" >
	<tbody>
		<tr>
			<td class="w3-blue" align="left" style="border-bottom:1px solid #cccccc;border-right:none;"colspan="2"><span >12055 DDN JANSHTBDI</span></TD>
		</tr>
		<tr>
			<td align="left" style="border-bottom:none;border-right:none;"><span class="bluehead">Days of Run: &nbsp;</span>Daily</TD>
			<td align="right" style="border-bottom:none;"><span class="bluehead">Type: &nbsp;</span><span>JAN SHATABDI</span></TD>
		</tr>
	</tbody>
</table>
<table class="table table-bordered table-condensed table-striped">
	<tbody>
		<tr valign="top" height="20">
			<td><font style="font-size:small large; font-weight: bold">Sr.</font></td>
			<td><font style="font-size:small large; font-weight: bold">Station</font></td>
			<td><font style="font-size:small large; font-weight: bold">Code</font></td>
			<td><font style="font-size:small large; font-weight: bold">Avg. Arr. Delay</font></td>
			<td><font style="font-size:small large; font-weight: bold">Avg. Dep. Delay</font></td>
		</tr>
		 <tr>
			<td><font style="font-size:small large;">1</font></td>
			<td align="left"><font style="font-size:small large;">NEW DELHI</font></td>
			<td><font style="font-size:small large;">NDLS</font></td>
			<td>
			</td>
			<td>

				<font style="font-size:small large;  color: green">On Time</font>

			</td>
		</tr>

		 <tr>
			<td><font style="font-size:small large;">2</font></td>
			<td align="left"><font style="font-size:small large;">GHAZIABAD</font></td>
			<td><font style="font-size:small large;">GZB</font></td>
			<td>

			<font style="font-size:small large;  color: red">00:14</font>

			</td>
			<td>

			<font style="font-size:small large;  color: red">00:15</font>

			</td>
		</tr>
	</tbody>
</table>"#;

#[tokio::test]
async fn missing_or_invalid_train_is_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/ntes/average-delay",
        "/rail-api/ntes/average-delay?train=",
        "/rail-api/ntes/average-delay?train=12",
        "/rail-api/ntes/average-delay?train=123456",
        "/rail-api/ntes/average-delay?train=00000",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn average_delay_returns_normalized_delays() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(AD_HTML);

    let (status, body) = app.get("/rail-api/ntes/average-delay?train=12055").await;
    assert_eq!(status, 200);
    assert_eq!(body["train_no"], "12055");
    assert_eq!(body["train_name"], "DDN JANSHTBDI");
    assert_eq!(body["days_of_run"], "Daily");
    assert_eq!(body["train_type"], "JAN SHATABDI");

    let stations = body["stations"].as_array().unwrap();
    assert_eq!(stations.len(), 2);

    assert_eq!(stations[0]["sr"], "1");
    assert_eq!(stations[0]["name"], "NEW DELHI");
    assert_eq!(stations[0]["code"], "NDLS");
    assert_eq!(stations[0]["arrival_delay"], "");
    assert_eq!(stations[0]["departure_delay"], "On Time");

    assert_eq!(stations[1]["arrival_delay"], "00:14");
    assert_eq!(stations[1]["departure_delay"], "00:15");

    assert_eq!(body["data_source"], "NTES");
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/average-delay?train=12055").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}
