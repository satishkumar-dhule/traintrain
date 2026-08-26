use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::models::{ParcelResponse, ParcelTrain};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// All currently running parcel special trains from NTES.
    ///
    /// The final DTO (not the raw upstream payload) is cached, so a later hit
    /// works regardless of source availability. `data_source` honestly reports
    /// the source (`"NTES"`). When NTES is unreachable or the shape is
    /// unexpected the error is propagated as-is; no fallback exists for this
    /// slice.
    pub async fn get_parcel(state: &AppState) -> Result<ParcelResponse, AppError> {
        let cache_key = "parcel".to_string();
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
        let data = state.ntes_web.parcel_special_trains().await.map_err(|e| {
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

        let resp = map_ntes(data)?;

        tracing::info!(
            source = "NTES",
            latency_ms = ntes_started.elapsed().as_millis(),
            "parcel special trains resolved from NTES"
        );

        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        Ok(resp)
    }
}

fn map_ntes(data: Value) -> Result<ParcelResponse, AppError> {
    let list = data
        .get("list")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppError::internal("NTES: unexpected parcel shape"))?;

    let trains = list.iter().map(map_train).collect();
    Ok(ParcelResponse {
        trains: Some(trains),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
    })
}

fn map_train(entry: &Value) -> ParcelTrain {
    ParcelTrain {
        number: str_field(entry, "trainNo"),
        name: str_field(entry, "trainName"),
        route: str_field(entry, "route"),
        validity_from: str_field(entry, "validityFrom"),
        validity_to: str_field(entry, "validityTo"),
        days_of_run: str_field(entry, "daysOfRun"),
        source_code: str_field(entry, "srcCode"),
        source_time: str_field(entry, "srcTime"),
        dest_code: str_field(entry, "dstCode"),
        dest_time: str_field(entry, "dstTime"),
        travel_time: str_field(entry, "travelTime"),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
