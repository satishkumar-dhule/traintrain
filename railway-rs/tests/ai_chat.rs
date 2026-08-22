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
    assert!(
        sent.contains("\"model\":\"x-preview-f-free\""),
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
    assert_eq!(body["model"], "x-preview-f-free");
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
