//! Local tool registry for the assistant's agentic loop: the model can call
//! real rail services (trains-between, live status, average delay) which run
//! in-process against the same upstreams the rest of the app uses. Tool
//! outputs are compact JSON, truncated hard so one tool call cannot blow the
//! context window.

use serde_json::{json, Value};

use crate::core::ai::AssembledToolCall;
use crate::core::error::AppError;
use crate::state::AppState;

/// Hard cap per tool result (chars) before it is truncated with a marker.
const MAX_TOOL_OUTPUT_CHARS: usize = 6_000;

pub fn schemas() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "trains_between",
                "description": "Search live trains running between two stations today. Accepts station names (e.g. 'Hyderabad') or codes (e.g. 'SC').",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "src": {"type": "string", "description": "Origin station name or code"},
                        "dst": {"type": "string", "description": "Destination station name or code"}
                    },
                    "required": ["src", "dst"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "live_status",
                "description": "Live running position of a train today: current station, delay, upcoming stops.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "train": {"type": "string", "description": "5-digit train number"}
                    },
                    "required": ["train"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "average_delay",
                "description": "Historical average delay statistics for a train.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "train": {"type": "string", "description": "5-digit train number"}
                    },
                    "required": ["train"]
                }
            }
        }),
    ]
}

/// Execute one model-requested tool against local rail services, returning
/// the compact JSON payload to feed back to the model.
pub async fn execute(state: &AppState, call: &AssembledToolCall) -> Result<String, AppError> {
    let args: Value = serde_json::from_str(call.arguments.trim()).unwrap_or(json!({}));
    let dto: Value = match call.name.as_str() {
        "trains_between" => {
            let src = require_str(&args, "src")?;
            let dst = require_str(&args, "dst")?;
            let src = resolve_station(state, &src).await?;
            let dst = resolve_station(state, &dst).await?;
            if src == dst {
                return Err(AppError::bad_request(
                    "origin and destination are the same station",
                ));
            }
            to_value(
                crate::slices::trains_between::service::Service::get_trains_between(
                    state, &src, &dst,
                )
                .await?,
            )
        }
        "live_status" => {
            let train = require_train(&args)?;
            // Empty date = today IST, resolved by the inner service.
            to_value(
                crate::slices::live_status::service::Service::get_live_status(state, &train, "")
                    .await?,
            )
        }
        "average_delay" => {
            let train = require_train(&args)?;
            to_value(
                crate::slices::average_delay::service::Service::get_average_delay(state, &train)
                    .await?,
            )
        }
        other => return Err(AppError::bad_request(format!("unknown tool: {other}"))),
    };
    Ok(clamp(serde_json::to_string(&dto).unwrap_or_default()))
}

fn to_value<T: serde::Serialize>(dto: T) -> Value {
    serde_json::to_value(dto).unwrap_or(Value::Null)
}

fn require_str(args: &Value, key: &str) -> Result<String, AppError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request(format!("tool argument '{key}' is required")))
}

fn require_train(args: &Value) -> Result<String, AppError> {
    let t = require_str(args, "train")?;
    if t.len() == 5 && t.bytes().all(|b| b.is_ascii_digit()) && t != "00000" {
        Ok(t)
    } else {
        Err(AppError::bad_request(format!(
            "'{t}' is not a valid 5-digit train number"
        )))
    }
}

/// Resolve a free-text station reference to a dataset code: exact/known codes
/// pass through; otherwise fall back to the local station search.
async fn resolve_station(state: &AppState, input: &str) -> Result<String, AppError> {
    use crate::slices::station_codes as codes;
    let upper = input.trim().to_ascii_uppercase();
    if codes::is_valid_code(&upper) && codes::code_known(state, &upper) {
        return Ok(upper);
    }
    let hits = state.datasets.search_stations(input.trim(), 1);
    match hits.first() {
        Some(rec) => Ok(rec.code.clone()),
        None => Err(AppError::bad_request(format!("unknown station: {input}"))),
    }
}

/// Serialize-cap a tool payload; the model reads truncated JSON-as-text fine.
fn clamp(s: String) -> String {
    if s.chars().count() <= MAX_TOOL_OUTPUT_CHARS {
        s
    } else {
        let head: String = s.chars().take(MAX_TOOL_OUTPUT_CHARS).collect();
        format!("{head}…[truncated]")
    }
}
