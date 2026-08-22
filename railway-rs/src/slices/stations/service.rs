use crate::data::StationRecord;
use crate::models::Station;
use crate::state::AppState;

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
}
