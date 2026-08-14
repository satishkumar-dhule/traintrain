use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::railyatri;
use crate::models::{ScheduleResponse, ScheduleStop};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Resolve the full schedule (route + running days) for a train number.
    ///
    /// NTES (`enquiry.indianrail.gov.in`) is the primary source; Railyatri's
    /// SSR page is the fallback when NTES is unreachable or malformed. The
    /// winning source is reported honestly in `data_source`.
    pub async fn get_schedule(state: &AppState, train: &str) -> Result<ScheduleResponse, AppError> {
        let key = format!("schedule:{train}");

        let ntes_started = Instant::now();
        let ntes_failure = match state.ntes.schedule(train, "").await {
            Ok(data) => {
                state
                    .metrics
                    .record_source_latency("ntes", ntes_started.elapsed());
                match ntes_schedule_response(train, &data, state.config.cache_ttl.as_secs()) {
                    Ok(resp) => {
                        tracing::info!(
                            %train,
                            source = "NTES",
                            latency_ms = ntes_started.elapsed().as_millis(),
                            "schedule resolved from NTES"
                        );
                        state.cache.set(&key, serde_json::to_value(&resp)?);
                        return Ok(resp);
                    }
                    Err(e) => e.message(),
                }
            }
            Err(e) => e.message(),
        };

        match railyatri_schedule(state, train).await {
            Ok(resp) => {
                tracing::warn!(
                    %train,
                    source = "Railyatri",
                    %ntes_failure,
                    "schedule resolved from Railyatri after NTES failure"
                );
                state.cache.set(&key, serde_json::to_value(&resp)?);
                Ok(resp)
            }
            Err(AppError::NotFound(msg)) => Err(AppError::not_found(msg)),
            Err(ry_err) => Err(AppError::source_unavailable(
                "all-sources",
                format!(
                    "schedule for {train} failed: NTES: {ntes_failure} | Railyatri: {}",
                    ry_err.message()
                ),
            )),
        }
    }
}

/// Normalize a NTES `GetTrainSchedule` response into the wire model.
fn ntes_schedule_response(
    train: &str,
    data: &Value,
    cache_ttl: u64,
) -> Result<ScheduleResponse, AppError> {
    let list = [
        "trainScheduleList",
        "trainStnList",
        "stationList",
        "trainSchedule",
    ]
    .iter()
    .find_map(|k| data.get(*k).and_then(Value::as_array))
    .filter(|a| !a.is_empty())
    .ok_or_else(|| {
        AppError::source_unavailable("NTES", "unexpected GetTrainSchedule response shape")
    })?;

    let stops: Vec<ScheduleStop> = list
        .iter()
        .map(|s| ScheduleStop {
            code: field(s, &["stationCode", "stationcode", "code"]),
            name: field(s, &["stationName", "stationname", "name"]),
            arrival: field(s, &["arrivalTime", "arrivaltime", "arrival"]),
            departure: field(
                s,
                &["departureTime", "departuretime", "departure", "depTime"],
            ),
            day: int_field(s, &["day", "stopDay"]).unwrap_or(1),
        })
        .filter(|s| !s.code.is_empty() || !s.name.is_empty())
        .collect();

    if stops.is_empty() {
        return Err(AppError::source_unavailable("NTES", "no stops in schedule"));
    }

    Ok(ScheduleResponse {
        train_number: Some(non_empty(&data["trainNo"]).unwrap_or_else(|| train.to_string())),
        train_name: non_empty(&data["trainName"]),
        route_description: None,
        running_days: None,
        stops: Some(stops),
        source: Some("NTES".to_string()),
        cache_ttl: Some(cache_ttl),
        notice: Some(
            "Live data from NTES, the official Indian Railways enquiry system (enquiry.indianrail.gov.in)."
                .to_string(),
        ),
        data_source: Some("NTES".to_string()),
    })
}

/// Fetch and normalize the Railyatri SSR schedule page.
async fn railyatri_schedule(state: &AppState, train: &str) -> Result<ScheduleResponse, AppError> {
    let url = state.config.source_url(
        &state.config.railyatri_base,
        &format!("/time-table/{train}"),
    );

    let started = Instant::now();
    let res = state
        .http
        .inner()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::source_unavailable("Railyatri", format!("GET {url}: {e}")))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::not_found(format!("Train {train} not found.")));
    }
    if !status.is_success() {
        return Err(AppError::source_unavailable(
            "Railyatri",
            format!("GET {url} returned {status}"),
        ));
    }
    let html = res.text().await.map_err(|e| {
        AppError::source_unavailable("Railyatri", format!("read body of {url}: {e}"))
    })?;

    let _nd = railyatri::extract_next_data(&html)
        .map_err(|e| AppError::source_unavailable("Railyatri", e.message()))?;
    let norm = railyatri::parse_schedule(&html)
        .map_err(|e| AppError::source_unavailable("Railyatri", e.message()))?;
    state
        .metrics
        .record_source_latency("railyatri", started.elapsed());

    let stop_values = norm["stops"].as_array().cloned().unwrap_or_default();
    if stop_values.is_empty() {
        return Err(AppError::source_unavailable(
            "Railyatri",
            "no stops in timetable",
        ));
    }
    let stops = stop_values
        .iter()
        .map(|s| ScheduleStop {
            code: s["code"].as_str().unwrap_or_default().to_string(),
            name: s["name"].as_str().unwrap_or_default().to_string(),
            arrival: s["arrival"].as_str().unwrap_or_default().to_string(),
            departure: s["departure"].as_str().unwrap_or_default().to_string(),
            day: s["day"].as_i64().unwrap_or(1),
        })
        .collect();

    let running_days = norm["run_days"]
        .as_array()
        .map(|days| {
            days.iter()
                .filter_map(|d| d.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(ScheduleResponse {
        train_number: non_empty(&norm["train_number"]),
        train_name: non_empty(&norm["train_name"]),
        route_description: non_empty(&norm["route_description"]),
        running_days: Some(running_days),
        stops: Some(stops),
        source: Some("Railyatri".to_string()),
        cache_ttl: Some(state.config.cache_ttl.as_secs()),
        notice: Some("Live data extracted from Railyatri.".to_string()),
        data_source: Some("Railyatri".to_string()),
    })
}

fn non_empty(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(String::from)
}

fn field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn int_field(v: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|k| match v.get(*k) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ntes_normalizer_accepts_documented_shape() {
        let data = json!({
            "trainNo": "12951",
            "trainName": "MUMBAI RAJDHANI",
            "trainScheduleList": [
                {"stationCode": "MMCT", "stationName": "MUMBAI CENTRAL", "arrivalTime": "--", "departureTime": "17:40", "day": 1},
                {"stationCode": "NDLS", "stationName": "NEW DELHI", "arrivalTime": "08:32", "departureTime": "--", "day": 2}
            ]
        });
        let resp = ntes_schedule_response("12951", &data, 120).unwrap();
        assert_eq!(resp.data_source.as_deref(), Some("NTES"));
        assert_eq!(resp.train_number.as_deref(), Some("12951"));
        let stops = resp.stops.unwrap();
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0].code, "MMCT");
        assert_eq!(stops[0].departure, "17:40");
        assert_eq!(stops[1].day, 2);
    }

    #[test]
    fn ntes_normalizer_rejects_empty_schedule() {
        let data = json!({ "trainNo": "12951", "trainScheduleList": [] });
        let err = ntes_schedule_response("12951", &data, 120).unwrap_err();
        assert!(matches!(err, AppError::SourceUnavailable { source, .. } if source == "NTES"));
    }
}
