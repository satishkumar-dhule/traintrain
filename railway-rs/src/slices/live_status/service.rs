use std::time::Instant;

use serde_json::{json, Value};

use crate::core::error::AppError;
use crate::models::{LiveStatusResponse, LiveStop};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Resolve the live position of a running train.
    ///
    /// `date` is `YYYY-MM-DD` (optional; empty means today). A non-empty date
    /// that is neither today (UTC) nor the train's `train_start_date` is
    /// rejected - a past-day position is never invented.
    ///
    /// NTES (`enquiry.indianrail.gov.in`) is the primary source; Railyatri's
    /// SSR page is the fallback. The winning source is reported in
    /// `data_source`.
    pub async fn get_live_status(
        state: &AppState,
        train: &str,
        date: &str,
    ) -> Result<LiveStatusResponse, AppError> {
        let key = format!("live_status:{train}");
        if let Some(cached) = state.cache.get(&key) {
            if let Ok(resp) = map_response(&cached) {
                return Ok(resp);
            }
        }

        let ntes_started = Instant::now();
        let ntes_failure = match state.ntes.live_status(train, "").await {
            Ok(data) => {
                state
                    .metrics
                    .record_source_latency("ntes", ntes_started.elapsed());
                match ntes_norm(&data) {
                    Ok(norm) => match map_response(&norm) {
                        Ok(resp) => {
                            tracing::info!(
                                %train,
                                source = "NTES",
                                latency_ms = ntes_started.elapsed().as_millis(),
                                "live status resolved from NTES"
                            );
                            state.cache.set(&key, norm);
                            return Ok(resp);
                        }
                        Err(e) => e.message(),
                    },
                    Err(e) => e.message(),
                }
            }
            Err(e) => e.message(),
        };

        match railyatri_norm(state, train).await {
            Ok(norm) => {
                if !date.is_empty() && !matches_date(date, &norm) {
                    return Err(AppError::not_found(
                        "Live position is only available for today's run.",
                    ));
                }
                tracing::warn!(
                    %train,
                    source = "Railyatri",
                    %ntes_failure,
                    "live status resolved from Railyatri after NTES failure"
                );
                let resp = map_response(&norm)?;
                state.cache.set(&key, norm);
                Ok(resp)
            }
            Err(AppError::NotFound(msg)) => Err(AppError::not_found(msg)),
            Err(ry_err) => Err(AppError::source_unavailable(
                "all-sources",
                format!(
                    "live status for {train} failed: NTES: {ntes_failure} | Railyatri: {}",
                    ry_err.message()
                ),
            )),
        }
    }
}

/// Normalize a NTES `ShowFullRunJson` response into the shared normalized
/// shape `map_response` understands, tagging `data_source` so cache hits and
/// the wire model stay honest about which source served the data.
fn ntes_norm(data: &Value) -> Result<Value, AppError> {
    let stops = ["stationList", "trainStationList"]
        .iter()
        .find_map(|k| data.get(*k).and_then(Value::as_array))
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            json!({
                "name": field(&s, &["stationName", "stationname", "name"]),
                "code": field(&s, &["stationCode", "stationcode", "code"]),
                "arrival": field(&s, &["arrivalTime", "arrivaltime", "arrival"]),
                "actual_arrival": field(&s, &["actualArrival", "actualarrival"]),
            })
        })
        .filter(|s| {
            !s["name"].as_str().unwrap_or_default().is_empty()
                || !s["code"].as_str().unwrap_or_default().is_empty()
        })
        .collect::<Vec<_>>();

    let start_station = field(
        data,
        &["startStationCode", "trainStartCode", "startstationcode"],
    );
    let end_station = field(data, &["endStationCode", "endstationcode"]);
    let at_station = field(data, &["atStationCode", "atstationcode"]);
    let next_station_code = field(data, &["nextStationCode", "nextstationcode"]);

    // NTES reports the last departed station in `atStationCode`; a train is
    // still at its origin only when the next station has not moved past it.
    let at_src = !at_station.is_empty()
        && at_station == start_station
        && (next_station_code.is_empty() || next_station_code == start_station);

    Ok(json!({
        "train_number": field(data, &["trainNo", "trainno"]),
        "train_name": field(data, &["trainName", "trainname"]),
        "source_stn_name": field(data, &["startStationName", "startstationname"]),
        "dest_stn_name": field(data, &["endStationName", "endstationname"]),
        "next_station_name": field(data, &["nextStationName", "nextstationname"]),
        "next_station_code": next_station_code,
        "platform_number": field(data, &["platformNumber", "platformno", "platformNo"]),
        "at_src": at_src.to_string(),
        "at_dstn": (!at_station.is_empty() && at_station == end_station).to_string(),
        "train_start_date": field(data, &["trainStartDate", "startDate", "startdate"]),
        "data_source": "NTES",
        "stops": stops,
    }))
}

/// Fetch and normalize the Railyatri SSR live-status page.
async fn railyatri_norm(state: &AppState, train: &str) -> Result<Value, AppError> {
    let url = state.config.source_url(
        &state.config.railyatri_base,
        &format!("/live-train-status/{train}"),
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

    let norm = crate::core::railyatri::parse_live_status(&html)
        .map_err(|e| AppError::source_unavailable("Railyatri", e.message()))?;
    state
        .metrics
        .record_source_latency("railyatri", started.elapsed());
    Ok(norm)
}

/// `date` matches when it equals today (UTC) or the train's `train_start_date`.
fn matches_date(date: &str, norm: &Value) -> bool {
    use chrono::{Datelike, Utc};
    let now = Utc::now();
    let today = format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day());
    if date == today {
        return true;
    }
    match norm.get("train_start_date").and_then(Value::as_str) {
        Some(start) => date == start,
        None => false,
    }
}

/// Map normalized source data into the wire model, deriving honest statuses
/// from the real `next_station_code`. Never invents arrivals or delays; the
/// `actual_arrival` column is surfaced only when the source provides it
/// (NTES), and stays empty otherwise (Railyatri).
fn map_response(norm: &Value) -> Result<LiveStatusResponse, AppError> {
    let src = norm
        .get("data_source")
        .and_then(Value::as_str)
        .unwrap_or("Railyatri");
    let source_stn_name = str_at(norm, "source_stn_name");
    let dest_stn_name = str_at(norm, "dest_stn_name");
    let next_station_name = str_at(norm, "next_station_name");
    let next_station_code = str_at(norm, "next_station_code");
    let platform_number = str_at(norm, "platform_number");
    let at_src = norm.get("at_src").and_then(Value::as_str) == Some("true");
    let at_dstn = norm.get("at_dstn").and_then(Value::as_str) == Some("true");

    let mut location = if at_src {
        format!("Train at {source_stn_name} (origin).")
    } else if at_dstn {
        format!("Arrived at {dest_stn_name} (destination).")
    } else if !next_station_name.is_empty() {
        format!(
            "Running between {source_stn_name} and {dest_stn_name}; next station {next_station_name}."
        )
    } else {
        "Running; position awaiting update.".to_string()
    };
    if !platform_number.is_empty() {
        location.push_str(&format!(" Expected platform {platform_number}."));
    }

    let stops = norm
        .get("stops")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if stops.is_empty() {
        return Err(AppError::source_unavailable(
            src,
            "live status response contained no stops",
        ));
    }
    let next_idx = stops
        .iter()
        .position(|s| s.get("code").and_then(Value::as_str) == Some(next_station_code.as_str()));

    let stations: Vec<LiveStop> = stops
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let status = match next_idx {
                Some(n) if i < n => "departed",
                Some(n) if i == n => "expected",
                _ => "scheduled",
            };
            LiveStop {
                name: str_at(s, "name"),
                code: str_at(s, "code"),
                scheduled_arrival: str_at(s, "arrival"),
                actual_arrival: str_at(s, "actual_arrival"),
                delay_minutes: 0,
                status: status.to_string(),
            }
        })
        .collect();

    Ok(LiveStatusResponse {
        train_number: Some(str_at(norm, "train_number")),
        train_name: Some(str_at(norm, "train_name")),
        current_location_info: Some(location),
        data_source: Some(src.to_string()),
        stations: Some(stations),
    })
}

fn str_at(v: &Value, field: &str) -> String {
    v.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ntes_norm_marks_train_at_origin() {
        let data = json!({
            "trainNo": "12951",
            "startStationCode": "MMCT",
            "startStationName": "MUMBAI CENTRAL",
            "endStationCode": "NDLS",
            "endStationName": "NEW DELHI",
            "atStationCode": "MMCT",
            "nextStationCode": "MMCT",
            "stationList": [{"stationCode": "MMCT", "stationName": "MUMBAI CENTRAL"}]
        });
        let norm = ntes_norm(&data).unwrap();
        assert_eq!(norm["at_src"], "true");
        assert_eq!(norm["data_source"], "NTES");
        assert_eq!(norm["train_start_date"], "");
    }

    #[test]
    fn ntes_norm_running_train_is_not_at_origin() {
        let data = json!({
            "trainNo": "12951",
            "startStationCode": "MMCT",
            "endStationCode": "NDLS",
            "atStationCode": "MMCT",
            "nextStationCode": "BVI",
            "stationList": [{"stationCode": "MMCT"}, {"stationCode": "BVI"}]
        });
        let norm = ntes_norm(&data).unwrap();
        assert_eq!(norm["at_src"], "false");
        assert_eq!(norm["next_station_code"], "BVI");
    }

    #[test]
    fn map_response_surfaces_ntes_actuals_and_source() {
        let norm = json!({
            "train_number": "12951",
            "source_stn_name": "MUMBAI CENTRAL",
            "dest_stn_name": "NEW DELHI",
            "next_station_code": "BVI",
            "next_station_name": "BORIVALI",
            "at_src": "false",
            "at_dstn": "false",
            "data_source": "NTES",
            "stops": [
                {"name": "MUMBAI CENTRAL", "code": "MMCT", "arrival": "17:40", "actual_arrival": "17:40"},
                {"name": "BORIVALI", "code": "BVI", "arrival": "18:05", "actual_arrival": "18:15"},
                {"name": "NEW DELHI", "code": "NDLS", "arrival": "08:32", "actual_arrival": ""}
            ]
        });
        let resp = map_response(&norm).unwrap();
        assert_eq!(resp.data_source.as_deref(), Some("NTES"));
        let stations = resp.stations.unwrap();
        assert_eq!(stations[0].status, "departed");
        assert_eq!(stations[1].status, "expected");
        assert_eq!(stations[2].status, "scheduled");
        assert_eq!(stations[1].actual_arrival, "18:15");
        assert_eq!(stations[2].actual_arrival, "");
        assert!(resp.current_location_info.unwrap().contains("BORIVALI"));
    }
}
