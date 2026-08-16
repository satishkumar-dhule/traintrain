use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
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
        let cache_key = format!("average_delay:{train}");
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        let ntes_started = Instant::now();
        let data = state.ntes_web.average_delay(train).await?;
        state
            .metrics
            .record_source_latency("ntes", ntes_started.elapsed());
        let resp = map_ntes(data)?;

        tracing::info!(
            %train,
            source = "NTES",
            latency_ms = ntes_started.elapsed().as_millis(),
            "average-delay resolved from NTES"
        );

        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
    }
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
            sr: str_field(entry, "sr"),
            name: str_field(entry, "name"),
            code: str_field(entry, "code"),
            arrival_delay: str_field(entry, "arrivalDelay"),
            departure_delay: str_field(entry, "departureDelay"),
        })
        .collect();

    Ok(AverageDelayResponse {
        train_no: Some(str_field(&data, "trainNo")),
        train_name: Some(str_field(&data, "trainName")),
        days_of_run: Some(str_field(&data, "daysOfRun")),
        train_type: Some(str_field(&data, "trainType")),
        stations: Some(stations),
        data_source: Some("NTES".to_string()),
    })
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
