use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::irctc;
use crate::models::{AvailabilityResponse, AvailabilityTrain};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Direct trains with availability between `src` and `dst` on `date`
    /// (`YYYY-MM-DD`), normalized from IRCTC's `altAvlEnq/TC` response.
    pub async fn get_availability(
        state: &AppState,
        src: &str,
        dst: &str,
        date: &str,
    ) -> Result<AvailabilityResponse, AppError> {
        let cache_key = format!("irctc:availability:{src}:{dst}:{date}");
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        let start = Instant::now();
        let data = state.irctc.availability(src, dst, date).await?;
        state
            .metrics
            .record_source_latency("irctc", start.elapsed());

        let resp = map_response(data, src, dst, date)?;
        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
    }
}

fn map_response(
    data: Value,
    src: &str,
    dst: &str,
    date: &str,
) -> Result<AvailabilityResponse, AppError> {
    let norm = irctc::normalize::availability_trains(&data)?;
    let trains: Vec<AvailabilityTrain> = norm["trains"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|t| AvailabilityTrain {
                    number: str_field(t, "number"),
                    name: str_field(t, "name"),
                    from_code: str_field(t, "from_code"),
                    from_name: str_field(t, "from_name"),
                    to_code: str_field(t, "to_code"),
                    to_name: str_field(t, "to_name"),
                    departure_time: str_field(t, "departure_time"),
                    arrival_time: str_field(t, "arrival_time"),
                    duration: str_field(t, "duration"),
                    distance: str_field(t, "distance"),
                    classes: t["classes"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .map(String::from)
                                .collect()
                        })
                        .unwrap_or_default(),
                    train_type: str_field(t, "train_type"),
                    runs_on: t["runs_on"]
                        .as_array()
                        .map(|a| a.iter().filter_map(Value::as_bool).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    if trains.is_empty() {
        return Err(AppError::source_unavailable(
            irctc::client::SOURCE,
            "no trains with availability in altAvlEnq response",
        ));
    }

    Ok(AvailabilityResponse {
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        date: Some(date.to_string()),
        trains: Some(trains),
        data_source: Some(irctc::client::SOURCE.to_string()),
        notice: Some(
            "Live availability from IRCTC (www.irctc.co.in), the official Indian Railways booking portal. IRCTC is IP-geofenced to India."
                .to_string(),
        ),
    })
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
