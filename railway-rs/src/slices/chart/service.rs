use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::irctc;
use crate::models::{ChartBerth, ChartCoach, ChartResponse};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Prepared-chart berth status for `train` on `date` from `station`,
    /// normalized from IRCTC's `trainComposition` response.
    pub async fn get_chart(
        state: &AppState,
        train: &str,
        date: &str,
        station: &str,
    ) -> Result<ChartResponse, AppError> {
        let cache_key = format!("irctc:chart:{train}:{date}:{station}");
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        let start = Instant::now();
        let data = state.irctc.train_composition(train, date, station).await?;
        state
            .metrics
            .record_source_latency("irctc", start.elapsed());

        let resp = map_response(data, train, date, station)?;
        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
    }
}

fn map_response(
    data: Value,
    train: &str,
    date: &str,
    station: &str,
) -> Result<ChartResponse, AppError> {
    let norm = irctc::normalize::chart(&data)?;

    let coaches: Vec<ChartCoach> = norm["coaches"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|c| ChartCoach {
                    code: str_field(c, "code"),
                    class_code: str_field(c, "class_code"),
                    berths: c["berths"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|b| ChartBerth {
                                    number: b["number"].as_i64().unwrap_or(0),
                                    status: str_field(b, "status"),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    if coaches.is_empty() {
        return Err(AppError::source_unavailable(
            irctc::client::SOURCE,
            "no coaches in trainComposition response",
        ));
    }

    Ok(ChartResponse {
        train_number: non_empty(&norm["train_number"]).or_else(|| Some(train.to_string())),
        train_name: non_empty(&norm["train_name"]),
        journey_date: Some(date.to_string()),
        boarding_station: if station.is_empty() {
            None
        } else {
            Some(station.to_string())
        },
        coaches: Some(coaches),
        data_source: Some(irctc::client::SOURCE.to_string()),
        notice: Some(
            "Live prepared-chart data from IRCTC online-charts (www.irctc.co.in). Charts are published only shortly before departure; before that the chart may be unavailable. IRCTC is IP-geofenced to India."
                .to_string(),
        ),
    })
}

fn non_empty(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(String::from)
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
