use super::{StationRow, SuggestHit};
use crate::models::TrainLite;
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Real train search over the pre-warmed NTES master list. Matches train
    /// number and name (IntelliSense-style ranking).
    pub fn search_trains(state: &AppState, query: &str, limit: usize) -> Vec<TrainLite> {
        state
            .datasets
            .search_trains(query, limit)
            .into_iter()
            .map(|t| TrainLite {
                number: t.number,
                name: t.name,
            })
            .collect()
    }

    /// Real station search over the unified ranking authority, mapped to the
    /// F2 row shape: hydration optionals flow through, absent values are
    /// omitted on the wire.
    pub fn search_stations(state: &AppState, query: &str, limit: usize) -> Vec<StationRow> {
        state
            .datasets
            .search_stations(query, limit)
            .into_iter()
            .map(StationRow::from)
            .collect()
    }

    /// Combined station + train IntelliSense suggestions from the pre-warmed
    /// datasets, interleaved by relevance. Stations are ranked by the same
    /// unified tiered authority as `Datasets::search_stations` and carry the
    /// hydration fields for the autocomplete subtitle.
    pub fn suggest(state: &AppState, query: &str, limit: usize) -> Vec<SuggestHit> {
        state
            .datasets
            .suggest(query, limit)
            .into_iter()
            .map(|s| SuggestHit {
                r#type: s.kind,
                code: s.code,
                number: s.number,
                name: s.name,
                name_hi: s.name_hi,
                name_gu: s.name_gu,
                district: s.district,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::AppState;

    #[test]
    fn train_search_returns_real_trains() {
        let state = AppState::for_test(Config::default());
        let hits = Service::search_trains(&state, "12951", 5);
        assert!(hits.iter().any(|t| t.number == "12951"));
        assert!(hits[0].name.contains("RAJDHANI") || hits[0].number == "12951");
    }

    #[test]
    fn suggest_returns_trains_and_stations() {
        let state = AppState::for_test(Config::default());
        let hits = Service::suggest(&state, "12951", 10);
        assert!(hits
            .iter()
            .any(|h| h.r#type == "train" && h.number.as_deref() == Some("12951")));

        let hits = Service::suggest(&state, "NDLS", 10);
        assert!(hits
            .iter()
            .any(|h| h.r#type == "station" && h.code.as_deref() == Some("NDLS")));
    }
}
