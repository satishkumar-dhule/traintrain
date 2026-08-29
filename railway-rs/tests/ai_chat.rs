mod common;

use axum::http::StatusCode;
use railway_rs::config::Config;
use serde_json::{json, Value};

use common::{RouteSpec, TestApp};

/// Verbatim Zen-style streaming completion: a reasoning fragment, an answer
/// fragment, usage and the terminal `[DONE]` sentinel.
const SSE_FIXTURE: &str = concat!(
    "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking\"}}]}\n\n",
    "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n",
    "data: {\"usage\":{\"prompt_tokens\":92,\"completion_tokens\":10}}\n\n",
    "data: [DONE]\n\n"
);

/// Mid-stream hard error as the Zen gateway reports it inside an otherwise
/// healthy 200 event-stream.
const SSE_ERROR_FRAME: &str = concat!(
    "data: {\"type\":\"error\",\"error\":{\"type\":\"FreeUsageLimitError\",",
    "\"message\":\"Rate limit exceeded.\"}}\n\n"
);

const CHAT_PATH: &str = "/rail-api/ai/chat";
const USER_QUESTION: &str = "When does the 12951 Rajdhani run?";

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

fn chat_payload() -> Value {
    json!({ "messages": [{ "role": "user", "content": USER_QUESTION }] })
}

#[tokio::test]
async fn happy_path_relays_sse_events_with_prepended_persona() {
    let app = TestApp::spawn().await;
    serve_sse(&app, SSE_FIXTURE);

    let (status, content_type) = app.post_probe(CHAT_PATH, chat_payload()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type.starts_with("text/event-stream"),
        "content-type was {content_type}"
    );

    app.mock("zen").clear_calls();
    let (status, body) = app.post_raw(CHAT_PATH, chat_payload()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"type\":\"reasoning\""), "body: {body}");
    assert!(body.contains("\"text\":\"thinking\""), "body: {body}");
    assert!(body.contains("\"type\":\"delta\""), "body: {body}");
    assert!(body.contains("\"text\":\"Hello\""), "body: {body}");
    assert!(body.contains("\"type\":\"done\""), "body: {body}");
    assert!(body.contains("\"prompt_tokens\":92"), "body: {body}");
    assert!(body.contains("\"completion_tokens\":10"), "body: {body}");

    let calls = app.mock("zen").calls();
    assert_eq!(calls.len(), 1, "exactly one upstream POST");
    let (path, sent) = &calls[0];
    assert_eq!(path, "/chat/completions");
    let expected_model = Config::default().ai_model;
    assert!(
        sent.contains(&format!("\"model\":\"{expected_model}\"")),
        "upstream model must match config default, sent: {sent}"
    );
    assert!(sent.contains("\"stream\":true"), "sent: {sent}");
    assert!(sent.contains(USER_QUESTION), "sent: {sent}");
    let system_entries = sent.matches("\"role\":\"system\"").count();
    assert_eq!(
        system_entries, 1,
        "persona must be injected exactly once by the server, sent: {sent}"
    );
}

#[tokio::test]
async fn invalid_requests_are_bad_request_without_reaching_zen() {
    let app = TestApp::spawn().await;
    serve_sse(&app, SSE_FIXTURE);

    let many: Vec<Value> = (0..41)
        .map(|_| json!({"role": "user", "content": "hi"}))
        .collect();
    let oversized = json!({"messages": [{"role": "user", "content": "a".repeat(70_000)}]});
    for payload in [
        json!({"messages": []}),
        json!({"messages": [{"role": "robot", "content": "hello"}]}),
        json!({"messages": [{"role": "user", "content": "   "}]}),
        json!({"messages": [{"role": "user", "content": ""}]}),
        json!({ "messages": many }),
        oversized,
    ] {
        let (status, body) = app.post_raw(CHAT_PATH, payload).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("error"), "expected error body: {body}");
    }
    assert!(
        app.mock("zen").calls().is_empty(),
        "rejected requests must never hit the gateway"
    );
}

#[tokio::test]
async fn disabled_feature_is_502_without_calling_zen() {
    let app = TestApp::spawn_with_config(Config {
        ai_enabled: false,
        ..Default::default()
    })
    .await;

    let (status, body) = app.post_raw(CHAT_PATH, chat_payload()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body.contains("disabled"), "body: {body}");
    assert!(
        app.mock("zen").calls().is_empty(),
        "no request may reach zen while AI is disabled"
    );
}

#[tokio::test]
async fn upstream_failure_before_stream_is_502_with_upstream_message() {
    let app = TestApp::spawn().await;
    app.mock("zen").route(
        "/chat/completions",
        RouteSpec {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: json!({"error": {"message": "boom"}}),
            content_type: "application/json".into(),
            set_cookie: None,
        },
    );

    let (status, body) = app.post_raw(CHAT_PATH, chat_payload()).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body.contains("boom"), "body: {body}");
}

#[tokio::test]
async fn mid_stream_error_frame_becomes_error_event_on_live_stream() {
    let app = TestApp::spawn().await;
    serve_sse(&app, SSE_ERROR_FRAME);

    let (status, body) = app.post_raw(CHAT_PATH, chat_payload()).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "SSE headers are already committed; errors ride in-band"
    );
    assert!(body.contains("\"type\":\"error\""), "body: {body}");
    assert!(body.contains("Rate limit exceeded."), "body: {body}");
}

#[tokio::test]
async fn status_reports_configuration_truth() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get("/rail-api/ai/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], true);
    assert_eq!(body["model"], Config::default().ai_model);
    assert_eq!(body["keyed"], false);

    let off = TestApp::spawn_with_config(Config {
        ai_enabled: false,
        ..Default::default()
    })
    .await;
    let (status, body) = off.get("/rail-api/ai/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);
}

// ---------- agentic tool loop ----------

/// NTES trains-between table as the tool's inner service expects to parse it
/// (Secunderabad → Pune, one row).
const TB_TOOL_HTML: &str = r#"<table>
<tr><th colspan="9">2 Trains found from SC - SECUNDERABAD to PUNE - PUNE JN</th></tr>
<tr class="w3-round">
  <td colspan=3>
    <span><b>17013</b>&nbsp;&nbsp;HUBLI EXPRESS</span><br>
    <span>Daily</span>
    <span class="w3-round w3-blue" onclick="onTrainStatus('17013',document.getElementsByName('frmTBS')[0],'')">See Train Status >></span>
    <span style="text-align: left;width: 25%;"><b>21:35</b><br>Secunderabad<br>SC</span>
    <div style="text-align: center; width: 50%;">--11:45 Hrs.--</div>
    <span style="text-align: right; width: 25%;"><b>09:20</b><br>Pune<br><b>PUNE</b></span>
  </td>
</tr>
</table>"#;

fn sse_tool_call_round(name: &str, args: &str) -> String {
    let escaped = args.replace('"', "\\\"");
    format!(
        "data: {{\"choices\":[{{\"delta\":{{\"tool_calls\":[{{\"index\":0,\"id\":\"call_tb\",\"type\":\"function\",\"function\":{{\"name\":\"{name}\",\"arguments\":\"{escaped}\"}}}}]}}}}]}}\n\ndata: [DONE]\n\n"
    )
}

#[tokio::test]
async fn tool_loop_executes_trains_between_and_streams_final_answer() {
    let app = TestApp::spawn().await;
    app.mock("ntes").ntes_web(TB_TOOL_HTML);

    let round1 = sse_tool_call_round("trains_between", "{\"src\":\"SC\",\"dst\":\"PUNE\"}");
    let round2 = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"HUBLI EXPRESS (17013) runs Secunderabad to Pune.\"}}]}\n\n",
        "data: {\"usage\":{\"prompt_tokens\":210,\"completion_tokens\":18}}\n\n",
        "data: [DONE]\n\n"
    );
    app.mock("zen").route_raw_seq(
        "/chat/completions",
        vec![
            (StatusCode::OK, "text/event-stream", round1),
            (StatusCode::OK, "text/event-stream", round2.to_string()),
        ],
    );

    let (status, body) = app
        .post_raw(
            CHAT_PATH,
            json!({"messages":[{"role":"user","content":"trains from Hyderabad to Pune?"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("\"type\":\"tools\""),
        "tools chip frame missing: {body}"
    );
    assert!(body.contains("trains_between"));
    assert!(body.contains("17013"), "final answer not streamed: {body}");
    assert!(body.contains("\"type\":\"done\""));

    // Two zen rounds; the second carries the local tool result.
    let zen_calls = app.mock("zen").calls();
    assert_eq!(zen_calls.len(), 2, "expected exactly two model rounds");
    let second = &zen_calls[1].1;
    assert!(
        second.contains("\"role\":\"tool\""),
        "tool result not fed back: {second}"
    );
    assert!(
        second.contains("17013"),
        "grounded data missing from tool result"
    );

    // The inner service really hit the NTES mock.
    assert!(!app.mock("ntes").calls().is_empty(), "ntes never called");
}

#[tokio::test]
async fn tool_failure_is_fed_back_and_model_recovers() {
    let app = TestApp::spawn().await;
    let round1 = sse_tool_call_round("live_status", "{\"train\":\"12AB\"}");
    let round2 = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"That train number looked invalid.\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    app.mock("zen").route_raw_seq(
        "/chat/completions",
        vec![
            (StatusCode::OK, "text/event-stream", round1),
            (StatusCode::OK, "text/event-stream", round2.to_string()),
        ],
    );

    let (status, body) = app.post_raw(CHAT_PATH, chat_payload()).await;
    assert_eq!(status, StatusCode::OK);
    // The request still answers 200 SSE; the tool error rode in-band to the
    // model and the final prose arrived.
    assert!(body.contains("invalid"), "recovery answer missing: {body}");
    assert!(body.contains("\"type\":\"done\""));

    let zen_calls = app.mock("zen").calls();
    assert_eq!(zen_calls.len(), 2);
    assert!(
        zen_calls[1].1.contains("not a valid 5-digit train number"),
        "tool error payload not fed back: {}",
        zen_calls[1].1
    );
}

#[tokio::test]
async fn runaway_tool_loop_is_capped_after_four_rounds() {
    let app = TestApp::spawn().await;
    serve_sse(
        &app,
        &sse_tool_call_round("average_delay", "{\"train\":\"12951\"}"),
    );

    let (status, body) = app
        .post_raw(
            CHAT_PATH,
            json!({"messages":[{"role":"user","content":"delay?"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("too many tool steps"),
        "cap error missing: {body}"
    );
    assert_eq!(app.mock("zen").calls().len(), 4);
}

// ---------- seat_availability / station_board tools ----------

const PAYTM_SEARCH_PATH: &str = "/api/trains/v5/search";

/// Trimmed Paytm Travel search payload (mirrors tests/availability.rs shape):
/// three SC→PUNE trains. The first carries no class rows so the projection
/// must rank it last; the others exercise AVAILABLE, WL and RAC statuses.
fn seat_paytm_payload() -> Value {
    json!({
        "error": null,
        "status": {"result": "success", "message": {"title": "Successful"}},
        "code": 200,
        "body": {"trains": [
            {
                "trainNumber": "22143",
                "trainName": "PUNE EXPRESS",
                "source": "SC",
                "destination": "PUNE",
                "source_name": "Secunderabad",
                "destination_name": "Pune Jn",
                "departure": "2026-10-20T18:40:00+00:00",
                "arrival": "2026-10-21T06:30:00+00:00",
                "duration": "11:50",
                "classes": ["SL"],
                "train_type": "o"
            },
            {
                "departure": "2026-10-20T21:35:00+00:00",
                "arrival": "2026-10-21T09:20:00+00:00",
                "trainName": "HUBLI EXPRESS",
                "trainNumber": "17013",
                "source": "SC",
                "destination": "PUNE",
                "source_name": "Secunderabad",
                "destination_name": "Pune Jn",
                "duration": "11:45",
                "classes": ["SL", "3A"],
                "train_type": "o",
                "runs_on": {"text": "Runs on Mon, Tue, Wed, Thu, Fri, Sat, Sun"},
                "availability": [
                    {
                        "code": "SL",
                        "name": "Sleeper Class",
                        "non_formatted_status": "GNWL82/WL59",
                        "available_flag": "false",
                        "fare": 875,
                        "quota": "GN",
                        "pnr_prediction": {"value": 95}
                    },
                    {
                        "code": "3A",
                        "name": "AC 3 Tier",
                        "status": "AVAILABLE 0022",
                        "available_flag": true,
                        "fare": 2195
                    }
                ]
            },
            {
                "departure": "2026-10-20T10:15:00+00:00",
                "arrival": "2026-10-20T22:05:00+00:00",
                "trainName": "UDYAN EXPRESS",
                "trainNumber": "11301",
                "source": "SC",
                "destination": "PUNE",
                "source_name": "Secunderabad",
                "destination_name": "Pune Jn",
                "duration": "11:50",
                "classes": ["3A"],
                "train_type": "o",
                "runs_on": {"text": "Runs on Mon, Wed"},
                "availability": [
                    {"code": "3A", "name": "AC 3 Tier", "status": "RAC 12", "fare": 1890}
                ]
            }
        ]},
        "meta": {}
    })
}

#[tokio::test]
async fn seat_availability_tool_round_emits_projection() {
    let app = TestApp::spawn().await;
    app.mock("paytm")
        .route_json(PAYTM_SEARCH_PATH, seat_paytm_payload());

    let round1 = sse_tool_call_round("seat_availability", "{\"src\":\"SC\",\"dst\":\"PUNE\"}");
    let round2 = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"3A on HUBLI EXPRESS is AVAILABLE.\"}}]}\n\n",
        "data: {\"usage\":{\"prompt_tokens\":300,\"completion_tokens\":12}}\n\n",
        "data: [DONE]\n\n"
    );
    app.mock("zen").route_raw_seq(
        "/chat/completions",
        vec![
            (StatusCode::OK, "text/event-stream", round1),
            (StatusCode::OK, "text/event-stream", round2.to_string()),
        ],
    );

    let (status, body) = app
        .post_raw(
            CHAT_PATH,
            json!({"messages":[{"role":"user","content":"seats available SC to Pune?"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"type\":\"tools\""), "chip missing: {body}");
    assert!(body.contains("seat_availability"));
    assert!(body.contains("\"type\":\"done\""), "no final frame: {body}");

    // Two zen rounds; the second carries the projection as the tool result.
    let zen_calls = app.mock("zen").calls();
    assert_eq!(zen_calls.len(), 2);
    let second = &zen_calls[1].1;
    assert!(
        second.contains("\"seat_availability\""),
        "tool name not echoed in round 2: {second}"
    );
    assert!(
        second.contains("\"role\":\"tool\""),
        "tool result not fed back: {second}"
    );
    assert!(
        second.contains("classes"),
        "projection marker 'classes' missing from tool result: {second}"
    );
    assert!(
        second.contains("\\\"tone\\\""),
        "tone strings missing from tool result: {second}"
    );
    for tone in ["\\\"ok\\\"", "\\\"warn\\\"", "\\\"bad\\\""] {
        assert!(second.contains(tone), "tone {tone} missing: {second}");
    }

    // Availability-rich trains outrank the bare one regardless of upstream order.
    let rich = second.find("17013").expect("17013 in tool result");
    let bare = second.find("22143").expect("22143 in tool result");
    assert!(rich < bare, "trains with class status must rank first");

    // The inner service really hit the Paytm search endpoint.
    let paytm_calls = app.mock("paytm").calls();
    assert_eq!(paytm_calls.len(), 1);
    assert!(paytm_calls[0].0.starts_with(PAYTM_SEARCH_PATH));
}

/// Same table shape the live-station slice parses (two rows: one on time,
/// one running late with a platform number).
const BOARD_HTML: &str = r#"<table>
<tr><th colspan="10">28 Trains departing from/arriving at <b>SBC- BANGALORE CITY</b> in next 2 Hrs.</th></tr>
<tr><td nowrap style="width:20px;">1</td>
  <td align=left nowrap><b>12951</b>&nbsp;|<b> MUMBAI RAJDHANI</b><br>
    <span class="w3-round w3-blue w3-tiny" onclick="onTrainStatus('12951',document.getElementsByName('frmSTN')[0],'13-Aug-2026')">See Train Status >></span>
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
async fn station_board_tool_returns_rows() {
    let app = TestApp::spawn().await;
    app.mock("ntes").ntes_web(BOARD_HTML);

    let round1 = sse_tool_call_round("station_board", "{\"station\":\"SBC\"}");
    let round2 = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"MUMBAI RAJDHANI is on time at 09:15.\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    app.mock("zen").route_raw_seq(
        "/chat/completions",
        vec![
            (StatusCode::OK, "text/event-stream", round1),
            (StatusCode::OK, "text/event-stream", round2.to_string()),
        ],
    );

    let (status, body) = app
        .post_raw(
            CHAT_PATH,
            json!({"messages":[{"role":"user","content":"what is arriving at SBC?"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"type\":\"tools\""), "chip missing: {body}");
    assert!(body.contains("station_board"));
    assert!(body.contains("\"type\":\"done\""));

    let zen_calls = app.mock("zen").calls();
    assert_eq!(zen_calls.len(), 2);
    let second = &zen_calls[1].1;
    assert!(
        second.contains("\"role\":\"tool\""),
        "tool result not fed back: {second}"
    );
    for field in ["12951", "12301", "09:15", "10:30"] {
        assert!(
            second.contains(field),
            "{field} missing from rows: {second}"
        );
    }
    assert!(
        !app.mock("ntes").calls().is_empty(),
        "the NTES web flow was never exercised"
    );
}

#[tokio::test]
async fn seat_tool_defaults_blank_date_to_today_without_panicking() {
    let app = TestApp::spawn().await;
    app.mock("paytm")
        .route_json(PAYTM_SEARCH_PATH, seat_paytm_payload());

    let round1 = sse_tool_call_round(
        "seat_availability",
        "{\"src\":\"SC\",\"dst\":\"PUNE\",\"date\":\"\"}",
    );
    let round2 = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"Availability shown for today.\"}}]}\n\n",
        "data: [DONE]\n\n"
    );
    app.mock("zen").route_raw_seq(
        "/chat/completions",
        vec![
            (StatusCode::OK, "text/event-stream", round1),
            (StatusCode::OK, "text/event-stream", round2.to_string()),
        ],
    );

    let offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let today = chrono::Utc::now()
        .with_timezone(&offset)
        .date_naive()
        .to_string();

    let (status, body) = app.post_raw(CHAT_PATH, chat_payload()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"type\":\"done\""), "body: {body}");

    let zen_calls = app.mock("zen").calls();
    assert_eq!(zen_calls.len(), 2);
    let second = &zen_calls[1].1;
    let expected = format!("\\\"date\\\":\\\"{today}\\\"");
    assert!(
        second.contains(&expected),
        "blank date must resolve to today IST ({today}): {second}"
    );
    assert!(second.contains("\\\"trains\\\""), "rows missing: {second}");
}

#[tokio::test]
async fn tool_round_emits_card_and_action_frames() {
    let app = TestApp::spawn().await;
    app.mock("ntes").ntes_web(TB_TOOL_HTML);

    let round1 = sse_tool_call_round("trains_between", "{\"src\":\"SC\",\"dst\":\"PUNE\"}");
    let round2 = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"Found HUBLI EXPRESS.\"}}]}\n\n",
        "data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5}}\n\n",
        "data: [DONE]\n\n"
    );
    app.mock("zen").route_raw_seq(
        "/chat/completions",
        vec![
            (StatusCode::OK, "text/event-stream", round1),
            (StatusCode::OK, "text/event-stream", round2.to_string()),
        ],
    );

    let (status, body) = app
        .post_raw(
            CHAT_PATH,
            json!({"messages":[{"role":"user","content":"trains SC to PUNE?"}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Rich-card frame carries the projection for the UI.
    assert!(
        body.contains("\"type\":\"card\"") && body.contains("\"kind\":\"trains_between\""),
        "card frame missing: {body}"
    );
    assert!(body.contains("\"src_code\":\"SC\""), "codes not projected");

    // Actions frame lands before done, derived from the executed tool.
    let card_at = body.find("\"type\":\"card\"").expect("card index");
    let actions_at = body
        .find("\"type\":\"actions\"")
        .expect("actions frame missing");
    let done_at = body.find("\"type\":\"done\"").expect("done missing");
    assert!(
        card_at < actions_at && actions_at < done_at,
        "frame order wrong: {body}"
    );
    assert!(
        body.contains("{\"label\":\"Track 17013\",\"prompt\":\"live status of 17013\"}"),
        "track chip missing: {body}"
    );
}
