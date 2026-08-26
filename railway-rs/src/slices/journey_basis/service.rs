use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::fanout::{Candidate, fanout_n2};
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
        let cache_key = format!("journey_stations:{train}");
        if let Some(cached) = state.cache.get_json(&cache_key) {
            return Ok(cached);
        }

        // Super fan-out N²: NTES journey_stations (2 delegates: same train, duplicated for N=2)
        // raced concurrently, each retried. First success wins; static local fallback
        // guarantees the UI never sees a 30s hang when NTES is IP-blocked in Singapore.
        let train1 = train.to_string();
        let train2 = train.to_string();
        let state1 = state.clone();
        let state2 = state.clone();
        let candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state1.clone();
                let t = train1.clone();
                async move { s.ntes_web.journey_stations(&t).await }
            }),
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state2.clone();
                let t = train2.clone();
                async move { s.ntes_web.journey_stations(&t).await }
            }),
        ];
        let data = match fanout_n2(state, candidates, &format!("journey_stations:{train}")).await {
            Ok((_, v)) => v,
            Err(e) if matches!(e, AppError::NotFound(_)) => return Err(e),
            Err(e) => {
                let msg = e.message().to_lowercase();
                let is_timeout = msg.contains("timeout") || msg.contains("circuit open") || msg.contains("overall timeout");
                if !is_timeout {
                    return Err(e);
                }
                tracing::warn!(train, err=%e.message(), "journey_stations: live timed out, serving static empty");
                let resp = JourneyStationsResponse {
                    train_no: Some(train.to_string()),
                    stations: Some(Vec::new()),
                    data_source: Some("local".to_string()),
                };
                let _ = state.cache.set_json(&cache_key, &resp);
                return Ok(resp);
            }
        };
        let stations: Vec<JourneyStationInfo> = data
            .get("list")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(journey_station_info)
            .collect();
        let resp = JourneyStationsResponse {
            train_no: Some(
                data.get("trainNo")
                    .and_then(Value::as_str)
                    .unwrap_or(train)
                    .to_string(),
            ),
            stations: Some(stations),
            data_source: Some(crate::core::source::labels::NTES.to_string()),
        };
        let _ = state.cache.set_json(&cache_key, &resp);
        Ok(resp)
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

        let cache_key = keys::journey_basis(train, station);
        if let Some(cached) = state.cache.get_json(&cache_key) {
            return Ok(cached);
        }

        // First NTES call: journey_stations (fan-out N², 2 delegates). If it fails,
        // return a synthetic response with data_source "local" so the UI never hangs.
        let train_a1 = train.to_string();
        let train_a2 = train.to_string();
        let state_a1 = state.clone();
        let state_a2 = state.clone();
        let candidates_a = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_a1.clone();
                let t = train_a1.clone();
                async move { s.ntes_web.journey_stations(&t).await }
            }),
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_a2.clone();
                let t = train_a2.clone();
                async move { s.ntes_web.journey_stations(&t).await }
            }),
        ];
        let list = match fanout_n2(state, candidates_a, &format!("journey_basis_stations:{train}")).await {
            Ok((_, v)) => v,
            Err(e) if matches!(e, AppError::NotFound(_)) => return Err(e),
            Err(e) => {
                let msg = e.message().to_lowercase();
                let is_timeout = msg.contains("timeout") || msg.contains("circuit open") || msg.contains("overall timeout");
                if !is_timeout {
                    return Err(e);
                }
                tracing::warn!(train, station, err=%e.message(), "journey_basis: journey_stations fan-out timed out, serving static empty");
                let resp = JourneyBasisResponse {
                    status: crate::models::LiveStatusResponse {
                        train_number: Some(train.to_string()),
                        data_source: Some("local".to_string()),
                        stations: Some(Vec::new()),
                        ..Default::default()
                    },
                    journey_station: None,
                };
                let _ = state.cache.set_json(&cache_key, &resp);
                return Ok(resp);
            }
        };
        let info = journey_station_matching(&list, station).ok_or_else(|| {
            AppError::bad_request(format!(
                "station {station} is not on the route of train {train}"
            ))
        })?;
        let j_station_value = format!("{}#{}#{}", info.code, info.day_change, info.seq);

        // Second NTES call: journey_station_basis (fan-out N², 2 delegates with same jStation value).
        let train_b1 = train.to_string();
        let train_b2 = train.to_string();
        let js1 = j_station_value.clone();
        let js2 = j_station_value.clone();
        let state_b1 = state.clone();
        let state_b2 = state.clone();
        let candidates_b = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_b1.clone();
                let t = train_b1.clone();
                let js = js1.clone();
                async move { s.ntes_web.journey_station_basis(&t, &js).await }
            }),
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_b2.clone();
                let t = train_b2.clone();
                let js = js2.clone();
                async move { s.ntes_web.journey_station_basis(&t, &js).await }
            }),
        ];
        let norm = match fanout_n2(state, candidates_b, &format!("journey_basis:{train}:{station}")).await {
            Ok((_, v)) => v,
            Err(e) if matches!(e, AppError::NotFound(_)) => return Err(e),
            Err(e) => {
                let msg = e.message().to_lowercase();
                let is_timeout = msg.contains("timeout") || msg.contains("circuit open") || msg.contains("overall timeout");
                if !is_timeout {
                    return Err(e);
                }
                tracing::warn!(train, station, js=%j_station_value, err=%e.message(), "journey_basis: journey_station_basis fan-out timed out, serving static empty");
                let resp = JourneyBasisResponse {
                    status: crate::models::LiveStatusResponse {
                        train_number: Some(train.to_string()),
                        data_source: Some("local".to_string()),
                        stations: Some(Vec::new()),
                        ..Default::default()
                    },
                    journey_station: Some(info),
                };
                let _ = state.cache.set_json(&cache_key, &resp);
                return Ok(resp);
            }
        };
        let status = map_response(&norm)?;
        let resp = JourneyBasisResponse {
            status,
            journey_station: Some(info),
        };
        state.cache.set_json(&cache_key, &resp)?;
        Ok(resp)
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
