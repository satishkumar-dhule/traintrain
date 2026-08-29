use std::time::Duration;

use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::fanout::{fanout_n2_singleflight, Candidate};
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

        // Super fan-out N²: NTES (2 delegates) + optional ntes-proxy hedged delegate
        // (env-driven). First success wins; static local fallback only on timeout.
        let train_ntes1 = train.to_string();
        let train_ntes2 = train.to_string();
        let state_ntes1 = state.clone();
        let state_ntes2 = state.clone();
        let mut candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes1.clone();
                let t = train_ntes1.clone();
                async move { s.ntes_web.train_exceptions(&t).await }
            }),
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes2.clone();
                let t = train_ntes2.clone();
                async move { s.ntes_web.train_exceptions(&t).await }
            }),
        ];
        if let Some(proxy_base) = state
            .config
            .ntes_proxy_base
            .clone()
            .filter(|v| !v.is_empty())
        {
            let t = train.to_string();
            let base = proxy_base;
            candidates.push(Candidate::new("ntes-proxy", move || {
                let t = t.clone();
                let base = base.clone();
                async move {
                    let url = format!(
                        "{}/rail-api/ntes/exceptional?train={}",
                        base.trim_end_matches('/'),
                        urlencoding::encode(&t)
                    );
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(4))
                        .build()
                        .map_err(|e| {
                            AppError::source_unavailable("ntes-proxy", format!("client build: {e}"))
                        })?;
                    let res = client.get(&url).send().await.map_err(|e| {
                        AppError::source_unavailable("ntes-proxy", format!("GET {url}: {e}"))
                    })?;
                    if !res.status().is_success() {
                        return Err(AppError::source_unavailable(
                            "ntes-proxy",
                            format!("GET {url} returned {}", res.status()),
                        ));
                    }
                    let data: Value = res.json().await.map_err(|e| {
                        AppError::source_unavailable(
                            "ntes-proxy",
                            format!("invalid JSON from {url}: {e}"),
                        )
                    })?;
                    // Proxy already returns normalized exceptional shape; pass through.
                    Ok(data)
                }
            }));
        }
        let data = match fanout_n2_singleflight(state, candidates, &format!("exceptional:{train}"))
            .await
        {
            Ok((_, v)) => v,
            Err(e) if matches!(e, AppError::NotFound(_)) => return Err(e),
            Err(e) => {
                // Honest degradation: only a genuine timeout / circuit-open /
                // overall-deadline (the live source hanging or IP-blocked)
                // degrades to a static-local calendar. Any other live failure
                // (source returned an unexpected shell page, no route, etc.)
                // propagates as an honest 502 so the UI is never lied to.
                let msg = e.message().to_lowercase();
                let is_timeout = msg.contains("timeout")
                    || msg.contains("circuit open")
                    || msg.contains("overall timeout");
                if !is_timeout {
                    return Err(e);
                }
                if let Some(stale) = state.cache.get_stale(&key) {
                    if let Some(resp) = map_response(&stale, train, kind, state) {
                        tracing::warn!(train, err=%e.message(), "exceptional: live down, serving stale cache");
                        return Ok(resp);
                    }
                }
                tracing::warn!(train, err=%e.message(), "exceptional: live down, serving static empty");
                let synthetic = serde_json::json!({
                    "train": {"number": train, "name": "", "source": "", "destination": ""},
                    "exceptions": [],
                    "noData": true
                });
                let mut response =
                    map_response(&synthetic, train, kind, state).ok_or_else(|| {
                        AppError::internal("unexpected exception-calendar response shape")
                    })?;
                response.data_source = Some("local".to_string());
                return Ok(response);
            }
        };
        let mut response = map_response(&data, train, kind, state)
            .ok_or_else(|| AppError::internal("unexpected exception-calendar response shape"))?;
        // Cache live NTES data for 2 hours.
        state.cache.set_with_ttl(&key, data, EXCEPTIONAL_CACHE_TTL);
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
