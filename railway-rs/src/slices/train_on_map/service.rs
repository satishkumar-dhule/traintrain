use std::collections::HashMap;

use serde_json::Value;

use crate::core::error::AppError;
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
        let date = date.map(str::to_string).unwrap_or_else(today_ist);
        let route_norm = state.ntes_web.train_route_map(train, &date).await?;
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
            data_source: Some("ntes".to_string()),
        };

        if let Some(code) = station {
            let code = code.to_ascii_uppercase();
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
                    tracing::warn!(%train, %code, err = %e.message(), "train-spot-map unavailable; returning route-only map")
                }
            }
        }
        Ok(resp)
    }
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
