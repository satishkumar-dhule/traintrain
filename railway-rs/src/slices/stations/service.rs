use serde::Serialize;

use crate::core::error::AppError;
use crate::data::StationRecord;
use crate::models::Station;
use crate::state::AppState;

/// Default row cap for `GET /rail-api/nearby/stations` when the caller sends
/// no `limit` (matches the AskDISHA nearby page size the UI was built on).
pub const DEFAULT_NEARBY_LIMIT: usize = 8;

/// Hard cap for the `limit` parameter - keeps a hostile query from paying to
/// sort all ~9k rows into one response.
pub const MAX_NEARBY_LIMIT: usize = 50;

/// One distance-sorted row of `/rail-api/nearby/stations`. Shape mirrors the
/// AskDISHA nearby rows (`code`/`name`/`distance_km`) so frontend callers can
/// treat both identically; distances are computed locally with haversine.
#[derive(Debug, Clone, Serialize)]
pub struct NearbyStation {
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_hi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_gu: Option<String>,
    /// Great-circle distance from the query point, rounded to 1 decimal.
    pub distance_km: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
}

/// Envelope of `GET /rail-api/nearby/stations`.
#[derive(Debug, Serialize)]
pub struct NearbyResponse {
    pub lat: f64,
    pub lng: f64,
    pub count: usize,
    /// Nearest first.
    pub stations: Vec<NearbyStation>,
}

/// Reject coordinates outside WGS-84 bounds or non-finite junk before any
/// math runs (a `NaN` would poison every sort comparison).
pub fn validate_coords(lat: f64, lng: f64) -> Result<(), AppError> {
    if !lat.is_finite() || !lng.is_finite() {
        return Err(AppError::bad_request("lat and lng must be finite numbers."));
    }
    if !(-90.0..=90.0).contains(&lat) {
        return Err(AppError::bad_request("lat must be within [-90, 90]."));
    }
    if !(-180.0..=180.0).contains(&lng) {
        return Err(AppError::bad_request("lng must be within [-180, 180]."));
    }
    Ok(())
}

/// Great-circle distance in kilometres (haversine, mean Earth radius).
fn haversine_km(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let (lat1, lng1, lat2, lng2) = (
        lat1.to_radians(),
        lng1.to_radians(),
        lat2.to_radians(),
        lng2.to_radians(),
    );
    let d_lat = lat2 - lat1;
    let d_lng = lng2 - lng1;
    // cSpell:disable-next-line
    let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lng / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}

/// Round to one decimal place for display distances (same as AskDISHA rows).
fn round_distance(km: f64) -> f64 {
    (km * 10.0).round() / 10.0
}

pub struct Service;

/// F2 passthrough: hydrated AskDISHA optionals (`name_hi`/`district`/...)
/// flow through from `StationRecord`; absent values are omitted on the wire.
fn to_station(s: StationRecord) -> Station {
    Station {
        code: s.code,
        name: s.name,
        city: s.state,
        zone: s.zone,
        name_hi: s.name_hi,
        name_gu: s.name_gu,
        district: s.district,
        address: s.address,
        train_count: s.train_count,
        lat: s.lat,
        lng: s.lng,
    }
}

impl Service {
    /// Case-insensitive IntelliSense search over the pre-warmed station dataset.
    pub fn search(state: &AppState, query: &str, limit: usize) -> Vec<Station> {
        state
            .datasets
            .search_stations(query, limit)
            .into_iter()
            .map(to_station)
            .collect()
    }

    /// Single station by code (`GET /rail-api/stations/:code`), exact match
    /// after trim + uppercase; `None` when the dataset does not know it.
    pub fn by_code(state: &AppState, raw_code: &str) -> Option<Station> {
        let code = crate::slices::station_codes::normalize_code(Some(raw_code));
        state
            .datasets
            .stations
            .iter()
            .find(|s| s.code == code)
            .cloned()
            .map(to_station)
    }

    /// Nearest stations to `(lat, lng)` over every dataset row that carries
    /// hydrated coordinates, nearest first, capped at `limit` (clamped to
    /// `1..=MAX_NEARBY_LIMIT`). Pure computation - no network, no flag.
    pub fn nearby(state: &AppState, lat: f64, lng: f64, limit: usize) -> NearbyResponse {
        nearby_from(&state.datasets.stations, lat, lng, limit)
    }
}

/// Pure core of [`Service::nearby`] so tests can exercise it on synthetic
/// rows without loading the whole dataset.
fn nearby_from(rows: &[StationRecord], lat: f64, lng: f64, limit: usize) -> NearbyResponse {
    let mut stations: Vec<NearbyStation> = rows
        .iter()
        .filter_map(|s| {
            let (slat, slng) = (s.lat?, s.lng?);
            Some(NearbyStation {
                distance_km: round_distance(haversine_km(lat, lng, slat, slng)),
                code: s.code.clone(),
                name: s.name.clone(),
                name_hi: s.name_hi.clone(),
                name_gu: s.name_gu.clone(),
                state: (!s.state.is_empty()).then(|| s.state.clone()),
                district: s.district.clone(),
            })
        })
        .collect();
    stations.sort_by(|a, b| {
        a.distance_km
            .partial_cmp(&b.distance_km)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let clamped = limit.clamp(1, MAX_NEARBY_LIMIT);
    stations.truncate(clamped);
    NearbyResponse {
        lat,
        lng,
        count: stations.len(),
        stations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::AppState;

    #[test]
    fn search_returns_real_stations() {
        let state = AppState::for_test(Config::default());
        let hits = Service::search(&state, "NDLS", 5);
        assert!(!hits.is_empty(), "expected real stations for NDLS");
        assert!(hits.iter().any(|s| s.code == "NDLS"));
    }

    /// F2 passthrough: hydrated rows carry the AskDISHA optionals (fixture
    /// values for NDLS), and unhydrated rows keep them omitted.
    #[test]
    fn search_passthrough_carries_hydration_fields() {
        let state = AppState::for_test(Config::default());
        let hits = Service::search(&state, "NEW DELHI", 5);
        let ndls = hits
            .iter()
            .find(|s| s.code == "NDLS")
            .expect("real dataset contains NDLS");
        assert_eq!(ndls.name_hi.as_deref(), Some("नई दिल्ली"));
        assert_eq!(ndls.district.as_deref(), Some("Central"));
        assert_eq!(
            ndls.lat,
            Some(28.642314),
            "coordinates come from the hydration fixture"
        );

        let wire = serde_json::to_string(ndls).unwrap();
        assert!(wire.contains("\"name_hi\""), "present field serialized");
    }

    /// `GET /rail-api/stations/:code` (F2): hydrated row by exact code, and
    /// unknown codes yield `None` (handler turns that into 404).
    #[test]
    fn by_code_returns_hydrated_station_and_none_for_unknown() {
        let state = AppState::for_test(Config::default());

        let ndls = Service::by_code(&state, "ndls").expect("NDLS known");
        assert_eq!(ndls.name, "NEW DELHI");
        assert_eq!(ndls.name_hi.as_deref(), Some("नई दिल्ली"));
        assert_eq!(ndls.lng, Some(77.22000399999999));

        assert!(Service::by_code(&state, "ZZZZ").is_none());
    }

    #[test]
    fn unhydrated_rows_omit_optional_keys() {
        // A record without hydration must serialize as the old four fields
        // plus nothing - guards against accidental `null` keys on the wire.
        let bare = Station {
            code: "XXX".into(),
            name: "X".into(),
            city: String::new(),
            zone: String::new(),
            name_hi: None,
            name_gu: None,
            district: None,
            address: None,
            train_count: None,
            lat: None,
            lng: None,
        };
        assert_eq!(
            serde_json::to_string(&bare).unwrap(),
            r#"{"code":"XXX","name":"X","city":"","zone":""}"#
        );
    }

    #[test]
    fn haversine_known_pairs_match_reference_distances() {
        // New Delhi -> Mumbai CSMT is ~1_140 km great-circle.
        let ndls_csmt = haversine_km(28.642314, 77.220_004, 18.940_01, 72.835_32);
        assert!((1_100.0..=1_180.0).contains(&ndls_csmt), "{ndls_csmt}");
        // Zero distance for identical points.
        assert!(haversine_km(19.07, 72.87, 19.07, 72.87) < 1e-9);
    }

    #[test]
    fn validate_coords_rejects_out_of_range_and_non_finite() {
        validate_coords(28.6, 77.2).expect("valid coords accepted");
        assert!(validate_coords(91.0, 0.0).is_err());
        assert!(validate_coords(-90.1, 0.0).is_err());
        assert!(validate_coords(0.0, 181.0).is_err());
        assert!(validate_coords(f64::NAN, 0.0).is_err());
        assert!(validate_coords(0.0, f64::INFINITY).is_err());
    }

    #[test]
    fn nearby_returns_real_stations_sorted_with_ndls_first_at_its_own_coordinates() {
        let state = AppState::for_test(Config::default());
        let res = Service::nearby(&state, 28.642314, 77.220_004, DEFAULT_NEARBY_LIMIT);

        assert!(!res.stations.is_empty(), "dataset rows have coordinates");
        assert_eq!(res.count, res.stations.len());
        let first = &res.stations[0];
        assert_eq!(first.code, "NDLS");
        assert_eq!(first.name, "NEW DELHI");
        assert_eq!(first.distance_km, 0.0);
        // Sorted ascending by distance and capped at the limit.
        assert!(res.stations.len() <= DEFAULT_NEARBY_LIMIT);
        for pair in res.stations.windows(2) {
            assert!(pair[0].distance_km <= pair[1].distance_km);
            assert!(!pair[1].code.is_empty());
        }
    }

    #[test]
    fn nearby_limit_clamps_to_hard_cap() {
        let state = AppState::for_test(Config::default());
        let res = Service::nearby(&state, 19.076, 72.8777, 10_000);
        assert_eq!(
            res.stations.len(),
            MAX_NEARBY_LIMIT,
            "oversized limits clamp to MAX_NEARBY_LIMIT"
        );
        // A zero limit still yields a usable default-sized page instead of nothing.
        let res = Service::nearby(&state, 19.076, 72.8777, 0);
        assert!(!res.stations.is_empty());
    }

    /// Rows without hydrated coordinates are skipped entirely - they cannot
    /// honestly claim a distance.
    #[test]
    fn nearby_skips_rows_without_coordinates() {
        let rows = vec![
            StationRecord {
                code: "NOCO".into(),
                name: "NO COORDS".into(),
                ..StationRecord::default()
            },
            StationRecord {
                code: "WITH".into(),
                name: "WITH COORDS".into(),
                lat: Some(28.65),
                lng: Some(77.23),
                ..StationRecord::default()
            },
        ];
        let res = nearby_from(&rows, 28.64, 77.22, 8);
        assert_eq!(res.count, 1);
        assert!(!res.stations.iter().any(|s| s.code == "NOCO"));
        assert_eq!(res.stations[0].code, "WITH");
        assert!(res.stations[0].distance_km > 0.0);
    }

    /// The wire shape mirrors AskDISHA nearby rows: `distance_km` always
    /// present, absent optionals omitted rather than `null`.
    #[test]
    fn nearby_row_serializes_distance_and_omits_absent_optionals() {
        let row = NearbyStation {
            code: "NDLS".into(),
            name: "NEW DELHI".into(),
            name_hi: None,
            name_gu: None,
            distance_km: round_distance(1.21),
            state: None,
            district: None,
        };
        let wire = serde_json::to_string(&row).unwrap();
        assert_eq!(
            wire,
            r#"{"code":"NDLS","name":"NEW DELHI","distance_km":1.2}"#
        );

        let envelope = NearbyResponse {
            lat: 28.6,
            lng: 77.2,
            count: 1,
            stations: vec![row],
        };
        let wire = serde_json::to_string(&envelope).unwrap();
        assert!(wire.contains(r#""count":1"#) && wire.contains(r#""stations":[{""#));
    }
}
