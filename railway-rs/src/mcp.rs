//! Model Context Protocol (MCP) server surface for railway-rs.
//!
//! Exposes the same local rail tool registry the assistant uses
//! ([`crate::slices::ai_chat::tools`]) to ANY external agent over the MCP
//! JSON-RPC protocol. The transport-agnostic core lives here:
//! [`handle`] maps one request to at most one response (`None` for
//! notifications), so tests drive it with plain values and the stdio binary
//! is a 30-line loop.

use serde_json::{json, Value};

use crate::core::error::AppError;
use crate::slices::ai_chat::tools;
use crate::state::AppState;

/// Protocol revision this server speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
pub const SERVER_NAME: &str = "railway-rs";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Handle one incoming JSON-RPC message. Returns `None` when the message is
/// a notification (no response expected) or lacks an id entirely.
pub async fn handle(state: &AppState, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(json!({}));

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION}
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": registry_descriptors()
        })),
        "tools/call" => tools_call(state, &params).await,
        // Notifications complete silently; unknown *requests* are errors.
        _ if id.is_null() => return None,
        other => Err(AppError::bad_request(format!("unknown method: {other}"))),
    };

    Some(match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(e) if e.message().starts_with("unknown method:") => json!({
            "jsonrpc": "2.0", "id": id,
            "error": {"code": -32601, "message": e.message()}
        }),
        Err(e) => {
            // Tool-level failures ride inside a successful tools/call result
            // so agents can read and recover from them.
            json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "content": [{"type": "text", "text": e.message()}],
                    "isError": true
                }
            })
        }
    })
}

fn registry_descriptors() -> Vec<Value> {
    tools::registry()
        .into_iter()
        .map(|d| {
            json!({
                "name": d.name,
                "description": d.description,
                "inputSchema": d.parameters
            })
        })
        .collect()
}

async fn tools_call(state: &AppState, params: &Value) -> Result<Value, AppError> {
    let name = params
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or_else(|| AppError::bad_request("tools/call requires 'name'"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let args_json = serde_json::to_string(&arguments)?;

    // Every MCP caller gets a full fresh budget: there is no conversation to
    // protect, one call == one result.
    let budget = tools::Budget::new(tools::DEFAULT_BUDGET_CHARS);
    let payload = tools::call_tool(state, &budget, name, &args_json).await?;

    Ok(json!({
        "content": [{"type": "text", "text": payload}],
        "isError": false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    async fn state() -> AppState {
        AppState::for_test(Config::default())
    }

    fn req(id: u64, method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    #[tokio::test]
    async fn initialize_reports_capabilities_and_version() {
        let s = state().await;
        let resp = handle(&s, &req(1, "initialize", json!({}))).await.unwrap();
        assert_eq!(resp["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(resp["result"]["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(resp["result"]["capabilities"]["tools"], json!({}));
    }

    #[tokio::test]
    async fn initialized_notification_yields_no_response() {
        let s = state().await;
        let msg = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        assert!(handle(&s, &msg).await.is_none());
    }

    #[tokio::test]
    async fn tools_list_exposes_the_whole_registry() {
        let s = state().await;
        let resp = handle(&s, &req(2, "tools/list", json!({}))).await.unwrap();
        let names: Vec<&str> = resp["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        // Registry grows over time; pin the invariants, not the full list.
        assert_eq!(names.first(), Some(&"trains_between"));
        assert_eq!(names.last(), Some(&"search_rail"));
        for expected in ["live_status", "average_delay"] {
            assert!(names.contains(&expected), "missing {expected}: {names:?}");
        }
        assert!(resp["result"]["tools"][0]["inputSchema"].is_object());
    }

    #[tokio::test]
    async fn tools_call_search_rail_hits_local_corpus() {
        let s = state().await;
        let resp = handle(
            &s,
            &req(
                3,
                "tools/call",
                json!({"name":"search_rail","arguments":{"query":"new delhi","limit":3}}),
            ),
        )
        .await
        .unwrap();
        assert_eq!(resp["result"]["isError"], false);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["results"][0]["code"], "NDLS");
    }

    #[tokio::test]
    async fn tool_failures_surface_as_is_error_results_not_rpc_errors() {
        let s = state().await;
        let resp = handle(
            &s,
            &req(
                4,
                "tools/call",
                json!({"name":"live_status","arguments":{"train":"12AB"}}),
            ),
        )
        .await
        .unwrap();
        assert_eq!(resp["result"]["isError"], true);
        assert!(resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not a valid 5-digit train number"));
    }

    #[tokio::test]
    async fn unknown_tool_is_caller_error_inside_result_envelope() {
        let s = state().await;
        let resp = handle(
            &s,
            &req(5, "tools/call", json!({"name":"teleport","arguments":{}})),
        )
        .await
        .unwrap();
        assert_eq!(resp["result"]["isError"], true);
        assert!(resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown tool"));
    }

    #[tokio::test]
    async fn unknown_method_is_json_rpc_error_32601() {
        let s = state().await;
        let resp = handle(&s, &req(6, "resources/list", json!({})))
            .await
            .unwrap();
        assert_eq!(resp["error"]["code"], -32601);
        assert!(resp.get("result").is_none());
    }

    #[tokio::test]
    async fn ping_answers_empty_result() {
        let s = state().await;
        let resp = handle(&s, &req(7, "ping", json!({}))).await.unwrap();
        assert_eq!(resp["result"], json!({}));
    }
}
