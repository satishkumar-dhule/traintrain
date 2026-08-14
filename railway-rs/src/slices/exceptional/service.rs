use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::ntes::NtesWebClient;
use crate::models::{ExceptionalResponse, ExceptionalTrain};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Exceptional running for one category: `cancelled`, `rescheduled` or
    /// `diverted` (anything else is `AppError::bad_request`).
    pub async fn get_exceptional(
        state: &AppState,
        kind: &str,
    ) -> Result<ExceptionalResponse, AppError> {
        let key = format!("exceptional:{kind}");
        if let Some(cached) = state.cache.get(&key) {
            if let Some(trains) = map_trains(&cached) {
                return Ok(build_response(kind, trains));
            }
        }

        let web = NtesWebClient::new(&state.http, &state.config.ntes_base);
        let start = Instant::now();
        let data = web.exceptional(kind).await;
        state.metrics.record_source_latency("ntes", start.elapsed());
        let data = data?;

        let trains = map_trains(&data)
            .ok_or_else(|| AppError::internal("NTES: unexpected exceptional response shape"))?;

        state.cache.set(&key, data);
        Ok(build_response(kind, trains))
    }
}

fn build_response(kind: &str, trains: Vec<ExceptionalTrain>) -> ExceptionalResponse {
    ExceptionalResponse {
        r#type: Some(kind.to_string()),
        trains: Some(trains),
        data_source: Some("NTES".to_string()),
    }
}

fn map_trains(data: &Value) -> Option<Vec<ExceptionalTrain>> {
    let rows = match data {
        Value::Array(rows) => Some(rows),
        Value::Object(map) => map.get("list").and_then(Value::as_array),
        _ => None,
    };
    rows.map(|rows| rows.iter().filter_map(map_train).collect())
}

fn map_train(row: &Value) -> Option<ExceptionalTrain> {
    Some(ExceptionalTrain {
        number: string_field(row, &["number", "trainNo"])?,
        name: string_field(row, &["name", "trainName"])?,
        date: string_field(row, &["date"])?,
        reason: string_field(row, &["reason"]).unwrap_or_default(),
    })
}

fn string_field(row: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = row.get(*key).and_then(Value::as_str) {
            if !s.trim().is_empty() {
                return Some(s.trim().to_string());
            }
        }
    }
    None
}
