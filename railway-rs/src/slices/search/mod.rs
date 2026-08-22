//! Search slice.
//!
//! Endpoints (all offline, backed by the pre-warmed local datasets):
//! - `GET /rail-api/search/trains?q=<query>`   -> JSON array of `TrainLite`
//! - `GET /rail-api/search/stations?q=<query>` -> JSON array of `StationRow`
//! - `GET /rail-api/search/suggest?q=<query>`  -> JSON array of `SuggestHit`
//!   (combined stations + trains for one-round-trip IntelliSense autocomplete)
//!
//! Both station response types carry the hydrated AskDISHA optionals
//! (`name_hi`/`name_gu`/`district`/... , F2 passthrough from the hydrated
//! `StationRecord`); keys are omitted whenever the dataset has no value, so
//! unhydrated rows keep the exact old shape on the wire.
//!
//! Trains: real local `data/trains.json` (`state.datasets.trains`) via
//! `Datasets::search_trains`; stations: real `data/stations.json` via
//! `Datasets::search_stations` — the single unified tiered ranking authority
//! (exact code > exact name > code prefix > name prefix) also used by
//! `/rail-api/stations` and by the station half of `suggest`, so every query
//! path agrees. Both lists are pre-warmed into lowercase indexes at startup
//! (`Datasets::rank_stations` matches against them, so no station is
//! re-lowercased per request). Multi-word queries like `q=MUMBAI RAJDHANI`
//! match whole names and rank all-token hits first. Empty query or no matches
//! -> empty array.
//!
//! Note: `GET /rail-api/trains?q=` is NOT part of this slice; train search
//! lives here only.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::models::TrainLite;
use crate::state::AppState;

pub mod service;

/// `GET /rail-api/search/stations` row: the ranked station identity plus the
/// hydrated AskDISHA optionals (F2). Absent values are omitted on the wire.
#[derive(Debug, Serialize)]
pub struct StationRow {
    pub code: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_hi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_gu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub train_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lng: Option<f64>,
}

impl From<crate::data::StationRecord> for StationRow {
    fn from(s: crate::data::StationRecord) -> Self {
        Self {
            code: s.code,
            name: s.name,
            name_hi: s.name_hi,
            name_gu: s.name_gu,
            district: s.district,
            address: s.address,
            train_count: s.train_count,
            lat: s.lat,
            lng: s.lng,
        }
    }
}

/// `GET /rail-api/search/suggest` hit - either a station (`code`) or a train
/// (`number`). Station hits carry the hydration fields the autocomplete
/// subtitle needs; trains never do (keys omitted).
#[derive(Debug, Serialize)]
pub struct SuggestHit {
    pub r#type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_hi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_gu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
}

const SEARCH_LIMIT: usize = 10;
const MAX_Q_LEN: usize = 128;

#[derive(Deserialize, Default)]
struct SearchQuery {
    q: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/rail-api/search/trains", get(search_trains))
        .route("/rail-api/search/stations", get(search_stations))
        .route("/rail-api/search/suggest", get(search_suggest))
}

fn clamp_q(q: Option<&str>) -> String {
    q.unwrap_or("").chars().take(MAX_Q_LEN).collect::<String>()
}

/// Real train search over the pre-warmed NTES master list, capped at 10 hits.
async fn search_trains(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<TrainLite>> {
    let query = clamp_q(q.q.as_deref());
    Json(service::Service::search_trains(
        &state,
        &query,
        SEARCH_LIMIT,
    ))
}

/// Real station search over the pre-warmed station dataset, capped at 10 hits.
/// Ranking comes from the single unified authority `Datasets::search_stations`
/// (exact code > exact name > code prefix, shortest first > name prefix,
/// shortest name first, then code); other stations are excluded. Rows carry
/// the hydrated AskDISHA optionals (F2).
async fn search_stations(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<StationRow>> {
    let query = clamp_q(q.q.as_deref());
    Json(service::Service::search_stations(
        &state,
        &query,
        SEARCH_LIMIT,
    ))
}

/// Combined station + train autocomplete, capped at 10 hits.
async fn search_suggest(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<SuggestHit>> {
    let query = clamp_q(q.q.as_deref());
    Json(service::Service::suggest(&state, &query, SEARCH_LIMIT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Datasets, StationRecord};
    use std::collections::HashMap;

    fn sample_datasets() -> Datasets {
        Datasets::new(sample_stations(), Vec::new(), HashMap::new())
    }

    /// Test-only constructor: the hydrated optional fields stay `None`.
    fn rec(code: &str, name: &str, state: &str, zone: &str) -> StationRecord {
        StationRecord {
            code: code.into(),
            name: name.into(),
            state: state.into(),
            zone: zone.into(),
            ..StationRecord::default()
        }
    }

    fn sample_stations() -> Vec<StationRecord> {
        vec![
            rec("BCY", "VARANASI CITY", "Uttar Pradesh", "NR"),
            rec("BSB", "VARANASI JN", "Uttar Pradesh", "NR"),
            rec("BSBY", "VARANASI YARD", "Uttar Pradesh", "NR"),
            rec("NDLS", "NEW DELHI", "Delhi", "NR"),
            rec("NDPL", "NDPL", "", ""),
            rec("NZM", "HAZRAT NIZAMUDDIN", "Delhi", "NR"),
            rec("DDR", "DELHI CANTT", "Delhi", "NR"),
            rec("BX", "ND HALT", "", ""),
        ]
    }

    #[test]
    fn name_prefix_ranks_shortest_name_first() {
        let hits = sample_datasets().search_stations("Varanasi", 10);
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].code, "BSB", "VARANASI JN first");
        assert_eq!(hits[1].code, "BCY", "VARANASI CITY second");
        assert_eq!(hits[2].code, "BSBY", "VARANASI YARD last");
    }

    #[test]
    fn code_prefix_outranks_name_prefix() {
        let hits = sample_datasets().search_stations("ND", 10);
        assert_eq!(hits[0].code, "NDLS", "shortest code-prefix first");
        assert_eq!(hits.last().unwrap().code, "BX", "name-prefix match last");
        assert!(hits.iter().any(|s| s.code == "NDLS"));
        assert!(hits.iter().any(|s| s.code == "NDPL"));
        assert!(hits.iter().any(|s| s.code == "BX"));
        assert!(hits.iter().all(|s| s.code != "NZM"));
    }

    #[test]
    fn exact_name_match_is_case_insensitive() {
        let hits = sample_datasets().search_stations("new delhi", 10);
        assert_eq!(
            hits.len(),
            1,
            "contains matches like DELHI CANTT are excluded"
        );
        assert_eq!(hits[0].code, "NDLS");
    }

    #[test]
    fn exact_code_match_ranks_first() {
        let hits = sample_datasets().search_stations("NDLS", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].code, "NDLS");
        assert_eq!(hits[0].name, "NEW DELHI");
    }

    #[test]
    fn empty_or_whitespace_query_is_empty() {
        assert!(sample_datasets().search_stations("", 10).is_empty());
        assert!(sample_datasets().search_stations("   ", 10).is_empty());
    }

    #[test]
    fn non_matching_query_is_empty() {
        assert!(sample_datasets()
            .search_stations("zzznothing", 10)
            .is_empty());
    }

    #[test]
    fn search_path_varanasi_bsb_first_on_real_data() {
        let state = AppState::for_test(crate::config::Config::default());
        let hits = state.datasets.search_stations("Varanasi", 10);
        let codes: Vec<&str> = hits.iter().map(|s| s.code.as_str()).collect();
        assert_eq!(codes, vec!["BSB", "BCY", "BSBY"]);
    }

    #[test]
    fn suggest_path_varanasi_bsb_first_on_real_data() {
        let state = AppState::for_test(crate::config::Config::default());
        let hits = state.datasets.suggest("Varanasi", 10);
        let station_codes: Vec<&str> = hits
            .iter()
            .filter(|h| h.kind == "station")
            .map(|h| h.code.as_deref().unwrap_or_default())
            .collect();
        assert_eq!(station_codes, vec!["BSB", "BCY", "BSBY"]);
    }

    #[test]
    fn search_and_suggest_paths_agree_on_varanasi() {
        let state = AppState::for_test(crate::config::Config::default());
        let via_search: Vec<String> = state
            .datasets
            .search_stations("Varanasi", 10)
            .iter()
            .map(|s| s.code.clone())
            .collect();
        let via_suggest: Vec<String> = state
            .datasets
            .suggest("Varanasi", 10)
            .iter()
            .filter(|h| h.kind == "station")
            .map(|h| h.code.clone().unwrap_or_default())
            .collect();
        assert!(!via_search.is_empty(), "real data has Varanasi stations");
        assert_eq!(via_search, via_suggest, "both query paths must agree");
    }

    #[test]
    fn nd_sample_code_prefix_ranks_shortest_first() {
        let d = sample_datasets();
        let hits = d.search_stations("ND", 10);
        assert_eq!(hits[0].code, "NDLS", "shortest code-prefix first");
        assert!(hits.iter().any(|s| s.code == "NDPL"));
        assert!(hits.iter().any(|s| s.code == "BX"));
        assert!(hits.iter().all(|s| s.code != "NZM"));
    }

    #[test]
    fn nd_real_data_exact_code_then_code_prefix() {
        let state = AppState::for_test(crate::config::Config::default());
        let hits = state.datasets.search_stations("ND", 100);
        assert_eq!(hits[0].code, "ND", "exact-code station NADIAD JN first");
        assert!(
            hits.iter().any(|s| s.code == "NDLS"),
            "NDLS in the code-prefix tier"
        );
    }

    #[test]
    fn case_insensitive_exact_name_on_dataset_path() {
        let state = AppState::for_test(crate::config::Config::default());
        let hits = state.datasets.search_stations("new delhi", 10);
        assert_eq!(hits[0].code, "NDLS");
        assert_eq!(hits[0].name, "NEW DELHI");
    }

    #[test]
    fn empty_query_is_empty_on_both_paths() {
        let state = AppState::for_test(crate::config::Config::default());
        assert!(state.datasets.search_stations("", 10).is_empty());
        assert!(state.datasets.search_stations("   ", 10).is_empty());
        assert!(state.datasets.suggest("", 10).is_empty());
        assert!(state.datasets.suggest("   ", 10).is_empty());
    }

    /// F2: `/rail-api/search/stations` rows carry the hydrated AskDISHA
    /// optionals (fixture values for NDLS) and present keys are serialized.
    #[test]
    fn station_rows_carry_hydration_fields() {
        let state = AppState::for_test(crate::config::Config::default());
        let rows = service::Service::search_stations(&state, "NEW DELHI", 10);
        let ndls = rows
            .iter()
            .find(|s| s.code == "NDLS")
            .expect("real dataset contains NDLS");
        assert_eq!(ndls.name_hi.as_deref(), Some("नई दिल्ली"));
        assert_eq!(ndls.district.as_deref(), Some("Central"));
        assert_eq!(ndls.lat, Some(28.642314));
        let wire = serde_json::to_string(ndls).unwrap();
        assert!(wire.contains("\"name_hi\""), "present field serialized");
    }

    /// F2 wire shape: a record without hydration serializes as exactly the
    /// old two keys - no `null`s, no extra fields.
    #[test]
    fn unhydrated_station_rows_omit_optional_keys() {
        let row = StationRow::from(rec("BX", "ND HALT", "", ""));
        assert_eq!(
            serde_json::to_string(&row).unwrap(),
            r#"{"code":"BX","name":"ND HALT"}"#
        );
    }

    /// F2: station suggestions carry the hydration fields the autocomplete
    /// subtitle needs; train hits never do.
    #[test]
    fn suggest_station_hits_carry_subtitle_fields_train_hits_do_not() {
        let state = AppState::for_test(crate::config::Config::default());

        let hits = service::Service::suggest(&state, "NDLS", 10);
        let ndls = hits
            .iter()
            .find(|h| h.r#type == "station" && h.code.as_deref() == Some("NDLS"))
            .expect("NDLS suggested");
        assert_eq!(ndls.name_hi.as_deref(), Some("नई दिल्ली"));
        assert_eq!(ndls.district.as_deref(), Some("Central"));
        let wire = serde_json::to_string(ndls).unwrap();
        assert!(wire.contains("\"name_hi\""), "present field serialized");

        let hits = service::Service::suggest(&state, "12951", 10);
        let train = hits
            .iter()
            .find(|h| h.r#type == "train" && h.number.as_deref() == Some("12951"))
            .expect("12951 suggested");
        let wire = serde_json::to_string(train).unwrap();
        assert!(
            !wire.contains("name_hi"),
            "train hits must not emit subtitle keys: {wire}"
        );
    }
}
