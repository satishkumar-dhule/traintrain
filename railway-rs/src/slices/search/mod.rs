//! Search slice.
//!
//! Endpoints (all offline, backed by the pre-warmed local datasets):
//! - `GET /rail-api/search/trains?q=<query>`   -> JSON array of `TrainLite`
//! - `GET /rail-api/search/stations?q=<query>` -> JSON array of `StationLite`
//! - `GET /rail-api/search/suggest?q=<query>`  -> JSON array of `Suggestion`
//!   (combined stations + trains for one-round-trip IntelliSense autocomplete)
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
use serde::Deserialize;

use crate::models::{StationLite, Suggestion, TrainLite};
use crate::state::AppState;

pub mod service;

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
/// shortest name first, then code); other stations are excluded.
async fn search_stations(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<StationLite>> {
    let query = clamp_q(q.q.as_deref());
    let hits = state
        .datasets
        .search_stations(&query, SEARCH_LIMIT)
        .into_iter()
        .map(|s| StationLite {
            code: s.code,
            name: s.name,
        })
        .collect();
    Json(hits)
}

/// Combined station + train autocomplete, capped at 10 hits.
async fn search_suggest(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<Vec<Suggestion>> {
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

    fn sample_stations() -> Vec<StationRecord> {
        vec![
            StationRecord {
                code: "BCY".into(),
                name: "VARANASI CITY".into(),
                state: "Uttar Pradesh".into(),
                zone: "NR".into(),
            },
            StationRecord {
                code: "BSB".into(),
                name: "VARANASI JN".into(),
                state: "Uttar Pradesh".into(),
                zone: "NR".into(),
            },
            StationRecord {
                code: "BSBY".into(),
                name: "VARANASI YARD".into(),
                state: "Uttar Pradesh".into(),
                zone: "NR".into(),
            },
            StationRecord {
                code: "NDLS".into(),
                name: "NEW DELHI".into(),
                state: "Delhi".into(),
                zone: "NR".into(),
            },
            StationRecord {
                code: "NDPL".into(),
                name: "NDPL".into(),
                state: "".into(),
                zone: "".into(),
            },
            StationRecord {
                code: "NZM".into(),
                name: "HAZRAT NIZAMUDDIN".into(),
                state: "Delhi".into(),
                zone: "NR".into(),
            },
            StationRecord {
                code: "DDR".into(),
                name: "DELHI CANTT".into(),
                state: "Delhi".into(),
                zone: "NR".into(),
            },
            StationRecord {
                code: "BX".into(),
                name: "ND HALT".into(),
                state: "".into(),
                zone: "".into(),
            },
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
}
