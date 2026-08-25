use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::models::{LiveStationResponse, StationTrain};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Trains expected at a station within `hours` (NTES supports 2, 4, or 8).
    /// `destination` is the optional "Going to station" filter forwarded to
    /// the NTES form (`jToStationInput`) so the source itself narrows the
    /// board; `None` returns the unfiltered board.
    pub async fn get_live_station(
        state: &AppState,
        station: &str,
        hours: u32,
        destination: Option<&str>,
    ) -> Result<LiveStationResponse, AppError> {
        let key = match destination {
            Some(dest) => format!("live_station:{station}:{hours}:to-{dest}"),
            None => format!("live_station:{station}:{hours}"),
        };
        if let Some(cached) = state.cache.get(&key) {
            if let Some(resp) = build_response(station, destination, hours, &cached) {
                return Ok(resp);
            }
        }

        let start = Instant::now();
        let name = state
            .datasets
            .station_name(station)
            .unwrap_or(station)
            .to_string();
        // The form wants the destination as its `CODE - NAME` pair; resolve
        // the official name from the same dataset the browser list uses.
        let dest_pair = destination.map(|dest| (dest, state.datasets.station_name(dest).unwrap_or(dest)));
        let data = state
            .ntes_web
            .live_station(station, &name, hours, dest_pair)
            .await;
        state.metrics.record_source_latency("ntes", start.elapsed());
        let data = data?;

        state.cache.set(&key, data.clone());
        build_response(station, destination, hours, &data)
            .ok_or_else(|| AppError::internal("NTES: unexpected TrainsAtStationJson shape"))
    }
}

fn build_response(
    station: &str,
    destination: Option<&str>,
    hours: u32,
    data: &Value,
) -> Option<LiveStationResponse> {
    let list = ["trainList", "trainsList", "trainBtwStationList"]
        .iter()
        .find_map(|k| {
            data.get(*k)
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
        })?;
    Some(LiveStationResponse {
        station: Some(station.to_string()),
        destination: destination.map(str::to_string),
        hours: Some(hours as u8),
        trains: Some(list.iter().map(station_train).collect()),
        data_source: Some("NTES".to_string()),
    })
}

fn station_train(v: &Value) -> StationTrain {
    StationTrain {
        number: v
            .get("trainNo")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        name: v
            .get("trainName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        sta: v
            .get("scheduledTime")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        eta: v
            .get("expectedTime")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        delay_arr: v
            .get("delayArr")
            .and_then(Value::as_bool)
            .unwrap_or_default(),
        platform: v
            .get("platformNo")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    }
}
