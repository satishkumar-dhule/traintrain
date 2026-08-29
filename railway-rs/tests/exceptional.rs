mod common;

use common::TestApp;

/// A realistic `subOpt=excpInfo` result page: train header, run days, month
/// calendar with two cancelled days and one rescheduled day (mirrors the real
/// NTES capture in `.agents/fixtures/mntes_excpinfo.html`).
const EXCP_HTML: &str = r##"<html><head><title>Exceptional Trains Details</title></head><body>
<h4>04138 - BJU GWL SPL</h4>
BARAUNI JN - GWALIOR JN.<br/>
Days of Run : <b>Wed,Sun</b>
<table><tr><th colspan="7"><font size="5pt">Aug-2026</font></th></tr>
<tr>
<td class="w3-tooltip" style="padding: 10px;">
<font color="#bfbfbf" size="4pt"><b>10</b></font>
</td>
<td class="w3-tooltip" style="padding: 10px;">
<font color="green" size="4pt"><b>12</b></font>
</td>
<td class="w3-tooltip" style="padding: 10px;">
<span style="position:absolute;left:0;bottom:40px" class="w3-text w3-tag w3-red w3-round-xlarge">[Train is Cancelled]</span>
<b> <font color="white" size="4pt" style="background: red;border-radius: 50%;padding: 5px;">16</font></b>
</td>
<td class="w3-tooltip" style="padding: 10px;">
<span style="position:absolute;left:0;bottom:40px" class="w3-text w3-tag w3-red w3-round-xlarge">[Train is Cancelled]</span>
<b> <font color="white" size="4pt" style="background: red;border-radius: 50%;padding: 5px;">19</font></b>
</td>
<td class="w3-tooltip" style="padding: 10px;">
<span style="position:absolute;left:0;bottom:40px" class="w3-text w3-tag w3-blue w3-round-xlarge">[Train is Rescheduled from Source]</span>
<b> <font color="white" size="4pt" style="background: blue;border-radius: 50%;padding: 5px;">23</font></b>
</td>
</tr>
</table></body></html>"##;

const EXCP_NODATA_HTML: &str = r#"<html><head><title>Exceptional Trains Details</title></head><body>
<div class="w3-panel w3-round w3-red"><h4>No Exceptional Details found for train 12951 !!!</h4></div>
</body></html>"#;

#[tokio::test]
async fn missing_train_is_rejected() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/exceptional").await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "train is required (4-5 digit train number)");
}

#[tokio::test]
async fn bad_train_is_rejected() {
    let app = TestApp::spawn().await;
    for train in ["abc", "12", "123456"] {
        let (status, body) = app
            .get(&format!("/rail-api/ntes/exceptional?train={train}"))
            .await;
        assert_eq!(status, 400, "train={train}");
        assert_eq!(body["error"], "train is required (4-5 digit train number)");
    }
}

#[tokio::test]
async fn bad_type_is_rejected() {
    let app = TestApp::spawn().await;
    let (status, body) = app
        .get("/rail-api/ntes/exceptional?train=04138&type=foo")
        .await;
    assert_eq!(status, 400);
    assert_eq!(
        body["error"],
        "type must be one of: cancelled, rescheduled, diverted"
    );
}

#[tokio::test]
async fn calendar_maps_to_exceptions() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(EXCP_HTML);
    let (status, body) = app.get("/rail-api/ntes/exceptional?train=04138").await;
    assert_eq!(status, 200);
    assert_eq!(body["train"]["number"], "04138");
    assert_eq!(body["train"]["name"], "BJU GWL SPL");
    assert_eq!(body["train"]["source"], "BARAUNI JN");
    assert_eq!(body["train"]["destination"], "GWALIOR JN");
    assert_eq!(
        body["train"]["days_of_run"],
        serde_json::json!(["Wed", "Sun"])
    );
    let exceptions = body["exceptions"].as_array().unwrap();
    assert_eq!(exceptions.len(), 3);
    assert_eq!(exceptions[0]["date"], "2026-08-16");
    assert_eq!(exceptions[0]["kind"], "cancelled");
    assert_eq!(exceptions[1]["kind"], "cancelled");
    assert_eq!(exceptions[2]["kind"], "rescheduled");
    assert_eq!(body["data_source"], "NTES");
    assert_eq!(body["cache_ttl"], 7200);
    assert!(body.get("message").is_none(), "no message when data exists");
}

#[tokio::test]
async fn type_filter_filters_exceptions() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(EXCP_HTML);
    let (status, body) = app
        .get("/rail-api/ntes/exceptional?train=04138&type=cancelled")
        .await;
    assert_eq!(status, 200);
    assert_eq!(body["type"], "cancelled");
    let exceptions = body["exceptions"].as_array().unwrap();
    assert_eq!(exceptions.len(), 2);
    assert!(exceptions.iter().all(|e| e["kind"] == "cancelled"));
}

#[tokio::test]
async fn nodata_page_returns_empty_exceptions() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(EXCP_NODATA_HTML);
    let (status, body) = app.get("/rail-api/ntes/exceptional?train=12951").await;
    assert_eq!(status, 200);
    assert_eq!(body["train"]["number"], "12951");
    // The no-data page does not echo the train identity, so the name is
    // resolved from the local master list (12951 -> NDLS TEJAS RAJ).
    assert_eq!(body["train"]["name"], "NDLS TEJAS RAJ");
    assert!(body["exceptions"].as_array().unwrap().is_empty());
    // The NTES page's own verdict is echoed verbatim.
    assert_eq!(
        body["message"],
        "No Exceptional Details found for train 12951 !!!"
    );
}

#[tokio::test]
async fn shell_page_without_calendar_is_honest_source_unavailable() {
    // The web client only trusts a page that carries the excpInfo result
    // (title marker + calendar); a nav shell is not guessed at - 502.
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web("<table><tr><th>No data</th></tr></table>");
    let (status, body) = app.get("/rail-api/ntes/exceptional?train=04138").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}

#[tokio::test]
async fn no_mock_route_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ntes/exceptional?train=04138").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unavailable"));
}

#[tokio::test]
async fn per_train_calendar_is_cached_for_two_hours() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(EXCP_HTML);

    let (status, body) = app.get("/rail-api/ntes/exceptional?train=04138").await;
    assert_eq!(status, 200);
    assert_eq!(body["cache_ttl"], 7200);
    assert_eq!(
        q_calls(&app),
        2,
        "first request hits upstream twice (N² fan-out)"
    );

    let (status, body) = app.get("/rail-api/ntes/exceptional?train=04138").await;
    assert_eq!(status, 200);
    assert_eq!(body["exceptions"].as_array().unwrap().len(), 3);
    assert_eq!(q_calls(&app), 2, "second request is served from cache");

    // A different train is a different cache key, so it hits upstream again.
    app.mocks["ntes"].ntes_web(EXCP_NODATA_HTML);
    let (status, _) = app.get("/rail-api/ntes/exceptional?train=12951").await;
    assert_eq!(status, 200);
    assert_eq!(q_calls(&app), 4, "per-train keys stay independent");
}

fn q_calls(app: &TestApp) -> usize {
    app.mocks["ntes"]
        .calls()
        .iter()
        .filter(|(path, _)| path == "/mntes/q")
        .count()
}
