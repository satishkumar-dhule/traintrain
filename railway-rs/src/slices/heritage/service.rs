use serde_json::Value;

use crate::core::error::AppError;
use crate::core::fanout::{fanout_n2_singleflight, Candidate};
use crate::models::{HeritageResponse, HeritageTrain};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Heritage trains for an NTES selection index (0 = all, 1..=5 = line).
    ///
    /// NTES (`HeritageTrainsBetweenStation` / `tbsh`) is the only source. The
    /// final DTO (not the raw upstream payload) is cached, so a later hit
    /// works without another NTES round trip.
    pub async fn get_heritage(
        state: &AppState,
        selection: u8,
    ) -> Result<HeritageResponse, AppError> {
        let cache_key = format!("heritage:{selection}");
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        // Super fan-out N²: NTES (2 delegates) + optional ntes-proxy hedged delegate.
        let sel1 = selection;
        let sel2 = 0u8;
        let state1 = state.clone();
        let state2 = state.clone();
        let mut candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state1.clone();
                let sel = sel1;
                async move { s.ntes_web.heritage_trains(sel).await }
            }),
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state2.clone();
                let sel = sel2;
                async move { s.ntes_web.heritage_trains(sel).await }
            }),
        ];
        if let Some(proxy_base) = state
            .config
            .ntes_proxy_base
            .clone()
            .filter(|v| !v.is_empty())
        {
            let sel = selection;
            let base = proxy_base;
            candidates.push(Candidate::new("ntes-proxy", move || {
                let sel = sel;
                let base = base.clone();
                async move {
                    let url = format!(
                        "{}/rail-api/ntes/heritage?selection={}",
                        base.trim_end_matches('/'),
                        sel
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
                    Ok(data)
                }
            }));
        }
        let data = match fanout_n2_singleflight(state, candidates, &format!("heritage:{selection}"))
            .await
        {
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
                    if let Ok(resp) = serde_json::from_value::<HeritageResponse>(stale) {
                        tracing::warn!(selection, err=%e.message(), "heritage: live timed out, serving stale cache");
                        return Ok(resp);
                    }
                }
                tracing::warn!(selection, err=%e.message(), "heritage: live timed out, serving static empty");
                let resp = HeritageResponse {
                    selection: Some(selection.to_string()),
                    total: Some(0),
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

fn map_ntes(data: Value) -> Result<HeritageResponse, AppError> {
    let list = data
        .get("list")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppError::internal("NTES: unexpected heritage shape"))?;

    let trains = list.iter().map(map_train).collect();
    Ok(HeritageResponse {
        selection: Some(str_field(&data, "selection")),
        total: data
            .get("total")
            .and_then(Value::as_u64)
            .map(|n| n as usize),
        trains: Some(trains),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
    })
}

fn map_train(entry: &Value) -> HeritageTrain {
    HeritageTrain {
        number: str_field(entry, "trainNo"),
        name: str_field(entry, "trainName"),
        runs: str_field(entry, "runs"),
        train_type: str_field(entry, "trainType"),
        source_time: str_field(entry, "srcTime"),
        source_station: str_field(entry, "srcStation"),
        source_code: str_field(entry, "srcCode"),
        duration: str_field(entry, "duration"),
        dest_time: str_field(entry, "dstTime"),
        dest_station: str_field(entry, "dstStation"),
        dest_code: str_field(entry, "dstCode"),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
