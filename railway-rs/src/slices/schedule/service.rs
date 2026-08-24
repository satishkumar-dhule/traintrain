use std::time::Instant;

use serde_json::Value;

use crate::core::corover::{self, SOURCE_API};
use crate::core::error::AppError;
use crate::core::railyatri;
use crate::models::{ScheduleResponse, ScheduleStop};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Resolve the full schedule (route + running days) for a train number.
    ///
    /// Ask DISHA (`trnscheduleEnq` via the CoRover guest API,
    /// `api.disha.corover.ai`) is the primary source - unlike NTES it answers
    /// from non-India IPs too. NTES (`enquiry.indianrail.gov.in`) is the
    /// first fallback and Railyatri's SSR page the final one; the winning
    /// source is reported honestly in `data_source`.
    ///
    /// Every candidate is sanity-checked against the local timetable master
    /// list: when the indexed name implies stations (e.g. "AKOT-AKOLA
    /// PASSENGER" -> AKOT/AK) that a served route never touches, that
    /// payload is treated as stale and the chain moves on; a final fallback
    /// that still conflicts is served with a caution notice.
    pub async fn get_schedule(state: &AppState, train: &str) -> Result<ScheduleResponse, AppError> {
        let key = format!("schedule:{train}");

        // The local timetable master list is the identity anchor. Aggregators
        // sometimes serve years-stale routes under a reused number (e.g.
        // 77608 still answering as MEDCHAL-SECUNDERABAD DEMU after the number
        // moved to the AKOT-AKOLA shuttle), so every candidate route must
        // touch at least one station implied by the indexed train name.
        let expected_stations = index_route_stations(state, train);

        // The final DTO (not the raw upstream payload) is cached, so a hit
        // replays the winning source's full response shape verbatim,
        // including its honest `data_source`.
        if let Some(cached) = state.cache.get(&key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        // Ask DISHA / CoRover primary (works worldwide): a disabled module or
        // any failure degrades to the NTES branch without extra latency cost.
        let corover_failure = match corover_schedule(state, train).await {
            Ok(resp) => {
                let stop_codes: Vec<&str> = resp
                    .stops
                    .iter()
                    .flatten()
                    .map(|s| s.code.as_str())
                    .collect();
                if route_reaches_expected(&expected_stations, &stop_codes) {
                    tracing::info!(
                        %train,
                        source = "CoRover",
                        "schedule resolved from CoRover"
                    );
                    state.cache.set(&key, serde_json::to_value(&resp)?);
                    return Ok(resp);
                }
                tracing::warn!(
                    %train,
                    expected = ?expected_stations,
                    got = ?stop_codes,
                    source = "CoRover",
                    "upstream route conflicts with the timetable index; trying next source"
                );
                route_conflict_message("CoRover", &expected_stations, &stop_codes)
            }
            Err(e) => e.message(),
        };

        let ntes_started = Instant::now();
        let ntes_failure = match state.ntes.schedule(train, "").await {
            Ok(data) => {
                state
                    .metrics
                    .record_source_latency("ntes", ntes_started.elapsed());
                match ntes_schedule_response(train, &data, state.config.cache_ttl.as_secs()) {
                    Ok(resp) => {
                        let stop_codes: Vec<&str> = resp
                            .stops
                            .iter()
                            .flatten()
                            .map(|s| s.code.as_str())
                            .collect();
                        if route_reaches_expected(&expected_stations, &stop_codes) {
                            tracing::info!(
                                %train,
                                source = "NTES",
                                latency_ms = ntes_started.elapsed().as_millis(),
                                %corover_failure,
                                "schedule resolved from NTES after CoRover failure"
                            );
                            state.cache.set(&key, serde_json::to_value(&resp)?);
                            return Ok(resp);
                        }
                        tracing::warn!(
                            %train,
                            expected = ?expected_stations,
                            got = ?stop_codes,
                            source = "NTES",
                            "upstream route conflicts with the timetable index; trying next source"
                        );
                        route_conflict_message("NTES", &expected_stations, &stop_codes)
                    }
                    Err(e) => e.message(),
                }
            }
            Err(e) => e.message(),
        };

        match railyatri_schedule(state, train).await {
            Ok(mut resp) => {
                let stop_codes: Vec<&str> = resp
                    .stops
                    .iter()
                    .flatten()
                    .map(|s| s.code.as_str())
                    .collect();
                if !route_reaches_expected(&expected_stations, &stop_codes) {
                    tracing::warn!(
                        %train,
                        expected = ?expected_stations,
                        got = ?stop_codes,
                        source = "Railyatri",
                        "final fallback route conflicts with the timetable index; serving with a caution notice"
                    );
                    let mut notice = resp.notice.take().unwrap_or_default();
                    if !notice.is_empty() {
                        notice.push(' ');
                    }
                    notice.push_str(&format!(
                        "Caution: this route never reaches {} — upstream sources may be serving stale data for train {train}.",
                        expected_stations.join("/")
                    ));
                    resp.notice = Some(notice);
                }
                tracing::info!(%train, source = "Railyatri", "schedule resolved");
                state.cache.set(&key, serde_json::to_value(&resp)?);
                Ok(resp)
            }
            Err(AppError::NotFound(msg)) => Err(AppError::not_found(msg)),
            Err(ry_err) => Err(AppError::source_unavailable(
                "all-sources",
                format!(
                    "schedule for {train} failed: CoRover: {corover_failure} | NTES: {ntes_failure} | Railyatri: {}",
                    ry_err.message()
                ),
            )),
        }
    }
}

/// Station codes implied by the timetable-index name of `train`
/// ("AKOT-AKOLA PASSENGER" -> ["AKOT", "AK"]). A token counts when it is a
/// known station code or exactly matches a known station name. Service-type
/// words ("DEMU", "SF") are excluded even though they collide with real
/// stations, and trains whose name yields nothing ("TEJAS RAJ") disable the
/// route check entirely.
fn index_route_stations(state: &AppState, train: &str) -> Vec<String> {
    let Some(name) = state.datasets.train_name(train) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for token in plausible_station_tokens(name) {
        if state.datasets.station_name(token).is_some() {
            if !out.iter().any(|c| c == token) {
                out.push(token.to_string());
            }
        } else if let Some(code) = state.datasets.station_code_by_name(token) {
            if !out.iter().any(|c| c.eq_ignore_ascii_case(code)) {
                out.push(code.to_string());
            }
        }
    }
    out
}

/// Tokens of an indexed train name that could plausibly denote a station:
/// alphanumeric words minus service-type words (which sometimes collide with
/// real station names, e.g. DEMU, SF).
fn plausible_station_tokens(name: &str) -> Vec<&str> {
    const NON_STATION_WORDS: &[&str] = &[
        "EXPRESS",
        "SUPERFAST",
        "SPECIAL",
        "PASSENGER",
        "MAIL",
        "DEMU",
        "MEMU",
        "DMU",
        "EMU",
        "SF",
        "SHATABDI",
        "RAJDHANI",
        "DURONTO",
        "TEJAS",
        "GARIB",
        "RATH",
        "RAJ",
        "KRANTI",
        "SAMPARK",
        "VANDE",
        "BHARAT",
        "FAST",
        "EXP",
        "AC",
        "HUMSAFAR",
        "ANTYODAYA",
        "JAN",
        "LINK",
    ];
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| (2..=24).contains(&t.len()))
        .filter(|t| {
            let upper = t.to_ascii_uppercase();
            !NON_STATION_WORDS.contains(&upper.as_str())
        })
        .collect()
}

/// A candidate schedule passes when it touches at least one expected station;
/// trains without derivable expectations always pass.
fn route_reaches_expected(expected: &[String], stop_codes: &[&str]) -> bool {
    expected.is_empty()
        || expected
            .iter()
            .any(|e| stop_codes.iter().any(|s| s.eq_ignore_ascii_case(e)))
}

fn route_conflict_message(source: &str, expected: &[String], stop_codes: &[&str]) -> String {
    format!(
        "{source} route ({}) never reaches timetable-index stations {expected:?}",
        stop_codes.join(">"),
    )
}

/// Normalize a NTES `GetTrainSchedule` response into the wire model.
fn ntes_schedule_response(
    train: &str,
    data: &Value,
    cache_ttl: u64,
) -> Result<ScheduleResponse, AppError> {
    let list = [
        "trainScheduleList",
        "trainStnList",
        "stationList",
        "trainSchedule",
    ]
    .iter()
    .find_map(|k| data.get(*k).and_then(Value::as_array))
    .filter(|a| !a.is_empty())
    .ok_or_else(|| {
        AppError::source_unavailable("NTES", "unexpected GetTrainSchedule response shape")
    })?;

    let stops: Vec<ScheduleStop> = list
        .iter()
        .map(|s| ScheduleStop {
            code: field(s, &["stationCode", "stationcode", "code"]),
            name: field(s, &["stationName", "stationname", "name"]),
            arrival: field(s, &["arrivalTime", "arrivaltime", "arrival"]),
            departure: field(
                s,
                &["departureTime", "departuretime", "departure", "depTime"],
            ),
            day: Some(u8::try_from(int_field(s, &["day", "stopDay"]).unwrap_or(1)).unwrap_or(1)),
            distance_km: None,
        })
        .filter(|s| !s.code.is_empty() || !s.name.is_empty())
        .collect();

    if stops.is_empty() {
        return Err(AppError::source_unavailable("NTES", "no stops in schedule"));
    }

    Ok(ScheduleResponse {
        train_number: Some(non_empty(&data["trainNo"]).unwrap_or_else(|| train.to_string())),
        train_name: non_empty(&data["trainName"]),
        route_description: None,
        running_days: None,
        stops: Some(stops),
        source: Some("NTES".to_string()),
        cache_ttl: Some(cache_ttl),
        notice: Some(
            "Live data from NTES, the official Indian Railways enquiry system (enquiry.indianrail.gov.in)."
                .to_string(),
        ),
        data_source: Some("NTES".to_string()),
    })
}

/// Fetch and normalize the Railyatri SSR schedule page.
async fn railyatri_schedule(state: &AppState, train: &str) -> Result<ScheduleResponse, AppError> {
    let url = state.config.source_url(
        &state.config.railyatri_base,
        &format!("/time-table/{train}"),
    );

    let started = Instant::now();
    let res = state
        .http
        .inner()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::source_unavailable("Railyatri", format!("GET {url}: {e}")))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::not_found(format!("Train {train} not found.")));
    }
    if !status.is_success() {
        return Err(AppError::source_unavailable(
            "Railyatri",
            format!("GET {url} returned {status}"),
        ));
    }
    let html = res.text().await.map_err(|e| {
        AppError::source_unavailable("Railyatri", format!("read body of {url}: {e}"))
    })?;

    let _nd = railyatri::extract_next_data(&html)
        .map_err(|e| AppError::source_unavailable("Railyatri", e.message()))?;
    let norm = railyatri::parse_schedule(&html)
        .map_err(|e| AppError::source_unavailable("Railyatri", e.message()))?;
    state
        .metrics
        .record_source_latency("railyatri", started.elapsed());

    let stop_values = norm["stops"].as_array().cloned().unwrap_or_default();
    if stop_values.is_empty() {
        return Err(AppError::source_unavailable(
            "Railyatri",
            "no stops in timetable",
        ));
    }
    let stops = stop_values
        .iter()
        .map(|s| ScheduleStop {
            code: s["code"].as_str().unwrap_or_default().to_string(),
            name: s["name"].as_str().unwrap_or_default().to_string(),
            arrival: s["arrival"].as_str().unwrap_or_default().to_string(),
            departure: s["departure"].as_str().unwrap_or_default().to_string(),
            day: Some(u8::try_from(s["day"].as_i64().unwrap_or(1)).unwrap_or(1)),
            distance_km: None,
        })
        .collect();

    let running_days = norm["run_days"]
        .as_array()
        .map(|days| {
            days.iter()
                .filter_map(|d| d.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(ScheduleResponse {
        train_number: non_empty(&norm["train_number"]),
        train_name: non_empty(&norm["train_name"]),
        route_description: non_empty(&norm["route_description"]),
        running_days: Some(running_days),
        stops: Some(stops),
        source: Some("Railyatri".to_string()),
        cache_ttl: Some(state.config.cache_ttl.as_secs()),
        notice: Some("Live data extracted from Railyatri.".to_string()),
        data_source: Some("Railyatri".to_string()),
    })
}

fn non_empty(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(String::from)
}

/// Fetch and normalize the Ask DISHA `trnscheduleEnq` schedule (primary
/// source of the chain). No-op when the module is disabled (`state.askdisha`
/// is `None`) - reports a source-unavailable error without any outbound call,
/// so a disabled deployment silently keeps the NTES -> Railyatri behaviour.
async fn corover_schedule(state: &AppState, train: &str) -> Result<ScheduleResponse, AppError> {
    let client = state
        .askdisha
        .as_deref()
        .ok_or_else(|| AppError::source_unavailable(SOURCE_API, "askdisha module disabled"))?;

    let started = Instant::now();
    let raw = client.trnschedule_enq(train, None, None).await?;
    state
        .metrics
        .record_source_latency(SOURCE_API, started.elapsed());

    corover_schedule_response(train, &raw, state.config.cache_ttl.as_secs())
}

/// Normalize the CoRover schedule payload into the same wire model the NTES /
/// Railyatri branches emit. `distance` / `dayCount` map onto
/// [`ScheduleStop::distance_km`] / [`ScheduleStop::day`]; the core parser has
/// already turned `"--"` sentinels and unparseable values into `None`.
fn corover_schedule_response(
    train: &str,
    data: &corover::ScheduleResponse,
    cache_ttl: u64,
) -> Result<ScheduleResponse, AppError> {
    if data.station_list.is_empty() {
        return Err(AppError::source_unavailable(
            SOURCE_API,
            "no stops in schedule",
        ));
    }

    let stops: Vec<ScheduleStop> = data
        .station_list
        .iter()
        .map(|s| ScheduleStop {
            code: s.station_code.clone(),
            name: s.station_name.clone(),
            arrival: s.arrival_time.clone().unwrap_or_default(),
            departure: s.departure_time.clone().unwrap_or_default(),
            day: s.day_count,
            distance_km: s.distance,
        })
        .collect();

    // Run-day "Y"/"N" flags map onto the same ["MON", ...] spelling the
    // Railyatri branch emits, so the primary source keeps wire parity.
    let running_days: Vec<String> = [
        (data.runs_mon, "MON"),
        (data.runs_tue, "TUE"),
        (data.runs_wed, "WED"),
        (data.runs_thu, "THU"),
        (data.runs_fri, "FRI"),
        (data.runs_sat, "SAT"),
        (data.runs_sun, "SUN"),
    ]
    .iter()
    .filter(|(runs, _)| *runs)
    .map(|(_, day)| day.to_string())
    .collect();

    Ok(ScheduleResponse {
        train_number: Some(if data.train_number.is_empty() {
            train.to_string()
        } else {
            data.train_number.clone()
        }),
        train_name: data.train_name.clone().filter(|n| !n.is_empty()),
        route_description: None,
        running_days: Some(running_days),
        stops: Some(stops),
        source: Some("CoRover".to_string()),
        cache_ttl: Some(cache_ttl),
        notice: Some("Live data from Ask DISHA (CoRover).".to_string()),
        data_source: Some("CoRover".to_string()),
    })
}

fn field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn int_field(v: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|k| match v.get(*k) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ntes_normalizer_accepts_documented_shape() {
        let data = json!({
            "trainNo": "12951",
            "trainName": "MUMBAI RAJDHANI",
            "trainScheduleList": [
                {"stationCode": "MMCT", "stationName": "MUMBAI CENTRAL", "arrivalTime": "--", "departureTime": "17:40", "day": 1},
                {"stationCode": "NDLS", "stationName": "NEW DELHI", "arrivalTime": "08:32", "departureTime": "--", "day": 2}
            ]
        });
        let resp = ntes_schedule_response("12951", &data, 120).unwrap();
        assert_eq!(resp.data_source.as_deref(), Some("NTES"));
        assert_eq!(resp.train_number.as_deref(), Some("12951"));
        let stops = resp.stops.unwrap();
        assert_eq!(stops.len(), 2);
        assert_eq!(stops[0].code, "MMCT");
        assert_eq!(stops[0].departure, "17:40");
        // NTES always knows the day-of-run; distance is never carried.
        assert_eq!(stops[1].day, Some(2));
        assert_eq!(stops[1].distance_km, None);
    }

    #[test]
    fn ntes_normalizer_rejects_empty_schedule() {
        let data = json!({ "trainNo": "12951", "trainScheduleList": [] });
        let err = ntes_schedule_response("12951", &data, 120).unwrap_err();
        assert!(matches!(err, AppError::SourceUnavailable { source, .. } if source == "NTES"));
    }

    #[test]
    fn corover_normalizer_maps_distance_and_daycount_onto_wire_model() {
        let raw = std::fs::read_to_string("testdata/askdisha/schedule_12951.json")
            .expect("fixture testdata/askdisha/schedule_12951.json must exist");
        let parsed: corover::ScheduleResponse =
            serde_json::from_str(&raw).expect("corover schedule fixture parses");

        let resp = corover_schedule_response("12951", &parsed, 120).expect("normalizes");
        assert_eq!(resp.data_source.as_deref(), Some("CoRover"));
        assert_eq!(resp.source.as_deref(), Some("CoRover"));
        assert_eq!(resp.train_number.as_deref(), Some("12951"));
        assert_eq!(resp.cache_ttl, Some(120));
        // The fixture train runs daily; flags map onto the Railyatri spelling.
        assert_eq!(
            resp.running_days,
            Some(
                vec!["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            )
        );

        let stops = resp.stops.expect("stops present");
        assert_eq!(stops.len(), 8);
        // Origin: distance 0 km, day 1.
        assert_eq!(stops[0].code, "MMCT");
        assert_eq!(stops[0].distance_km, Some(0.0));
        assert_eq!(stops[0].day, Some(1));
        assert_eq!(stops[0].departure, "17:00");
        // Cumulative km + day rollover survive the mapping.
        assert_eq!(stops[4].code, "RTM");
        assert_eq!(stops[4].distance_km, Some(653.0));
        assert_eq!(stops[4].day, Some(2));
        assert_eq!(stops.last().unwrap().distance_km, Some(1384.0));
    }

    #[test]
    fn corover_normalizer_defaults_train_number_and_rejects_empty_stops() {
        let bare = corover::ScheduleResponse {
            train_number: String::new(),
            train_name: None,
            station_from: None,
            station_to: None,
            runs_mon: false,
            runs_tue: false,
            runs_wed: false,
            runs_thu: false,
            runs_fri: false,
            runs_sat: false,
            runs_sun: false,
            error_message: None,
            station_list: Vec::new(),
        };
        let err = corover_schedule_response("12951", &bare, 120).unwrap_err();
        assert!(
            matches!(err, AppError::SourceUnavailable { source, .. } if source == corover::SOURCE_API)
        );

        let mut with_stop = bare.clone();
        with_stop.train_number = "12951".to_string();
        with_stop.station_list.push(corover::ScheduleStop {
            station_code: "MMCT".to_string(),
            station_name: "MUMBAI CENTRAL".to_string(),
            arrival_time: Some("--".to_string()),
            departure_time: None,
            route_number: None,
            halt_time: None,
            distance: None,
            day_count: None,
        });
        let resp = corover_schedule_response("99999", &with_stop, 120).unwrap();
        assert_eq!(resp.train_number.as_deref(), Some("12951"));
        let stops = resp.stops.unwrap();
        // "--" / absent times degrade to the same empty-string shape NTES
        // emits; enrichment stays None.
        assert_eq!(stops[0].arrival, "--");
        assert_eq!(stops[0].departure, "");
        assert_eq!(stops[0].distance_km, None);
        assert_eq!(stops[0].day, None);
    }

    #[test]
    fn station_tokens_skip_service_words_and_keep_places() {
        assert_eq!(
            plausible_station_tokens("AKOT-AKOLA PASSENGER"),
            vec!["AKOT", "AKOLA"]
        );
        // The 77608 stale payload's name: no usable anchor beyond the places.
        assert_eq!(
            plausible_station_tokens("MEDCHAL - SECUNDERABAD DEMU"),
            vec!["MEDCHAL", "SECUNDERABAD"]
        );
        // Service words that collide with real stations (DEMU/SF) are dropped.
        assert_eq!(
            plausible_station_tokens("AKOLA KHANDWA DEMU"),
            vec!["AKOLA", "KHANDWA"]
        );
        assert_eq!(plausible_station_tokens("NDLS TEJAS RAJ"), vec!["NDLS"]);
        assert_eq!(plausible_station_tokens("GARIB RATH"), Vec::<&str>::new());
    }

    #[test]
    fn route_check_passes_when_any_expected_station_is_touched() {
        let expected = vec!["AKOT".to_string(), "AK".to_string()];
        let medchal_route = ["MED", "SC"];
        assert!(!route_reaches_expected(&expected, &medchal_route));
        let akot_route = ["AKOT", "PAR", "AK"];
        assert!(route_reaches_expected(&expected, &akot_route));
        // Case-insensitive match against upstream spellings.
        assert!(route_reaches_expected(&expected, &["akot"]));
        // No derivable expectation -> always pass.
        assert!(route_reaches_expected(&[], &["MED"]));
    }
}
