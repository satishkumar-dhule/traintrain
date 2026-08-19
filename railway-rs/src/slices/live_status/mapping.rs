//! Shared mapping of a normalized NTES status payload (the JSON the web-form
//! client emits, e.g. from `train_status` / `journey_station_basis`) into the
//! `LiveStatusResponse` wire model.
//!
//! Both the `live_status` slice and the `journey_basis` slice consume this so
//! the honest status derivation (departed / expected / scheduled) and the
//! `current_location_info` phrasing are computed once, in one place.

use serde_json::Value;

use crate::core::error::AppError;
use crate::models::{LiveStatusResponse, LiveStop, TrainInstance};

/// Map normalized source data into the wire model, deriving honest statuses
/// from the real `next_station_code`. Never invents arrivals; `actual_arrival`
/// is surfaced only when the source provides it (NTES), and stays empty
/// otherwise (Railyatri). `delay_minutes` prefers the source's own per-stop
/// delay when present (the NTES web form reports the badge, which is accurate
/// across midnight); otherwise it is derived from a real NTES
/// `actual_arrival` vs `arrivalTime` when the delta is unambiguous (0-12h
/// late) - never guessed.
pub fn map_response(norm: &Value) -> Result<LiveStatusResponse, AppError> {
    let src = norm
        .get("data_source")
        .and_then(Value::as_str)
        .unwrap_or("Railyatri");
    let source_stn_name = str_at(norm, "source_stn_name");
    let dest_stn_name = str_at(norm, "dest_stn_name");
    let next_station_name = str_at(norm, "next_station_name");
    let next_station_code = str_at(norm, "next_station_code");
    let platform_number = str_at(norm, "platform_number");
    let at_src = norm.get("at_src").and_then(Value::as_str) == Some("true");
    let at_dstn = norm.get("at_dstn").and_then(Value::as_str) == Some("true");

    let mut location = if at_src {
        format!("Train at {source_stn_name} (origin).")
    } else if at_dstn {
        format!("Arrived at {dest_stn_name} (destination).")
    } else if !next_station_name.is_empty() {
        format!(
            "Running between {source_stn_name} and {dest_stn_name}; next station {next_station_name}."
        )
    } else {
        "Running; position awaiting update.".to_string()
    };
    if !platform_number.is_empty() {
        location.push_str(&format!(" Expected platform {platform_number}."));
    }

    let stops = norm
        .get("stops")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if stops.is_empty() {
        return Err(AppError::source_unavailable(
            src,
            "live status response contained no stops",
        ));
    }
    let next_idx = stops
        .iter()
        .position(|s| s.get("code").and_then(Value::as_str) == Some(next_station_code.as_str()));

    let stations: Vec<LiveStop> = stops
        .iter()
        .enumerate()
        .map(|(i, s)| {
            // A completed run (at destination) has no next station to key on,
            // so every stop is stamped departed rather than "scheduled".
            let status = if at_dstn {
                "departed"
            } else {
                match next_idx {
                    Some(n) if i < n => "departed",
                    Some(n) if i == n => "expected",
                    Some(_) => "scheduled",
                    None if at_src && i == 0 => "expected",
                    None => "scheduled",
                }
            };
            let scheduled = str_at(s, "arrival");
            let actual = str_at(s, "actual_arrival");
            LiveStop {
                name: str_at(s, "name"),
                code: str_at(s, "code"),
                scheduled_arrival: scheduled.clone(),
                actual_arrival: actual.clone(),
                delay_minutes: s
                    .get("delay_minutes")
                    .and_then(Value::as_i64)
                    .unwrap_or_else(|| {
                        if actual.is_empty() {
                            0
                        } else {
                            delay_minutes(&scheduled, &actual)
                        }
                    }),
                status: status.to_string(),
            }
        })
        .collect();

    let instances: Vec<TrainInstance> = norm
        .get("instances")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|i| {
            let inst_stops: Option<Vec<LiveStop>> = i
                .get("stops")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .enumerate()
                        .map(|(idx, s)| {
                            let next_code = str_at(s, "next_station_code");
                            let inst_next_idx = arr.iter().position(|ss| {
                                ss.get("code").and_then(Value::as_str) == Some(next_code.as_str())
                            });
                            let status = if i.get("at_dstn").and_then(Value::as_str) == Some("true")
                            {
                                "departed"
                            } else {
                                match inst_next_idx {
                                    Some(n) if idx < n => "departed",
                                    Some(n) if idx == n => "expected",
                                    Some(_) => "scheduled",
                                    None if i.get("at_src").and_then(Value::as_str)
                                        == Some("true")
                                        && idx == 0 =>
                                    {
                                        "expected"
                                    }
                                    None => "scheduled",
                                }
                            };
                            let scheduled = str_at(s, "arrival");
                            let actual = str_at(s, "actual_arrival");
                            LiveStop {
                                name: str_at(s, "name"),
                                code: str_at(s, "code"),
                                scheduled_arrival: scheduled.clone(),
                                actual_arrival: actual.clone(),
                                delay_minutes: s
                                    .get("delay_minutes")
                                    .and_then(Value::as_i64)
                                    .unwrap_or_else(|| {
                                        if actual.is_empty() {
                                            0
                                        } else {
                                            delay_minutes(&scheduled, &actual)
                                        }
                                    }),
                                status: status.to_string(),
                            }
                        })
                        .collect()
                })
                .filter(|v: &Vec<LiveStop>| !v.is_empty());
            TrainInstance {
                start_date: str_at(&i, "start_date"),
                position: str_at(&i, "position"),
                stops: inst_stops,
            }
        })
        .collect();

    let train_start_date = str_at(norm, "train_start_date");
    Ok(LiveStatusResponse {
        train_number: Some(str_at(norm, "train_number")),
        train_name: Some(str_at(norm, "train_name")),
        current_location_info: Some(location),
        train_start_date: (!train_start_date.is_empty()).then_some(train_start_date),
        instances: (!instances.is_empty()).then_some(instances),
        data_source: Some(src.to_string()),
        stations: Some(stations),
    })
}

pub fn str_at(v: &Value, field: &str) -> String {
    v.get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Honest delay minutes from a real NTES actual arrival: only when both
/// times parse as `HH:MM` and the actual is 0-12h later than scheduled
/// (longer gaps mean a cross-midnight day boundary and cannot be judged
/// from bare times alone). Early arrivals and unknowns report 0.
fn delay_minutes(scheduled: &str, actual: &str) -> i64 {
    let (Some((sh, sm)), Some((ah, am))) = (parse_hhmm(scheduled), parse_hhmm(actual)) else {
        return 0;
    };
    let s = sh as i64 * 60 + sm as i64;
    let a = ah as i64 * 60 + am as i64;
    let delta = a - s;
    if (0..=720).contains(&delta) {
        delta
    } else {
        0
    }
}

fn parse_hhmm(t: &str) -> Option<(u32, u32)> {
    let (h, m) = t.split_once(':')?;
    Some((h.parse().ok()?, m.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn map_response_surfaces_ntes_actuals_and_source() {
        let norm = json!({
            "train_number": "12951",
            "source_stn_name": "MUMBAI CENTRAL",
            "dest_stn_name": "NEW DELHI",
            "next_station_code": "BVI",
            "next_station_name": "BORIVALI",
            "at_src": "false",
            "at_dstn": "false",
            "data_source": "NTES",
            "stops": [
                {"name": "MUMBAI CENTRAL", "code": "MMCT", "arrival": "17:40", "actual_arrival": "17:40"},
                {"name": "BORIVALI", "code": "BVI", "arrival": "18:05", "actual_arrival": "18:15"},
                {"name": "NEW DELHI", "code": "NDLS", "arrival": "08:32", "actual_arrival": ""}
            ]
        });
        let resp = map_response(&norm).unwrap();
        assert_eq!(resp.data_source.as_deref(), Some("NTES"));
        let stations = resp.stations.unwrap();
        assert_eq!(stations[0].status, "departed");
        assert_eq!(stations[1].status, "expected");
        assert_eq!(stations[2].status, "scheduled");
        assert_eq!(stations[1].actual_arrival, "18:15");
        assert_eq!(stations[2].actual_arrival, "");
        assert!(resp.current_location_info.unwrap().contains("BORIVALI"));
    }

    #[test]
    fn map_response_derives_delay_only_from_real_actuals() {
        let norm = json!({
            "train_number": "12951",
            "at_src": "false",
            "at_dstn": "false",
            "data_source": "NTES",
            "stops": [
                {"name": "MUMBAI CENTRAL", "code": "MMCT", "arrival": "17:40", "actual_arrival": "17:40"},
                {"name": "BORIVALI", "code": "BVI", "arrival": "18:05", "actual_arrival": "18:15"},
                {"name": "NEW DELHI", "code": "NDLS", "arrival": "08:32", "actual_arrival": ""}
            ]
        });
        let resp = map_response(&norm).unwrap();
        let stations = resp.stations.unwrap();
        assert_eq!(stations[0].delay_minutes, 0, "on-time actual -> 0");
        assert_eq!(stations[1].delay_minutes, 10, "real 10-minute late actual");
        assert_eq!(stations[2].delay_minutes, 0, "no actual -> never invented");
    }

    #[test]
    fn map_response_prefers_explicit_per_stop_delay() {
        // The NTES web form reports the delay badge per arrived stop, which is
        // accurate across midnight where the bare-times derivation is not.
        let norm = json!({
            "train_number": "12055",
            "at_src": "false",
            "at_dstn": "false",
            "data_source": "NTES",
            "stops": [
                {"name": "GHAZIABAD", "code": "GZB", "arrival": "15:53", "actual_arrival": "15:56", "delay_minutes": 3},
                {"name": "MEERUT CITY", "code": "MTC", "arrival": "16:32", "actual_arrival": "", "delay_minutes": 0}
            ]
        });
        let resp = map_response(&norm).unwrap();
        let stations = resp.stations.unwrap();
        assert_eq!(stations[0].delay_minutes, 3, "badge wins over derivation");
        assert_eq!(stations[1].delay_minutes, 0, "no arrival -> 0");
    }

    #[test]
    fn map_response_surfaces_run_dates_from_ntes() {
        let norm = json!({
            "train_number": "12951",
            "train_start_date": "02-May-2026",
            "at_src": "true",
            "at_dstn": "false",
            "data_source": "NTES",
            "instances": [
                {"start_date": "02-May-2026", "position": "Yet to start from its source"},
                {"start_date": "01-May-2026", "position": "Running"}
            ],
            "stops": [
                {"name": "MUMBAI CENTRAL", "code": "MMCT", "arrival": "17:40", "actual_arrival": "17:40"}
            ]
        });
        let resp = map_response(&norm).unwrap();
        assert_eq!(resp.train_start_date.as_deref(), Some("02-May-2026"));
        let instances = resp.instances.unwrap();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].start_date, "02-May-2026");
        assert_eq!(instances[0].position, "Yet to start from its source");
        assert_eq!(instances[1].start_date, "01-May-2026");
    }

    #[test]
    fn map_response_drops_absent_instances() {
        let norm = json!({
            "train_number": "12951",
            "at_src": "false",
            "at_dstn": "false",
            "data_source": "Railyatri",
            "stops": [
                {"name": "MUMBAI CENTRAL", "code": "MMCT", "arrival": "17:40", "actual_arrival": ""}
            ]
        });
        let resp = map_response(&norm).unwrap();
        assert!(resp.instances.is_none());
        assert!(resp.train_start_date.is_none());
    }

    #[test]
    fn map_response_arrived_train_is_all_departed() {
        let norm = json!({
            "train_number": "12951",
            "source_stn_name": "MUMBAI CENTRAL",
            "dest_stn_name": "NEW DELHI",
            "at_src": "false",
            "at_dstn": "true",
            "data_source": "NTES",
            "stops": [
                {"name": "MUMBAI CENTRAL", "code": "MMCT", "arrival": "17:40", "actual_arrival": "17:40"},
                {"name": "NEW DELHI", "code": "NDLS", "arrival": "08:32", "actual_arrival": "08:32"}
            ]
        });
        let resp = map_response(&norm).unwrap();
        let stations = resp.stations.unwrap();
        assert!(stations.iter().all(|s| s.status == "departed"));
        assert!(resp
            .current_location_info
            .unwrap()
            .contains("Arrived at NEW DELHI"));
    }

    #[test]
    fn delay_minutes_is_conservative() {
        assert_eq!(delay_minutes("18:05", "18:15"), 10);
        assert_eq!(delay_minutes("18:05", "18:05"), 0);
        assert_eq!(
            delay_minutes("18:05", "17:50"),
            0,
            "early arrival -> on time"
        );
        assert_eq!(
            delay_minutes("23:55", "00:10"),
            0,
            "cross-midnight is ambiguous"
        );
        assert_eq!(delay_minutes("18:05", "**UA**"), 0);
        assert_eq!(delay_minutes("18:05", ""), 0);
    }
}
