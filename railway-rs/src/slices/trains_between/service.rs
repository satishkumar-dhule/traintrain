use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::models::{BetweenTrain, TrainsBetweenResponse};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Direct trains between two station codes.
    pub async fn get_trains_between(
        state: &AppState,
        src: &str,
        dst: &str,
    ) -> Result<TrainsBetweenResponse, AppError> {
        let cache_key = format!("trains_between:{src}:{dst}");
        if let Some(cached) = state.cache.get(&cache_key) {
            return map_response(cached, src, dst);
        }

        let start = Instant::now();
        let data = state.ntes.trains_between(src, dst, "XXX").await;
        state.metrics.record_source_latency("ntes", start.elapsed());
        let data = data?;

        let resp = map_response(data.clone(), src, dst)?;
        state.cache.set(&cache_key, data);
        Ok(resp)
    }
}

fn map_response(data: Value, src: &str, dst: &str) -> Result<TrainsBetweenResponse, AppError> {
    let list = data
        .get("trainBtwStationList")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .or_else(|| {
            data.get("trainList")
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
        })
        .ok_or_else(|| AppError::internal("NTES: unexpected TrainBtwStnJson shape"))?;

    let trains = list.iter().map(map_train).collect();
    Ok(TrainsBetweenResponse {
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        trains: Some(trains),
        data_source: Some("NTES".to_string()),
    })
}

fn map_train(entry: &Value) -> BetweenTrain {
    BetweenTrain {
        number: str_field(entry, "trainNo"),
        name: str_field(entry, "trainName"),
        departure_time: str_field(entry, "depTime"),
        arrival_time: str_field(entry, "arrTime"),
        runs_on: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .into_iter()
            .map(|day| day_bool(entry, day))
            .collect(),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Accept both the documented `runOn<Day>` and community `runsOn<Day>` spellings.
fn day_bool(entry: &Value, day: &str) -> bool {
    entry
        .get(format!("runOn{day}"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || entry
            .get(format!("runsOn{day}"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}
