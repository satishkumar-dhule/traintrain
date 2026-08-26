use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
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

        if state
            .failover
            .should_skip(crate::core::source::metric::NTES)
        {
            return Err(AppError::source_unavailable(
                crate::core::source::labels::NTES,
                "circuit open — ntes temporarily unavailable (cooldown)",
            ));
        }
        let ntes_started = Instant::now();
        let data = state
            .ntes_web
            .station_timetable(station, station_name, date.as_deref())
            .await
            .map_err(|e| {
                if matches!(
                    e,
                    AppError::SourceUnavailable { .. } | AppError::Internal(_)
                ) {
                    state
                        .failover
                        .record_failure(crate::core::source::metric::NTES);
                }
                e
            })?;
        state
            .metrics
            .record_source_latency(crate::core::source::metric::NTES, ntes_started.elapsed());
        state
            .failover
            .record_success(crate::core::source::metric::NTES);

        let date_label = date.as_deref().unwrap_or("any");
        let resp = map_ntes(data, station, date.as_deref())?;
        tracing::info!(
            %station,
            date = date_label,
            source = "NTES",
            latency_ms = ntes_started.elapsed().as_millis(),
            "station-timetable resolved from NTES"
        );
        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
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
