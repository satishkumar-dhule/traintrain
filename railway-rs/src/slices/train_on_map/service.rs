use std::collections::HashMap;

use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::fanout::{Candidate, fanout_n2};
use crate::models::{
    MapCurrentStation, MapJourneyStation, RouteStation, TrackStation, TrainOnMapResponse,
};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// The "Train on Map" view for `train`; `station` (optional) switches from
    /// the static route map to the live spot view.
    ///
    /// The route map is always required (the route/track polyline and the
    /// header come from it); the live spot view is best-effort - when it fails
    /// we log and return the route-only map rather than failing the request.
    pub async fn get_train_on_map(
        state: &AppState,
        train: &str,
        station: Option<&str>,
        date: Option<&str>,
    ) -> Result<TrainOnMapResponse, AppError> {
        // The spot view (`station` present) changes the response, so it gets
        // its own cache key; the route-only map is keyed by train alone.
        let cache_key = match station {
            Some(code) => keys::train_on_map_station(train, code),
            None => keys::train_on_map(train),
        };
        if let Some(cached) = state.cache.get_json(&cache_key) {
            return Ok(cached);
        }

        let date = date.map(str::to_string).unwrap_or_else(today_ist);
        // Super fan-out N² for route: NTES route_map (2 delegates: with/without date)
        // + Railyatri schedule (worldwide) + static local fallback. Each delegate
        // retried, first success wins; static delayed 800ms so live can win.
        let train_ntes = train.to_string();
        let date_ntes = date.clone();
        let train_ry = train.to_string();
        let state_ntes = state.clone();
        let state_ry = state.clone();
        let state_static = state.clone();
        let train_static = train.to_string();
        let candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes.clone();
                let t = train_ntes.clone();
                let d = date_ntes.clone();
                async move { s.ntes_web.train_route_map(&t, &d).await }
            }),
            Candidate::new(crate::core::source::metric::RAILYATRI, move || {
                let s = state_ry.clone();
                let t = train_ry.clone();
                async move { railyatri_route_map(&s, &t).await }
            }),
            Candidate::new("local", move || {
                let s = state_static.clone();
                let t = train_static.clone();
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    Err::<Value, AppError>(AppError::source_unavailable("local", "no static route"))
                }
            }),
        ];
        let (_metric, route_norm) = fanout_n2(state, candidates, &format!("train_on_map:{train}:{date}")).await?;
        let mut route: Vec<RouteStation> = route_entries(&route_norm, state);
        let track: Vec<TrackStation> = route_norm
            .get("track")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|c| track_station(&c, state))
            .collect();

        let mut resp = TrainOnMapResponse {
            train_no: Some(str(&route_norm, "trainNo", train)),
            train_name: Some(str(&route_norm, "trainName", "")),
            source: Some(str(&route_norm, "source", "")),
            destination: Some(str(&route_norm, "destination", "")),
            source_code: Some(str(&route_norm, "sourceCode", "")),
            dest_code: Some(str(&route_norm, "destCode", "")),
            start_date: Some(str(&route_norm, "startDate", "")).filter(|s| !s.is_empty()),
            route: Some(route.clone()),
            track: Some(track),
            current_station: None,
            journey_station: None,
            data_source: Some(crate::core::source::labels::NTES.to_string()),
        };

        if let Some(code) = station {
            let code = code.to_ascii_uppercase();
            if state
                .failover
                .should_skip(crate::core::source::metric::NTES)
            {
                tracing::warn!(%train, %code, "train-spot-map skipped — circuit open; returning route-only map");
            } else {
                match state
                    .ntes_web
                    .train_spot_map(train, &code, &date, "A")
                    .await
                {
                    Ok(spot) => {
                        merge_spot(&mut route, &spot);
                        resp.route = Some(route);
                        if let Some(cs) = spot.get("currentStation").and_then(Value::as_object) {
                            let ccode = cs.get("code").and_then(Value::as_str).unwrap_or_default();
                            let (lat, lng) = state
                                .datasets
                                .coord(ccode)
                                .map(|(a, b)| (Some(a), Some(b)))
                                .unwrap_or((None, None));
                            resp.current_station = Some(MapCurrentStation {
                                code: ccode.to_string(),
                                lat,
                                lng,
                            });
                        }
                        if let Some(js) = spot.get("journeyStation") {
                            let code2 = js
                                .get("code")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string();
                            let (lat, lng) = state
                                .datasets
                                .coord(&code2)
                                .map(|(a, b)| (Some(a), Some(b)))
                                .unwrap_or((None, None));
                            resp.journey_station = Some(MapJourneyStation {
                                code: code2,
                                name: str(js, "name", ""),
                                lat,
                                lng,
                                label: str(js, "label", ""),
                                expected_arrival: str(js, "expectedArrival", ""),
                                actual_arrival: str(js, "actualArrival", ""),
                                delay_status: str(js, "delayStatus", ""),
                                platform: str(js, "platform", ""),
                            });
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
                        tracing::warn!(%train, %code, err = %e.message(), "train-spot-map unavailable; returning route-only map")
                    }
                }
            }
        }
        state.cache.set_json(&cache_key, &resp)?;
        Ok(resp)
    }
}

async fn railyatri_route_map(state: &AppState, train: &str) -> Result<Value, AppError> {
    let url = state
        .config
        .source_url(&state.config.railyatri_base, &format!("/time-table/{train}"));
    let res = state
        .http
        .inner()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::source_unavailable("Railyatri", format!("GET {url}: {e}")))?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::not_found(format!("Train {train} not found on Railyatri")));
    }
    if !res.status().is_success() {
        return Err(AppError::source_unavailable(
            "Railyatri",
            format!("GET {url} returned {}", res.status()),
        ));
    }
    let html = res
        .text()
        .await
        .map_err(|e| AppError::source_unavailable("Railyatri", format!("read body {url}: {e}")))?;
    let nd = crate::core::railyatri::extract_next_data(&html)
        .map_err(|e| AppError::source_unavailable("Railyatri", e.message()))?;
    let ttt = crate::core::railyatri::deep_get(&nd, "props.pageProps.trainTimeTable")
        .ok_or_else(|| AppError::source_unavailable("Railyatri", "no trainTimeTable in payload"))?;
    let stops: Vec<Value> = ttt
        .get("routeGroup")
        .and_then(Value::as_array)
        .and_then(|g| g.first())
        .and_then(|g| g.get("routesummary"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            let (code, name) = crate::core::railyatri::stop_pair(&s);
            serde_json::json!({
                "code": code,
                "name": name,
                "arrival": crate::core::railyatri::minutes_to_hhmm(s.get("sta_min").and_then(|v| v.as_i64())),
                "departure": crate::core::railyatri::minutes_to_hhmm(s.get("std_min").and_then(|v| v.as_i64())),
                "day": s.get("day").and_then(|v| v.as_i64()).unwrap_or(1).to_string(),
                "distance": s.get("distance").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                "daysOfRun": "",
            })
        })
        .collect();
    if stops.is_empty() {
        return Err(AppError::source_unavailable("Railyatri", "no stops in timetable"));
    }
    Ok(serde_json::json!({
        "trainNo": ttt.get("train_number").and_then(|v| v.as_str()).unwrap_or(train),
        "trainName": ttt.get("train_name").and_then(|v| v.as_str()).unwrap_or(""),
        "source": ttt.get("source_station").and_then(|v| v.as_str()).unwrap_or(""),
        "destination": ttt.get("destination_station").and_then(|v| v.as_str()).unwrap_or(""),
        "sourceCode": "",
        "destCode": "",
        "startDate": "",
        "route": stops,
        "track": []
    }))
}

/// Today's date in IST (`DD-MMM-YYYY`), the spelling the NTES form expects.
fn today_ist() -> String {
    let offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or_else(|| {
        // Fallback to UTC if the offset constant cannot be built (never in practice).
        chrono::FixedOffset::east_opt(0).unwrap()
    });
    chrono::Utc::now()
        .with_timezone(&offset)
        .format("%d-%b-%Y")
        .to_string()
}

fn str(v: &Value, key: &str, dflt: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or(dflt)
        .to_string()
}

/// `route` array entries -> `RouteStation`s with coordinates resolved from the
/// local dataset; the spot fields start empty (merged in later when a live
/// spot view is available).
fn route_entries(route_norm: &Value, state: &AppState) -> Vec<RouteStation> {
    route_norm
        .get("route")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            let code = str(&entry, "code", "");
            let (lat, lng) = state
                .datasets
                .coord(&code)
                .map(|(a, b)| (Some(a), Some(b)))
                .unwrap_or((None, None));
            RouteStation {
                code,
                name: str(&entry, "name", ""),
                lat,
                lng,
                arrival: str(&entry, "arrival", ""),
                departure: str(&entry, "departure", ""),
                day: str(&entry, "day", ""),
                distance: str(&entry, "distance", ""),
                days_of_run: str(&entry, "daysOfRun", ""),
                expected_arrival: String::new(),
                actual_arrival: String::new(),
                expected_departure: String::new(),
                actual_departure: String::new(),
                arrival_delay: String::new(),
                departure_delay: String::new(),
            }
        })
        .collect()
}

fn track_station(code: &Value, state: &AppState) -> TrackStation {
    let code = code.as_str().unwrap_or_default();
    let (lat, lng) = state
        .datasets
        .coord(code)
        .map(|(a, b)| (Some(a), Some(b)))
        .unwrap_or((None, None));
    TrackStation {
        code: code.to_string(),
        lat,
        lng,
    }
}

/// Overlay the live spot status (`arrivalDelay` / `expectedArrival`, ...) onto
/// the route stations. The NTES spot page reports a single displayed time plus
/// a delay badge per arrival/departure, so `actual_*` stays empty.
fn merge_spot(route: &mut [RouteStation], spot: &Value) {
    let status_by_code: HashMap<&str, &Value> = match spot.get("status").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter_map(|entry| {
                entry
                    .get("code")
                    .and_then(Value::as_str)
                    .map(|code| (code, entry))
            })
            .collect(),
        None => HashMap::new(),
    };
    for station in route.iter_mut() {
        if let Some(entry) = status_by_code.get(station.code.as_str()) {
            station.expected_arrival = str(entry, "expectedArrival", "");
            station.expected_departure = str(entry, "expectedDeparture", "");
            station.arrival_delay = str(entry, "arrivalDelay", "");
            station.departure_delay = str(entry, "departureDelay", "");
        }
    }
}
