use std::time::Duration;

use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::fanout::{Candidate, fanout_n2};
use crate::models::{ExceptionEntry, ExceptionalResponse, ExceptionalTrainDetail};
use crate::state::AppState;

/// Per-train exception calendars are cached for 2 hours: the NTES page is a
/// static month calendar, so a 2-hour shelf life is plenty and keeps the
/// source's per-train form from being hammered.
pub const EXCEPTIONAL_CACHE_TTL: Duration = Duration::from_secs(2 * 60 * 60);

pub struct Service;

impl Service {
    /// Exceptional dates for one `train` (e.g. `04138`), cached per-train for
    /// 2 hours. `kind` optionally filters the list to `cancelled`,
    /// `rescheduled` or `diverted`; `None` returns every exception kind
    /// (including `new_source` / `new_destination`).
    pub async fn get_exceptional(
        state: &AppState,
        train: &str,
        kind: Option<&str>,
    ) -> Result<ExceptionalResponse, AppError> {
        let key = keys::exceptional(train);
        if let Some(cached) = state.cache.get(&key) {
            if let Some(response) = map_response(&cached, train, kind, state) {
                return Ok(response);
            }
        }

        // Super fan-out N²: NTES (2 delegates: with/without kind) + static local
        // (800ms delayed). Each delegate retried, first success wins.
        let train_ntes = train.to_string();
        let train_static = train.to_string();
        let state_ntes = state.clone();
        let state_static = state.clone();
        let candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes.clone();
                let t = train_ntes.clone();
                async move { s.ntes_web.train_exceptions(&t).await }
            }),
            Candidate::new("local", move || {
                let t = train_static.clone();
                let _s = state_static.clone();
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    Ok::<Value, AppError>(serde_json::json!({
                        "train": {"number": t, "name": "", "source": "", "destination": ""},
                        "exceptions": [],
                        "noData": true
                    }))
                }
            }),
        ];
        let (metric, data) = fanout_n2(state, candidates, &format!("exceptional:{train}")).await?;
        let mut response = map_response(&data, train, kind, state)
            .ok_or_else(|| AppError::internal("unexpected exception-calendar response shape"))?;
        if metric == "local" {
            response.data_source = Some("local".to_string());
        }
        // Only cache live NTES data for 2 hours; local static is not cached long.
        if metric == crate::core::source::metric::NTES {
            state.cache.set_with_ttl(&key, data, EXCEPTIONAL_CACHE_TTL);
        }
        Ok(response)
    }
}

fn build_response(
    kind: Option<&str>,
    train: Value,
    exceptions: Vec<ExceptionEntry>,
    message: Option<String>,
) -> ExceptionalResponse {
    ExceptionalResponse {
        r#type: kind.map(str::to_string),
        train: train_detail(&train),
        exceptions,
        message,
        data_source: Some(crate::core::source::labels::NTES.to_string()),
        cache_ttl: Some(EXCEPTIONAL_CACHE_TTL.as_secs()),
    }
}

fn map_response(
    data: &Value,
    train: &str,
    kind: Option<&str>,
    state: &AppState,
) -> Option<ExceptionalResponse> {
    let no_data = data.get("noData").and_then(Value::as_bool).unwrap_or(false);
    let mut train_obj = match data.get("train") {
        Some(Value::Object(map)) if !map.is_empty() => data["train"].clone(),
        _ => json_value_for_train(train),
    };
    // NTES does not echo the train identity on the no-data page; fall back to
    // the local master list (10,609 real trains) so the response always shows
    // a Train No./Name.
    let name = string_field(&train_obj, &["name"]);
    if name.as_deref().unwrap_or("").is_empty() {
        if let Some(fallback) = state.datasets.train_name(train) {
            if let Some(obj) = train_obj.as_object_mut() {
                obj.insert("name".to_string(), Value::String(fallback.to_string()));
            }
        }
    }
    let entries: Vec<ExceptionEntry> = data
        .get("exceptions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let entry = ExceptionEntry {
                date: string_field(e, &["date"]).unwrap_or_default(),
                kind: string_field(e, &["kind"])?,
                note: string_field(e, &["note"]).unwrap_or_default(),
            };
            match kind {
                Some(want) if entry.kind != want => None,
                _ => Some(entry),
            }
        })
        .collect();
    let message = if no_data {
        Some(format!(
            "No Exceptional Details found for train {train} !!!"
        ))
    } else {
        None
    };
    Some(build_response(kind, train_obj, entries, message))
}

/// Fallback identity used for the no-data page, where NTES does not echo any
/// train header: the requested train number is all we know.
fn json_value_for_train(train: &str) -> Value {
    serde_json::json!({
        "number": train,
        "name": "",
        "source": "",
        "destination": "",
        "daysOfRun": [],
    })
}

fn train_detail(train: &Value) -> Option<ExceptionalTrainDetail> {
    Some(ExceptionalTrainDetail {
        number: string_field(train, &["number"])?,
        name: string_field(train, &["name"]).unwrap_or_default(),
        source: string_field(train, &["source"]).unwrap_or_default(),
        destination: string_field(train, &["destination"]).unwrap_or_default(),
        days_of_run: train
            .get("daysOfRun")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
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
