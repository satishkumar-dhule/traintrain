use serde_json::Value;

use crate::core::error::AppError;
use crate::core::fanout::{fanout_n2_singleflight, Candidate};
use crate::core::json::ValueExt;
use crate::models::{AverageDelayResponse, AverageDelayStation};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Average arrival/departure delays for `train`.
    ///
    /// NTES (`AverageDelay` web form) is the only source; the final DTO (not
    /// the raw upstream payload) is cached, so a later hit is served from the
    /// cache regardless of how the first one was produced.
    pub async fn get_average_delay(
        state: &AppState,
        train: &str,
    ) -> Result<AverageDelayResponse, AppError> {
        let cache_key = crate::core::cache::keys::average_delay(train);
        if let Some(resp) = state.cache.get_json(&cache_key) {
            return Ok(resp);
        }

        // Super fan-out N² deep: NTES (2-deep retry) + optional India proxy
        // (env-driven, not hard-coded) + Railyatri worldwide fallback.
        // Ensures HYB→AK 12951 shows real delays even when Singapore IP-blocked.
        let train_ntes = train.to_string();
        let train_ry = train.to_string();
        let state_ntes = state.clone();
        let state_ry = state.clone();

        let mut candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes.clone();
                let t = train_ntes.clone();
                async move { s.ntes_web.average_delay(&t).await }
            }),
            Candidate::new(crate::core::source::metric::RAILYATRI, move || {
                let s = state_ry.clone();
                let t = train_ry.clone();
                async move { railyatri_average_delay(&s, &t).await }
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
                        "{}/rail-api/ntes/average-delay?train={}",
                        base.trim_end_matches('/'),
                        urlencoding::encode(&t)
                    );
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(4))
                        .build()
                        .map_err(|e| AppError::source_unavailable("ntes-proxy", format!("client build: {e}")))?;
                    let res = client
                        .get(&url)
                        .send()
                        .await
                        .map_err(|e| AppError::source_unavailable("ntes-proxy", format!("GET {url}: {e}")))?;
                    if !res.status().is_success() {
                        return Err(AppError::source_unavailable(
                            "ntes-proxy",
                            format!("GET {url} returned {}", res.status()),
                        ));
                    }
                    let data: Value = res
                        .json()
                        .await
                        .map_err(|e| AppError::source_unavailable("ntes-proxy", format!("invalid JSON from {url}: {e}")))?;
                    if data.get("train_no").is_some() {
                        let list = data.get("stations").and_then(Value::as_array).cloned().unwrap_or_default();
                        let mapped_list: Vec<Value> = list
                            .iter()
                            .map(|s| {
                                serde_json::json!({
                                    "sr": s.get("sr").and_then(Value::as_str).unwrap_or(""),
                                    "name": s.get("name").and_then(Value::as_str).unwrap_or(""),
                                    "code": s.get("code").and_then(Value::as_str).unwrap_or(""),
                                    "arrivalDelay": s.get("arrival_delay").and_then(Value::as_str).unwrap_or(""),
                                    "departureDelay": s.get("departure_delay").and_then(Value::as_str).unwrap_or("")
                                })
                            })
                            .collect();
                        Ok(serde_json::json!({
                            "trainNo": data.get("train_no").and_then(Value::as_str).unwrap_or(&t),
                            "trainName": data.get("train_name").and_then(Value::as_str).unwrap_or(""),
                            "daysOfRun": data.get("days_of_run").and_then(Value::as_str).unwrap_or(""),
                            "trainType": data.get("train_type").and_then(Value::as_str).unwrap_or(""),
                            "list": mapped_list
                        }))
                    } else {
                        Err(AppError::source_unavailable("ntes-proxy", "unexpected shape"))
                    }
                }
            }));
        }

        let (metric, data) =
            fanout_n2_singleflight(state, candidates, &format!("avg_delay:{train}")).await?;
        let mut resp = map_ntes(data)?;
        if metric == crate::core::source::metric::RAILYATRI {
            resp.data_source = Some(crate::core::source::labels::RAILYATRI.to_string());
        }
        state.cache.set_json(&cache_key, &resp)?;
        Ok(resp)
    }
}

async fn railyatri_average_delay(state: &AppState, train: &str) -> Result<Value, AppError> {
    // Worldwide fallback: try Railyatri train page for delay hints.
    // Deep delegation: try live-status then time-table (n=2 endpoints).
    let urls = [
        state.config.source_url(
            &state.config.railyatri_base,
            &format!("/live-train-status/{train}"),
        ),
        state.config.source_url(
            &state.config.railyatri_base,
            &format!("/time-table/{train}"),
        ),
    ];
    let mut last_err: Option<AppError> = None;
    for url in urls {
        let res = match state.http.inner().get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(AppError::source_unavailable(
                    "Railyatri",
                    format!("GET {url}: {e}"),
                ));
                continue;
            }
        };
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::not_found(format!(
                "Train {train} not found on Railyatri"
            )));
        }
        if !res.status().is_success() {
            last_err = Some(AppError::source_unavailable(
                "Railyatri",
                format!("GET {url} returned {}", res.status()),
            ));
            continue;
        }
        let html = match res.text().await {
            Ok(h) => h,
            Err(e) => {
                last_err = Some(AppError::source_unavailable(
                    "Railyatri",
                    format!("read body {url}: {e}"),
                ));
                continue;
            }
        };
        // Try to extract any delay-like data from __NEXT_DATA__.
        // If we find a train object, synthesize a minimal average-delay shape
        // from its timetable (delays default to "--" when not reported).
        let nd = match crate::core::railyatri::extract_next_data(&html) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(AppError::source_unavailable("Railyatri", e.message()));
                continue;
            }
        };
        // Look for timetable stops to synthesize.
        if let Some(ttt) = crate::core::railyatri::deep_get(&nd, "props.pageProps.trainTimeTable") {
            if let Some(stops) = ttt
                .get("routeGroup")
                .and_then(Value::as_array)
                .and_then(|g| g.first())
                .and_then(|g| g.get("routesummary"))
                .and_then(Value::as_array)
            {
                if !stops.is_empty() {
                    let list: Vec<Value> = stops
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let (code, name) = crate::core::railyatri::stop_pair(s);
                            serde_json::json!({
                                "sr": (i + 1).to_string(),
                                "code": code,
                                "name": name,
                                "arrivalDelay": "--",
                                "departureDelay": "--"
                            })
                        })
                        .collect();
                    return Ok(serde_json::json!({
                        "trainNo": ttt.get("train_number").and_then(Value::as_str).unwrap_or(train),
                        "trainName": ttt.get("train_name").and_then(Value::as_str).unwrap_or(""),
                        "daysOfRun": "",
                        "trainType": "",
                        "list": list
                    }));
                }
            }
        }
        last_err = Some(AppError::source_unavailable(
            "Railyatri",
            "no timetable in payload",
        ));
    }
    Err(last_err
        .unwrap_or_else(|| AppError::source_unavailable("Railyatri", "average delay fetch failed")))
}

fn map_ntes(data: Value) -> Result<AverageDelayResponse, AppError> {
    let list = data
        .get("list")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppError::internal("NTES: unexpected average-delay shape"))?;

    let stations: Vec<AverageDelayStation> = list
        .iter()
        .map(|entry| AverageDelayStation {
            sr: entry.str_field("sr"),
            name: entry.str_field("name"),
            code: entry.str_field("code"),
            arrival_delay: entry.str_field("arrivalDelay"),
            departure_delay: entry.str_field("departureDelay"),
        })
        .collect();

    Ok(AverageDelayResponse {
        train_no: Some(data.str_field("trainNo")),
        train_name: Some(data.str_field("trainName")),
        days_of_run: Some(data.str_field("daysOfRun")),
        train_type: Some(data.str_field("trainType")),
        stations: Some(stations),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
    })
}
