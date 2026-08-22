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
