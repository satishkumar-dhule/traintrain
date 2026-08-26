use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::{irctc, paytm};
use crate::models::{AvailabilityClass, AvailabilityResponse, AvailabilityTrain};
use crate::slices::availability::SourcePref;
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Direct trains with availability between `src` and `dst` on `date`
    /// (`YYYY-MM-DD`). `source` picks the upstream: Paytm (default primary,
    /// per-class status + fares), IRCTC (fallback / explicit), or auto
    /// (Paytm first, IRCTC on failure). `data_source` names whoever answered.
    pub async fn get_availability(
        state: &AppState,
        src: &str,
        dst: &str,
        date: &str,
        source: SourcePref,
    ) -> Result<AvailabilityResponse, AppError> {
        let cache_key = format!(
            "availability:{src}:{dst}:{date}:{}",
            match source {
                SourcePref::Auto => "auto",
                SourcePref::PaytmOnly => "paytm",
                SourcePref::IrctcOnly => "irctc",
            }
        );
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        let (primary, fallback) = match source {
            SourcePref::Auto => ("paytm", Some("irctc")),
            SourcePref::PaytmOnly => ("paytm", None),
            SourcePref::IrctcOnly => ("irctc", None),
        };

        let primary_err = match fetch_from(state, primary, src, dst, date).await {
            Ok(resp) => {
                state.cache.set(&cache_key, serde_json::to_value(&resp)?);
                return Ok(resp);
            }
            Err(e) => e,
        };

        // A definitive "no reservable trains" answer from a booking platform
        // still leaves room for unreserved Passenger/DEMU services, which
        // never appear in reservation searches (no bookable classes). Ask
        // NTES before giving up.
        if matches!(primary_err, AppError::NotFound(_)) {
            return unreserved_fallback(state, &cache_key, src, dst, date, primary_err).await;
        }

        if let Some(fallback) = fallback {
            tracing::warn!(
                %src,
                %dst,
                source = primary,
                error = %primary_err,
                "primary availability source failed; trying fallback"
            );
            match fetch_from(state, fallback, src, dst, date).await {
                Ok(resp) => {
                    state.cache.set(&cache_key, serde_json::to_value(&resp)?);
                    return Ok(resp);
                }
                Err(e) => {
                    return Err(AppError::source_unavailable(
                        format!("{primary} + {fallback}"),
                        format!("{}; {}", primary_err.message(), e.message()),
                    ));
                }
            }
        }

        Err(primary_err)
    }
}

/// When a booking source reports "no direct trains", unreserved
/// Passenger/DEMU/DMU services may still run between the pair — they are
/// invisible to reservation searches because they have no bookable classes.
/// Ask the NTES trains-between form for the real list; if it finds any,
/// return them (without availability data, plus an explanatory notice)
/// instead of the dead-end `NotFound`. Any NTES failure returns the
/// original error untouched.
async fn unreserved_fallback(
    state: &AppState,
    cache_key: &str,
    src: &str,
    dst: &str,
    date: &str,
    original: AppError,
) -> Result<AvailabilityResponse, AppError> {
    if state
        .failover
        .should_skip(crate::core::source::metric::NTES)
    {
        tracing::debug!(%src, %dst, "NTES unreserved lookup skipped — circuit open");
        return Err(original);
    }
    let started = Instant::now();
    let from_name = state.datasets.station_name(src).unwrap_or(src).to_string();
    let to_name = state.datasets.station_name(dst).unwrap_or(dst).to_string();
    match state
        .ntes_web
        .trains_between(src, &from_name, dst, &to_name)
        .await
    {
        Ok(data) => {
            state
                .metrics
                .record_source_latency(crate::core::source::metric::NTES, started.elapsed());
            state
                .failover
                .record_success(crate::core::source::metric::NTES);
            match map_ntes_unreserved(&data, src, &from_name, dst, &to_name, date) {
                Some(resp) => {
                    tracing::info!(
                        %src,
                        %dst,
                        "booking sources report no reservable trains; NTES supplied unreserved services"
                    );
                    state.cache.set(cache_key, serde_json::to_value(&resp)?);
                    Ok(resp)
                }
                None => Err(original),
            }
        }
        Err(e) => {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state
                    .failover
                    .record_failure(crate::core::source::metric::NTES);
            }
            tracing::debug!(%src, %dst, error = %e.message(), "NTES unreserved lookup failed");
            Err(original)
        }
    }
}

/// Map the raw NTES trains-between payload to an availability response with
/// empty classes/availability. `None` when no trains are listed.
fn map_ntes_unreserved(
    data: &Value,
    src: &str,
    from_name: &str,
    dst: &str,
    to_name: &str,
    date: &str,
) -> Option<AvailabilityResponse> {
    let list = data
        .get("trainBtwStationList")
        .or_else(|| data.get("trainList"))
        .and_then(Value::as_array)?;
    if list.is_empty() {
        return None;
    }

    let trains: Vec<AvailabilityTrain> = list
        .iter()
        .map(|entry| map_unreserved_train(entry, src, from_name, dst, to_name))
        .collect();
    Some(AvailabilityResponse {
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        date: Some(date.to_string()),
        trains: Some(trains),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
        notice: Some(format!(
            "No reserved-class trains run between {src} and {dst}; these unreserved Passenger/DEMU/DMU services operate on this route. They have no IRCTC reservation chart or bookable classes, so class-wise availability does not apply — general tickets are sold at station counters and via UTS."
        )),
    })
}

fn map_unreserved_train(
    entry: &Value,
    src: &str,
    from_name: &str,
    dst: &str,
    to_name: &str,
) -> AvailabilityTrain {
    let name = str_field(entry, "trainName");
    AvailabilityTrain {
        number: str_field(entry, "trainNo"),
        train_type: unreserved_kind(&name).to_string(),
        name,
        from_code: src.to_string(),
        from_name: from_name.to_string(),
        to_code: dst.to_string(),
        to_name: to_name.to_string(),
        departure_time: str_field(entry, "depTime"),
        arrival_time: str_field(entry, "arrTime"),
        duration: String::new(),
        distance: String::new(),
        classes: Vec::new(),
        runs_on: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .iter()
            .map(|d| day_bool(entry, d))
            .collect(),
        availability: Vec::new(),
    }
}

/// Best-effort service kind from the NTES train name; empty when unknown.
fn unreserved_kind(name: &str) -> &str {
    let upper = name.to_ascii_uppercase();
    for (token, kind) in [
        ("DEMU", "DEMU"),
        ("MEMU", "MEMU"),
        ("DMU", "DMU"),
        ("PASSENGER", "Passenger"),
    ] {
        if upper.contains(token) {
            return kind;
        }
    }
    ""
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

/// Fetch from one named source (`"paytm"` / `"irctc"`) and normalize to the
/// shared DTO. Latency is recorded under the source's lowercase key so the
/// observability source-status panel sees it. Flip-flop: skipped when the
/// circuit is open so requests do not pay the upstream timeout.
async fn fetch_from(
    state: &AppState,
    source: &str,
    src: &str,
    dst: &str,
    date: &str,
) -> Result<AvailabilityResponse, AppError> {
    if state.failover.should_skip(source) {
        return Err(AppError::source_unavailable(
            source.to_string(),
            "circuit open (cooldown)",
        ));
    }
    let start = Instant::now();
    let data = match source {
        "paytm" => state.paytm.search(src, dst, date).await.map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state.failover.record_failure(source);
            }
            e
        })?,
        "irctc" => state
            .irctc
            .availability(src, dst, date)
            .await
            .map_err(|e| {
                if matches!(
                    e,
                    AppError::SourceUnavailable { .. } | AppError::Internal(_)
                ) {
                    state.failover.record_failure(source);
                }
                e
            })?,
        other => return Err(AppError::internal(format!("unknown source {other}"))),
    };
    state.metrics.record_source_latency(source, start.elapsed());
    state.failover.record_success(source);
    map_response(source, data, src, dst, date)
}

fn map_response(
    source: &str,
    data: Value,
    src: &str,
    dst: &str,
    date: &str,
) -> Result<AvailabilityResponse, AppError> {
    let norm = match source {
        "paytm" => paytm::normalize::availability_trains(&data)?,
        _ => irctc::normalize::availability_trains(&data)?,
    };
    let trains: Vec<AvailabilityTrain> = norm["trains"]
        .as_array()
        .map(|list| list.iter().map(map_train).collect())
        .unwrap_or_default();

    if trains.is_empty() {
        return Err(AppError::source_unavailable(
            source_label(source),
            "no trains with availability in response",
        ));
    }

    let (label, notice) = match source {
        "paytm" => (
            paytm::client::SOURCE,
            "Live availability from Paytm Travel (travel.paytm.com), reflecting IRCTC booking status with class-wise waitlist and fare details.",
        ),
        _ => (
            irctc::client::SOURCE,
            "Live availability from IRCTC (www.irctc.co.in), the official Indian Railways booking portal. IRCTC is IP-geofenced to India.",
        ),
    };

    Ok(AvailabilityResponse {
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        date: Some(date.to_string()),
        trains: Some(trains),
        data_source: Some(label.to_string()),
        notice: Some(notice.to_string()),
    })
}

fn map_train(t: &Value) -> AvailabilityTrain {
    AvailabilityTrain {
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
        availability: t["availability"]
            .as_array()
            .map(|a| a.iter().map(map_class).collect())
            .unwrap_or_default(),
    }
}

fn map_class(a: &Value) -> AvailabilityClass {
    AvailabilityClass {
        class: str_field(a, "class"),
        class_name: str_field(a, "class_name"),
        status: str_field(a, "status"),
        available: a.get("available").and_then(Value::as_bool),
        fare: a.get("fare").and_then(Value::as_i64),
        quota: a.get("quota").and_then(Value::as_str).map(String::from),
        prediction: a.get("prediction").and_then(Value::as_i64),
    }
}

fn source_label(source: &str) -> String {
    match source {
        "paytm" => paytm::client::SOURCE.to_string(),
        _ => irctc::client::SOURCE.to_string(),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// NTES mobile-shape payload as parsed from the TrainsBetweenStation HTML
    /// (AKOT→AK reality: three daily unreserved shuttles).
    fn ntes_akot_ak() -> Value {
        json!({
            "trainBtwStationList": [
                {"trainNo": "77608", "trainName": "AKOT-AKOLA PASSENGER",
                 "depTime": "09:00", "arrTime": "10:10",
                 "runOnMon": true, "runOnTue": true, "runOnWed": true,
                 "runOnThu": true, "runOnFri": true, "runOnSat": true, "runOnSun": true},
                {"trainNo": "77610", "trainName": "AKOT-AK DMU",
                 "depTime": "14:30", "arrTime": "15:40",
                 "runsOnMon": true, "runsOnTue": true, "runsOnWed": true,
                 "runsOnThu": true, "runsOnFri": true, "runsOnSat": true, "runsOnSun": false}
            ]
        })
    }

    #[test]
    fn unreserved_mapping_lists_trains_without_classes() {
        let resp = map_ntes_unreserved(
            &ntes_akot_ak(),
            "AKOT",
            "AKOT",
            "AK",
            "AKOLA JN",
            "2026-08-24",
        )
        .expect("trains expected");

        assert_eq!(resp.src.as_deref(), Some("AKOT"));
        assert_eq!(resp.dst.as_deref(), Some("AK"));
        assert_eq!(resp.date.as_deref(), Some("2026-08-24"));
        assert_eq!(resp.data_source.as_deref(), Some("NTES"));

        let trains = resp.trains.unwrap();
        assert_eq!(trains.len(), 2);
        let first = &trains[0];
        assert_eq!(first.number, "77608");
        assert_eq!(first.name, "AKOT-AKOLA PASSENGER");
        assert_eq!(first.train_type, "Passenger");
        assert_eq!(first.departure_time, "09:00");
        assert_eq!(first.arrival_time, "10:10");
        assert_eq!(first.from_code, "AKOT");
        assert_eq!(first.to_code, "AK");
        // Unreserved: no bookable classes, no per-class availability.
        assert!(first.classes.is_empty());
        assert!(first.availability.is_empty());
        // Runs all seven days (documented `runOn<Day>` spelling).
        assert_eq!(first.runs_on, vec![true; 7]);
        // Second train uses the community `runsOn<Day>` spelling.
        assert_eq!(
            trains[1].runs_on,
            vec![true, true, true, true, true, true, false]
        );

        let notice = resp.notice.unwrap();
        assert!(notice.contains("unreserved"), "{notice}");
    }

    #[test]
    fn unreserved_mapping_returns_none_for_empty_list() {
        let empty = json!({"trainBtwStationList": []});
        assert!(
            map_ntes_unreserved(&empty, "AKOT", "AKOT", "AK", "AKOLA JN", "2026-08-24").is_none()
        );
        assert!(
            map_ntes_unreserved(&json!({}), "AKOT", "AKOT", "AK", "AKOLA JN", "2026-08-24")
                .is_none()
        );
    }

    #[test]
    fn unreserved_kind_detects_service_type_from_name() {
        assert_eq!(unreserved_kind("AKOT-AKOLA PASSENGER"), "Passenger");
        assert_eq!(unreserved_kind("Akot - Akola DEMU"), "DEMU");
        assert_eq!(unreserved_kind("SECUNDERABAD-MEDCHAL MEMU"), "MEMU");
        assert_eq!(unreserved_kind("AKOLA-KHANDWA DMU"), "DMU");
        assert_eq!(unreserved_kind("57561 EXPRESS"), "");
    }
}
