use serde_json::Value;

use crate::core::error::AppError;
use crate::core::fanout::{Candidate, fanout_n2};
use crate::models::{StationTimetableResponse, StationTimetableTrain};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Trains scheduled at a station, resolved live from NTES.
    ///
    /// NTES (`TrainsAtStation`) is the sole source; when it is unreachable or
    /// malformed the `SourceUnavailable` / `Internal` error is propagated
    /// honestly - never fabricate trains. The final DTO (not the raw upstream
    /// payload) is cached, so a later hit within the TTL is served without
    /// touching NTES again.
    pub async fn get_station_timetable(
        state: &AppState,
        station: &str,
        station_name: &str,
        date: Option<String>,
    ) -> Result<StationTimetableResponse, AppError> {
        let cache_key = format!(
            "station_timetable:{station}:{}",
            date.as_deref().unwrap_or("any")
        );
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        // Super fan-out N²: NTES + Railyatri raced concurrently.
        let station_ntes = station.to_string();
        let station_ry = station.to_string();
        let name_ntes = station_name.to_string();
        let date_ntes = date.clone();
        let date_ry = date.clone();
        let state_ntes = state.clone();
        let state_ry = state.clone();

        let candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes.clone();
                let st = station_ntes.clone();
                let n = name_ntes.clone();
                let d = date_ntes.clone();
                async move { s.ntes_web.station_timetable(&st, &n, d.as_deref()).await }
            }),
            Candidate::new(crate::core::source::metric::RAILYATRI, move || {
                let s = state_ry.clone();
                let st = station_ry.clone();
                let d = date_ry.clone();
                async move { railyatri_station_timetable(&s, &st, d.as_deref()).await }
            }),
        ];

        let (metric, data) = fanout_n2(state, candidates, &format!("stn_tt:{station}")).await?;
        let mut resp = map_ntes(data, station, date.as_deref())?;
        if metric == crate::core::source::metric::RAILYATRI {
            resp.data_source = Some(crate::core::source::labels::RAILYATRI.to_string());
        }
        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
    }
}

async fn railyatri_station_timetable(
    state: &AppState,
    station: &str,
    date: Option<&str>,
) -> Result<Value, AppError> {
    let urls = [
        state.config.source_url(
            &state.config.railyatri_base,
            &format!("/trains-at-station/{station}"),
        ),
        state.config.source_url(
            &state.config.railyatri_base,
            &format!("/live-trains-at-station/{station}"),
        ),
    ];
    let mut last_err: Option<AppError> = None;
    for url in urls {
        let res = match state.http.inner().get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(AppError::source_unavailable("Railyatri", format!("GET {url}: {e}")));
                continue;
            }
        };
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::not_found(format!("Station {station} not found on Railyatri")));
        }
        if !res.status().is_success() {
            last_err = Some(AppError::source_unavailable("Railyatri", format!("GET {url} returned {}", res.status())));
            continue;
        }
        let html = match res.text().await {
            Ok(h) => h,
            Err(e) => {
                last_err = Some(AppError::source_unavailable("Railyatri", format!("read body {url}: {e}")));
                continue;
            }
        };
        let nd = match crate::core::railyatri::extract_next_data(&html) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(AppError::source_unavailable("Railyatri", e.message()));
                continue;
            }
        };
        // Try to find a trains list in the JSON (heuristic).
        if let Some(list) = find_trains(&nd) {
            if !list.is_empty() {
                let trains: Vec<Value> = list
                    .into_iter()
                    .take(100)
                    .map(|e| {
                        serde_json::json!({
                            "trainNo": e.get("train_number").or_else(|| e.get("trainNo")).and_then(Value::as_str).unwrap_or(""),
                            "trainName": e.get("train_name").or_else(|| e.get("trainName")).and_then(Value::as_str).unwrap_or(""),
                            "route": e.get("route").and_then(Value::as_str).unwrap_or(""),
                            "trainType": e.get("train_type").and_then(Value::as_str).unwrap_or(""),
                            "classes": e.get("classes").and_then(Value::as_str).unwrap_or(""),
                            "arrival": e.get("arrival").or_else(|| e.get("sta")).and_then(Value::as_str).unwrap_or(""),
                            "departure": e.get("departure").or_else(|| e.get("std")).and_then(Value::as_str).unwrap_or(""),
                            "days": e.get("days").and_then(Value::as_str).unwrap_or(""),
                        })
                    })
                    .collect();
                if !trains.is_empty() && trains.iter().any(|t| !t["trainNo"].as_str().unwrap_or("").is_empty()) {
                    let _ = date;
                    return Ok(serde_json::json!({ "list": trains, "station": station, "total": trains.len() }));
                }
            }
        }
        last_err = Some(AppError::source_unavailable("Railyatri", "no trains in board payload"));
    }
    Err(last_err.unwrap_or_else(|| AppError::source_unavailable("Railyatri", "station board fetch failed")))
}

fn find_trains(v: &Value) -> Option<Vec<Value>> {
    match v {
        Value::Array(arr) if !arr.is_empty() => {
            if let Some(first) = arr.first().and_then(Value::as_object) {
                if first.contains_key("train_number") || first.contains_key("trainNo") {
                    return Some(arr.clone());
                }
            }
            for el in arr {
                if let Some(found) = find_trains(el) {
                    return Some(found);
                }
            }
            None
        }
        Value::Object(map) => {
            for (_, val) in map {
                if let Some(found) = find_trains(val) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Map the NTES `TrainsAtStation` payload onto the `StationTimetableResponse`
/// DTO. The summary fields (`station` / `stationName` / `date` / `total`) are
/// read from the upstream payload, falling back to the validated request
/// values; an empty/missing `list` is treated as a malformed upstream shape.
fn map_ntes(
    data: Value,
    station: &str,
    date: Option<&str>,
) -> Result<StationTimetableResponse, AppError> {
    let list = data
        .get("list")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppError::internal("NTES: unexpected station-timetable shape"))?;

    Ok(StationTimetableResponse {
        station: opt_str_field(&data, "station").or_else(|| Some(station.to_string())),
        station_name: opt_str_field(&data, "stationName"),
        date: opt_str_field(&data, "date").or_else(|| date.map(str::to_string)),
        total: data
            .get("total")
            .and_then(Value::as_u64)
            .map(|v| v as usize),
        trains: Some(list.iter().map(map_train).collect()),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
    })
}

fn map_train(entry: &Value) -> StationTimetableTrain {
    StationTimetableTrain {
        number: str_field(entry, "trainNo"),
        name: str_field(entry, "trainName"),
        route: str_field(entry, "route"),
        train_type: str_field(entry, "trainType"),
        classes: str_field(entry, "classes"),
        arrival: str_field(entry, "arrival"),
        departure: str_field(entry, "departure"),
        days: days_field(entry),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn opt_str_field(entry: &Value, key: &str) -> Option<String> {
    entry.get(key).and_then(Value::as_str).map(str::to_string)
}

/// NTES renders the run-days cell as `- Fri -` / `- Daily -`; normalize away
/// the surrounding dashes so the wire model carries `Fri` / `Daily` / `Mon Wed Fri`.
fn days_field(entry: &Value) -> String {
    str_field(entry, "days")
        .trim_matches('-')
        .trim()
        .to_string()
}
