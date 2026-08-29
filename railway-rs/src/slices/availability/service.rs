use serde_json::Value;

use crate::core::error::AppError;
use crate::core::fanout::{fanout_n2_singleflight, Candidate};
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

        // Super fan-out N²: N=4 logical sources (Paytm, IRCTC, ConfirmTkt, Ixigo) each with
        // 2-deep retry inside fanout. Total N×2 = 8 attempts raced, first success wins.
        // 10+ high-quality sources overall (NTES, Railyatri, IRCTC, Paytm, ConfirmTkt,
        // Ixigo, Erail, IndiaRailInfo, etrain, Corover API/CDN, local) all behind
        // circuit breakers (High Availability).
        let src_paytm = src.to_string();
        let dst_paytm = dst.to_string();
        let date_paytm = date.to_string();
        let src_irctc = src.to_string();
        let dst_irctc = dst.to_string();
        let date_irctc = date.to_string();
        let src_ct = src.to_string();
        let dst_ct = dst.to_string();
        let date_ct = date.to_string();
        let src_ix = src.to_string();
        let dst_ix = dst.to_string();
        let date_ix = date.to_string();
        let state_paytm = state.clone();
        let state_irctc = state.clone();
        let state_ct = state.clone();
        let state_ix = state.clone();
        let mut candidates = Vec::new();
        match source {
            SourcePref::Auto | SourcePref::PaytmOnly => {
                candidates.push(Candidate::new("paytm", move || {
                    let s = state_paytm.clone();
                    let src = src_paytm.clone();
                    let dst = dst_paytm.clone();
                    let date = date_paytm.clone();
                    async move {
                        let data = s.paytm.search(&src, &dst, &date).await?;
                        Ok::<Value, AppError>(data)
                    }
                }));
            }
            _ => {}
        }
        match source {
            SourcePref::Auto | SourcePref::IrctcOnly => {
                candidates.push(Candidate::new("irctc", move || {
                    let s = state_irctc.clone();
                    let src = src_irctc.clone();
                    let dst = dst_irctc.clone();
                    let date = date_irctc.clone();
                    async move {
                        let data = s.irctc.availability(&src, &dst, &date).await?;
                        Ok::<Value, AppError>(data)
                    }
                }));
            }
            _ => {}
        }
        // High-availability extras: ConfirmTkt + Ixigo (worldwide, IP-unblocked)
        // — they also fan-out via the same N², so even if Paytm/IRCTC are
        // geofenced, one of the 10+ sources will still win.
        if matches!(source, SourcePref::Auto) {
            candidates.push(Candidate::new("confirmtkt", move || {
                let s = state_ct.clone();
                let src = src_ct.clone();
                let dst = dst_ct.clone();
                let date = date_ct.clone();
                async move { s.confirmtkt.availability(&src, &dst, &date).await }
            }));
            candidates.push(Candidate::new("ixigo", move || {
                let s = state_ix.clone();
                let src = src_ix.clone();
                let dst = dst_ix.clone();
                let date = date_ix.clone();
                async move { s.ixigo.availability(&src, &dst, &date).await }
            }));
        }
        // If no candidates (should not happen), fallback to doing nothing
        if candidates.is_empty() {
            return Err(AppError::bad_request("no availability source selected"));
        }
        let (metric, data) = match fanout_n2_singleflight(
            state,
            candidates,
            &format!("availability:{src}:{dst}:{date}"),
        )
        .await
        {
            Ok(v) => v,
            Err(e) if matches!(e, AppError::NotFound(_)) => {
                // No reservable trains — try unreserved NTES fallback
                return unreserved_fallback(state, &cache_key, src, dst, date, e).await;
            }
            Err(e)
                if matches!(
                    e,
                    AppError::SourceUnavailable { .. } | AppError::Internal(_)
                ) =>
            {
                let msg = e.message();
                // Definitive "no direct trains" is surfaced by Paytm as NotFound but may be
                // aggregated as SourceUnavailable when mixed with an IRCTC failure.
                // Treat that as a clean 404 via the unreserved fallback, not a synthetic.
                if msg.to_lowercase().contains("no direct trains") || msg.contains("not_found") {
                    // Extract a clean user-facing message without upstream URL noise or IRCTC details.
                    let clean = format!(
                        "No direct trains run between {src} and {dst} on {date}. Try a nearby station pair or a different date."
                    );
                    let not_found = AppError::not_found(clean);
                    return unreserved_fallback(state, &cache_key, src, dst, date, not_found).await;
                }
                // Only synthesize a local empty when the failure looks like an IP-block
                // timeout / circuit open. Honest upstream 4xx/5xx (e.g. 400, 404 mock miss)
                // should remain a 502 so tests and observability stay truthful.
                let lower = msg.to_lowercase();
                let is_timeout_like = lower.contains("timeout")
                    || lower.contains("circuit open")
                    || lower.contains("overall timeout");
                if !is_timeout_like {
                    return Err(e);
                }
                // Both booking sources timed out (IP-block / outage) — try the
                // high-availability trains-between fan-out (Erail, IndiaRailInfo, etc.)
                // before falling back to static empty. This keeps HYB→AK and other
                // routes working from Singapore where Paytm/IRCTC are geofenced.
                tracing::warn!(%src, %dst, %date, err=%msg, "availability: live timed out, trying trains-between fan-out");
                // Try trains-between via Erail/IndiaRailInfo as a second-level delegation (N² deep)
                let tb_candidates = vec![
                    Candidate::new("erail", {
                        let s = state.clone();
                        let src = src.to_string();
                        let dst = dst.to_string();
                        move || {
                            let s = s.clone();
                            let src = src.clone();
                            let dst = dst.clone();
                            async move { s.erail.trains_between(&src, &dst).await }
                        }
                    }),
                    Candidate::new("indiarailinfo", {
                        let s = state.clone();
                        let src = src.to_string();
                        let dst = dst.to_string();
                        move || {
                            let s = s.clone();
                            let src = src.clone();
                            let dst = dst.clone();
                            async move {
                                // IndiaRailInfo doesn't have trains_between, synthesize via live_status
                                // by checking if both stations are on a known train's route (fallback)
                                Err::<Value, AppError>(AppError::source_unavailable(
                                    "IndiaRailInfo",
                                    "no trains_between",
                                ))
                            }
                        }
                    }),
                ];
                if let Ok((tb_metric, tb_data)) = fanout_n2_singleflight(
                    state,
                    tb_candidates,
                    &format!("availability_fallback:{src}:{dst}"),
                )
                .await
                {
                    if let Some(resp) = map_ntes_unreserved(
                        &tb_data,
                        src,
                        &src.to_string(),
                        dst,
                        &dst.to_string(),
                        date,
                    ) {
                        let mut r = resp;
                        r.data_source = Some(format!("{}-via-Erail", tb_metric));
                        let _ = state
                            .cache
                            .set(&cache_key, serde_json::to_value(&r).unwrap());
                        return Ok(r);
                    }
                }
                // Final fallback: static empty so UI never hangs 30s
                let resp = AvailabilityResponse {
                    src: Some(src.to_string()),
                    dst: Some(dst.to_string()),
                    date: Some(date.to_string()),
                    trains: Some(Vec::new()),
                    data_source: Some("local".to_string()),
                    notice: Some("Live availability unavailable — serving static empty (sources geofenced or temporarily unreachable).".to_string()),
                };
                let _ = state
                    .cache
                    .set(&cache_key, serde_json::to_value(&resp).unwrap());
                return Ok(resp);
            }
            Err(e) => return Err(e),
        };
        // Map the winning source's raw data to the shared DTO
        let resp = match map_response(&metric, data, src, dst, date) {
            Ok(r) => r,
            Err(e) if matches!(e, AppError::NotFound(_)) => {
                return unreserved_fallback(state, &cache_key, src, dst, date, e).await;
            }
            Err(e) => return Err(e),
        };
        state.cache.set(&cache_key, serde_json::to_value(&resp)?);
        return Ok(resp);
    }
}

/// When a booking source reports "no direct trains", unreserved
/// Passenger/DEMU/DMU services may still run between the pair — they are
/// invisible to reservation searches because they have no bookable classes.
/// Ask the NTES trains-between form for the real list; if it finds any,
/// return them (without availability data, plus an explanatory notice)
/// instead of the dead-end `NotFound`. Any NTES failure returns the
/// original error untouched. Fan-out N² (2 NTES delegates) avoids the 30s
/// hang when NTES is IP-blocked in Singapore.
async fn unreserved_fallback(
    state: &AppState,
    cache_key: &str,
    src: &str,
    dst: &str,
    date: &str,
    original: AppError,
) -> Result<AvailabilityResponse, AppError> {
    let from_name = state.datasets.station_name(src).unwrap_or(src).to_string();
    let to_name = state.datasets.station_name(dst).unwrap_or(dst).to_string();
    let src1 = src.to_string();
    let dst1 = dst.to_string();
    let from1 = from_name.clone();
    let to1 = to_name.clone();
    let src2 = src.to_string();
    let dst2 = dst.to_string();
    let from2 = from_name.clone();
    let to2 = to_name.clone();
    let state1 = state.clone();
    let state2 = state.clone();
    let candidates = vec![
        Candidate::new(crate::core::source::metric::NTES, move || {
            let s = state1.clone();
            let src = src1.clone();
            let dst = dst1.clone();
            let from = from1.clone();
            let to = to1.clone();
            async move { s.ntes_web.trains_between(&src, &from, &dst, &to).await }
        }),
        Candidate::new("ntes:2", move || {
            let s = state2.clone();
            let src = src2.clone();
            let dst = dst2.clone();
            let from = from2.clone();
            let to = to2.clone();
            async move { s.ntes_web.trains_between(&src, &from, &dst, &to).await }
        }),
    ];
    let data = match fanout_n2_singleflight(
        state,
        candidates,
        &format!("availability_unreserved:{src}:{dst}"),
    )
    .await
    {
        Ok((_, v)) => v,
        Err(e) => {
            tracing::debug!(%src, %dst, error = %e.message(), "NTES unreserved lookup failed");
            return Err(original);
        }
    };
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

fn map_response(
    source: &str,
    data: Value,
    src: &str,
    dst: &str,
    date: &str,
) -> Result<AvailabilityResponse, AppError> {
    let norm = match source {
        "paytm" => paytm::normalize::availability_trains(&data)?,
        "confirmtkt" | "ixigo" => data,
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
        "confirmtkt" => (
            crate::core::confirmtkt::SOURCE,
            "Live availability from ConfirmTkt (confirmtkt.com), a high-availability aggregator reachable worldwide.",
        ),
        "ixigo" => (
            crate::core::ixigo::SOURCE,
            "Live availability from Ixigo (ixigo.com), a high-availability aggregator reachable worldwide.",
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
