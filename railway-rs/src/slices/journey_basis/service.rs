use serde_json::Value;

use crate::core::error::AppError;
use crate::models::{JourneyBasisResponse, JourneyStationInfo, JourneyStationsResponse};
use crate::slices::live_status::mapping::map_response;
use crate::state::AppState;

pub struct Service;

impl Service {
    /// The journey stations NTES offers for `train`, as select options for the
    /// "Journey Station Basis" second mode of Spot Your Train.
    pub async fn get_journey_stations(
        state: &AppState,
        train: &str,
    ) -> Result<JourneyStationsResponse, AppError> {
        let norm = state.ntes_web.journey_stations(train).await?;
        let stations: Vec<JourneyStationInfo> = norm
            .get("list")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(journey_station_info)
            .collect();
        Ok(JourneyStationsResponse {
            train_no: Some(
                norm.get("trainNo")
                    .and_then(Value::as_str)
                    .unwrap_or(train)
                    .to_string(),
            ),
            stations: Some(stations),
            data_source: Some("ntes".to_string()),
        })
    }

    /// The journey-station-basis live run for `train` as seen from `station`.
    ///
    /// NTES auto-selects the run nearest today, so `date` (when supplied) is
    /// accepted but not sent upstream.
    pub async fn get_journey_basis(
        state: &AppState,
        train: &str,
        station: &str,
        date: Option<&str>,
    ) -> Result<JourneyBasisResponse, AppError> {
        let _ = date; // NTES auto-selects the run nearest today via the minimal ShowRunCStn body.
        let list = state.ntes_web.journey_stations(train).await?;
        let info = journey_station_matching(&list, station).ok_or_else(|| {
            AppError::bad_request(format!(
                "station {station} is not on the route of train {train}"
            ))
        })?;
        let j_station_value = format!("{}#{}#{}", info.code, info.day_change, info.seq);
        let norm = state
            .ntes_web
            .journey_station_basis(train, &j_station_value)
            .await?;
        let status = map_response(&norm)?;
        Ok(JourneyBasisResponse {
            status,
            journey_station: Some(info),
        })
    }
}

fn journey_station_info(v: &Value) -> JourneyStationInfo {
    JourneyStationInfo {
        code: str_field(v, "code"),
        name: str_field(v, "name"),
        seq: v
            .get("seq")
            .and_then(Value::as_u64)
            .map(|n| n as usize)
            .unwrap_or_default(),
        day_change: v.get("dayChange").and_then(Value::as_bool).unwrap_or(false),
        arrival_days: str_field(v, "arrivalDays"),
        departure_days: str_field(v, "departureDays"),
    }
}

fn journey_station_matching(list: &Value, station: &str) -> Option<JourneyStationInfo> {
    list.get("list")
        .and_then(Value::as_array)?
        .iter()
        .map(journey_station_info)
        .find(|s| s.code.eq_ignore_ascii_case(station))
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
