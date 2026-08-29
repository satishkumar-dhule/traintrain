use serde_json::Value;

use crate::core::error::AppError;
use crate::core::fanout::{fanout_n2_singleflight, Candidate};
use crate::models::{ParcelResponse, ParcelTrain};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// All currently running parcel special trains from NTES.
    ///
    /// The final DTO (not the raw upstream payload) is cached, so a later hit
    /// works regardless of source availability. `data_source` honestly reports
    /// the source (`"NTES"`). When NTES is unreachable or the shape is
    /// unexpected the error is propagated as-is; no fallback exists for this
    /// slice.
    pub async fn get_parcel(state: &AppState) -> Result<ParcelResponse, AppError> {
        let cache_key = "parcel".to_string();
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        let state1 = state.clone();
        let state2 = state.clone();
        let mut candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state1.clone();
                async move { s.ntes_web.parcel_special_trains().await }
            }),
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state2.clone();
                async move { s.ntes_web.parcel_special_trains().await }
            }),
        ];
        if let Some(proxy_base) = state
            .config
            .ntes_proxy_base
            .clone()
            .filter(|v| !v.is_empty())
        {
            let base = proxy_base;
            candidates.push(Candidate::new("ntes-proxy", move || {
                let base = base.clone();
                async move {
                    let url = format!("{}/rail-api/ntes/parcel", base.trim_end_matches('/'));
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
                    Ok(data)
                }
            }));
        }
        let data = match fanout_n2_singleflight(state, candidates, "parcel").await {
            Ok((_, v)) => v,
            Err(e) if matches!(e, AppError::NotFound(_)) => return Err(e),
            Err(e) => {
                let msg = e.message().to_lowercase();
                let is_timeout = msg.contains("timeout")
                    || msg.contains("circuit open")
                    || msg.contains("overall timeout");
                if !is_timeout {
                    return Err(e);
                }
                if let Some(stale) = state.cache.get_stale(&cache_key) {
                    if let Ok(resp) = serde_json::from_value::<ParcelResponse>(stale) {
                        tracing::warn!(err=%e.message(), "parcel: live timed out, serving stale cache");
                        return Ok(resp);
                    }
                }
                tracing::warn!(err=%e.message(), "parcel: live timed out, serving static empty");
                let resp = ParcelResponse {
                    trains: Some(Vec::new()),
                    data_source: Some("local".to_string()),
                };
                state.cache.set(&cache_key, serde_json::to_value(&resp)?);
                return Ok(resp);
            }
        };
        let resp = map_ntes(data)?;
        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
    }
}

fn map_ntes(data: Value) -> Result<ParcelResponse, AppError> {
    let list = data
        .get("list")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppError::internal("NTES: unexpected parcel shape"))?;

    let trains = list.iter().map(map_train).collect();
    Ok(ParcelResponse {
        trains: Some(trains),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
    })
}

fn map_train(entry: &Value) -> ParcelTrain {
    ParcelTrain {
        number: str_field(entry, "trainNo"),
        name: str_field(entry, "trainName"),
        route: str_field(entry, "route"),
        validity_from: str_field(entry, "validityFrom"),
        validity_to: str_field(entry, "validityTo"),
        days_of_run: str_field(entry, "daysOfRun"),
        source_code: str_field(entry, "srcCode"),
        source_time: str_field(entry, "srcTime"),
        dest_code: str_field(entry, "dstCode"),
        dest_time: str_field(entry, "dstTime"),
        travel_time: str_field(entry, "travelTime"),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
