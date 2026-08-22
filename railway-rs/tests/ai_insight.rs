mod common;

use axum::http::StatusCode;
use railway_rs::config::Config;
use serde_json::{json, Value};

use common::{RouteSpec, TestApp};

/// Verbatim Zen-style streaming completion: answer fragment plus terminal
/// `[DONE]` sentinel (no usage frame, so tokens report as 0).
const SSE_FIXTURE: &str = concat!(
    "data: {\"choices\":[{\"delta\":{\"content\":\"TRAIN IS 42 MIN LATE.\"}}]}\n\n",
    "data: [DONE]\n\n"
);

const INSIGHT_PATH: &str = "/rail-api/ai/insight";

/// Real NTES "Spot Your Train" popup captured for 12055 (active run 14-Aug,
/// next stop MEERUT CITY/MTC).
fn ntes_spot_train_fixture() -> String {
    std::fs::read_to_string("testdata/ntes_spot_train_12055.html").unwrap()
}

/// Average-delay page as NTES renders it: a header block (train no + name,
/// days of run, type) followed by the delay table (verbatim fixture shape
/// from tests/average_delay.rs).
fn ad_html() -> &'static str {
    r#"<table class="table table-bordered table-condensed table-striped" >
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
</table>"#
}

/// Two trains as the NTES trains-between web form renders them (verbatim
/// fixture shape from tests/trains_between.rs).
fn tbs_html() -> &'static str {
    r#"<table>
<tr><th colspan="9">30 Trains found from NDLS - NEW DELHI to MMCT - MUMBAI CENTRAL</th></tr>
<tr class="w3-round">
  <td colspan=3>
    <span><b>12951</b>&nbsp;&nbsp;MUMBAI RAJDHANI</span><br>
    <span>Daily | Superfast</span>
    <span class="w3-round w3-blue" onclick="onTrainStatus('12951',document.getElementsByName('frmTBS')[0],'')">See Train Status >></span>
    <span style="text-align: left;width: 25%;"><b>17:40</b><br>Mumbai Central<br>MMCT</span>
    <div style="text-align: center; width: 50%;">--14:52 Hrs.--</div>
    <span style="text-align: right; width: 25%;"><b>08:32</b><br>New Delhi<br><b>NDLS</b></span>
  </td>
</tr>
<tr class="w3-round">
  <td colspan=3>
    <span><b>12954</b>&nbsp;&nbsp;AK GOLD EXP</span><br>
    <span>Mon Wed Fri | Superfast</span>
    <span class="w3-round w3-blue" onclick="onTrainStatus('12954',document.getElementsByName('frmTBS')[0],'')">See Train Status >></span>
    <span style="text-align: left;width: 25%;"><b>20:05</b><br>Mumbai Central<br>MMCT</span>
    <span style="text-align: right; width: 25%;"><b>10:10</b><br>New Delhi<br><b>NDLS</b></span>
  </td>
</tr>
</table>"#
}

/// Wire the NTES spot-train flow (session bootstrap -> CSRF -> /mntes/tr).
fn mock_12055_spot_train(app: &TestApp) {
    let m = &app.mocks["ntes"];
    m.route_html("/mntes/", "<html><head><title>NTES</title></head></html>");
    m.route_html(
        "/mntes/GetCSRFToken",
        "<input type='hidden' name='csrfToken' value='tok123'>",
    );
    m.route_html("/mntes/tr", ntes_spot_train_fixture());
}

fn serve_sse(app: &TestApp, body: &str) {
    app.mock("zen").route(
        "/chat/completions",
        RouteSpec {
            status: StatusCode::OK,
            body: Value::String(body.to_string()),
            content_type: "text/event-stream".into(),
            set_cookie: None,
        },
    );
}

#[tokio::test]
async fn live_status_insight_is_grounded_and_summarized() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);
    serve_sse(&app, SSE_FIXTURE);

    let (status, body) = app
        .post_json(
            INSIGHT_PATH,
            json!({"kind": "live_status", "params": {"train": "12055"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "live_status");
    assert_eq!(body["summary"], "TRAIN IS 42 MIN LATE.");
    assert_eq!(body["data_source"], "zen+ntes");
    assert_eq!(body["cached"], false);
    assert_eq!(
        body["model"], "x-preview-f-free",
        "model must be reported honestly"
    );
    assert!(body["prompt_tokens"].is_u64());
    assert!(body["completion_tokens"].is_u64());

    // The posted messages must carry a system contract AND real grounded data
    // scraped from the NTES HTML (train number plus an actual stop code).
    let calls = app.mock("zen").calls();
    assert_eq!(calls.len(), 1, "exactly one upstream LLM call");
    let (path, sent) = &calls[0];
    assert_eq!(path, "/chat/completions");
    assert!(
        sent.contains("\"role\":\"system\""),
        "grounding contract must be present, sent: {sent}"
    );
    assert!(sent.contains("\"role\":\"user\""), "sent: {sent}");
    // NOTE: the live-status DTO does not repeat the queried train number;
    // grounding is proven by real scraped stop data instead.
    assert!(
        sent.contains("MTC"),
        "real next-station code from the NTES page must ground the answer, sent: {sent}"
    );
}

#[tokio::test]
async fn identical_second_call_is_cached_without_recalling_zen() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);
    serve_sse(&app, SSE_FIXTURE);

    let payload = json!({"kind": "live_status", "params": {"train": "12055"}});
    let (status, first) = app.post_json(INSIGHT_PATH, payload.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["cached"], false);

    let (status, second) = app.post_json(INSIGHT_PATH, payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["cached"], true);
    assert_eq!(second["summary"], first["summary"]);
    assert_eq!(second["kind"], "live_status");
    assert_eq!(second["data_source"], "zen+ntes");

    assert_eq!(
        app.mock("zen").calls().len(),
        1,
        "cache hit must not re-invoke the LLM"
    );
}

#[tokio::test]
async fn invalid_requests_are_bad_request_without_reaching_any_upstream() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);
    serve_sse(&app, SSE_FIXTURE);

    for payload in [
        json!({"kind": "horoscope", "params": {"train": "12055"}}),
        json!({"params": {"train": "12055"}}),
        json!({"kind": "live_status", "params": {}}),
        json!({"kind": "live_status", "params": {"train": "12A5"}}),
        json!({"kind": "live_status", "params": {"train": "00000"}}),
        json!({"kind": "live_status", "params": {"train": "1234"}}),
        json!({"kind": "average_delay"}),
        json!({"kind": "average_delay", "params": {"train": "00000"}}),
        json!({"kind": "trains_between", "params": {"src": "NDLS"}}),
        json!({"kind": "trains_between", "params": {"dst": "MMCT"}}),
        json!({"kind": "trains_between", "params": {"src": "NDLS", "dst": "ndls"}}),
        json!({"kind": "trains_between", "params": {"src": "N!", "dst": "MMCT"}}),
    ] {
        let (status, body) = app.post_json(INSIGHT_PATH, payload.clone()).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "payload {payload} should be rejected"
        );
        assert!(
            body["error"].as_str().is_some_and(|e| !e.is_empty()),
            "payload {payload} must carry an error message"
        );
    }
    assert!(
        app.mock("zen").calls().is_empty(),
        "rejected requests must never hit the LLM"
    );
}

#[tokio::test]
async fn inner_source_failure_is_propagated_honestly_without_calling_zen() {
    let app = TestApp::spawn().await;
    // Zen would happily answer if it were called - it must never be.
    serve_sse(&app, SSE_FIXTURE);

    // No NTES route registered: grounding fails before any LLM call.
    let (status, body) = app
        .post_json(
            INSIGHT_PATH,
            json!({"kind": "trains_between", "params": {"src": "NDLS", "dst": "MMCT"}}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let error = body["error"].as_str().unwrap();
    assert!(
        error.contains("unavailable"),
        "error must name the failed inner source honestly: {error}"
    );
    assert!(
        app.mock("zen").calls().is_empty(),
        "a failed grounding must never reach the LLM"
    );
}

#[tokio::test]
async fn zen_failure_surfaces_the_upstream_message() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(ad_html());
    app.mock("zen").route(
        "/chat/completions",
        RouteSpec {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: json!({"error": {"message": "boom"}}),
            content_type: "application/json".into(),
            set_cookie: None,
        },
    );

    let (status, body) = app
        .post_json(
            INSIGHT_PATH,
            json!({"kind": "average_delay", "params": {"train": "12055"}}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        body["error"].as_str().unwrap().contains("boom"),
        "body: {body}",
    );
}

/// Verbatim free-tier failure: the first completion dies instantly with
/// finish_reason "network_error" (no content, no usage). Single-shot
/// insights are idempotent, so the call is retried and the retry answers.
#[tokio::test]
async fn transient_zen_network_error_stream_is_retried_and_recovered() {
    let app = TestApp::spawn().await;
    mock_12055_spot_train(&app);
    let dead = concat!(
        "data: {\"choices\":[{\"index\":0,\"finish_reason\":\"network_error\",\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    app.mock("zen").route_raw_seq(
        "/chat/completions",
        vec![
            (StatusCode::OK, "text/event-stream", dead.to_string()),
            (StatusCode::OK, "text/event-stream", SSE_FIXTURE.to_string()),
        ],
    );

    let (status, body) = app
        .post_json(
            INSIGHT_PATH,
            json!({"kind": "live_status", "params": {"train": "12055"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "retry must recover: {body}");
    assert_eq!(body["summary"], "TRAIN IS 42 MIN LATE.");
    assert_eq!(body["cached"], false);
    assert_eq!(
        app.mock("zen").calls().len(),
        2,
        "the dead first attempt must have been retried exactly once"
    );
}

#[tokio::test]
async fn average_delay_happy_path_posts_the_ntes_form_and_summarizes() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(ad_html());
    serve_sse(&app, SSE_FIXTURE);

    let (status, body) = app
        .post_json(
            INSIGHT_PATH,
            json!({"kind": "average_delay", "params": {"train": "12055"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["summary"], "TRAIN IS 42 MIN LATE.");
    assert_eq!(body["kind"], "average_delay");
    assert_eq!(body["cached"], false);

    let calls = app.mocks["ntes"].calls();
    let q = calls
        .iter()
        .find(|(p, _)| p.starts_with("/mntes/q"))
        .expect("average-delay form must be queried");
    assert!(
        q.1.contains("csrfToken=tok123"),
        "form must carry the session CSRF token: {}",
        q.1
    );
    assert!(
        q.1.contains("trainNo=12055"),
        "form must ask for the requested train: {}",
        q.1
    );
}

#[tokio::test]
async fn trains_between_happy_path_posts_the_ntes_form_and_summarizes() {
    let app = TestApp::spawn().await;
    app.mocks["ntes"].ntes_web(tbs_html());
    serve_sse(&app, SSE_FIXTURE);

    let (status, body) = app
        .post_json(
            INSIGHT_PATH,
            json!({"kind": "trains_between", "params": {"src": " ndls ", "dst": "mmct"}}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["summary"], "TRAIN IS 42 MIN LATE.");
    assert_eq!(body["kind"], "trains_between");

    let calls = app.mocks["ntes"].calls();
    let q = calls
        .iter()
        .find(|(p, _)| p.starts_with("/mntes/q"))
        .expect("trains-between form must be queried");
    assert!(
        q.1.contains("jFromStationInput=NDLS"),
        "normalized src must drive the upstream form: {}",
        q.1
    );

    let zen_calls = app.mock("zen").calls();
    assert_eq!(zen_calls.len(), 1);
    assert!(
        zen_calls[0].1.contains("12951"),
        "grounded trains-between data must reach the model: {}",
        zen_calls[0].1
    );
}

#[tokio::test]
async fn disabled_ai_is_502_with_zero_zen_calls() {
    let app = TestApp::spawn_with_config(Config {
        ai_enabled: false,
        ..Default::default()
    })
    .await;

    let (status, body) = app
        .post_json(
            INSIGHT_PATH,
            json!({"kind": "live_status", "params": {"train": "12055"}}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        body["error"].as_str().unwrap().contains("disabled"),
        "body: {body}",
    );
    assert!(
        app.mock("zen").calls().is_empty(),
        "no request may reach zen while AI is disabled"
    );
}
