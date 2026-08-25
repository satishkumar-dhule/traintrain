use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
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

        let ntes_started = Instant::now();
        let data = state.ntes_web.heritage_trains(selection).await?;
        state
            .metrics
            .record_source_latency(crate::core::source::metric::NTES, ntes_started.elapsed());
        let resp = map_ntes(data)?;

        tracing::info!(
            selection,
            source = "NTES",
            latency_ms = ntes_started.elapsed().as_millis(),
            "heritage trains resolved from NTES"
        );
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
