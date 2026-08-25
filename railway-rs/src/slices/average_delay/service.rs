use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::json::ValueExt;
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
        let cache_key = crate::core::cache::keys::average_delay(train);
        if let Some(resp) = state.cache.get_json(&cache_key) {
            return Ok(resp);
        }

        let ntes_started = Instant::now();
        let data = state.ntes_web.average_delay(train).await?;
        state
            .metrics
            .record_source_latency(crate::core::source::metric::NTES, ntes_started.elapsed());
        let resp = map_ntes(data)?;

        tracing::info!(
            %train,
            source = "NTES",
            latency_ms = ntes_started.elapsed().as_millis(),
            "average-delay resolved from NTES"
        );

        state.cache.set_json(&cache_key, &resp)?;
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
            sr: entry.str_field("sr"),
            name: entry.str_field("name"),
            code: entry.str_field("code"),
            arrival_delay: entry.str_field("arrivalDelay"),
            departure_delay: entry.str_field("departureDelay"),
        })
        .collect();

    Ok(AverageDelayResponse {
        train_no: Some(data.str_field("trainNo")),
        train_name: Some(data.str_field("trainName")),
        days_of_run: Some(data.str_field("daysOfRun")),
        train_type: Some(data.str_field("trainType")),
        stations: Some(stations),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
    })
}
