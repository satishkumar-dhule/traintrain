use crate::models::{StationLite, Suggestion, TrainLite};
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

    /// Real station search over the pre-warmed station dataset.
    pub fn search_stations(state: &AppState, query: &str, limit: usize) -> Vec<StationLite> {
        state
            .datasets
            .search_stations(query, limit)
            .into_iter()
            .map(|s| StationLite {
                code: s.code,
                name: s.name,
            })
            .collect()
    }

    /// Combined station + train IntelliSense suggestions from the pre-warmed
    /// datasets, interleaved by relevance.
    pub fn suggest(state: &AppState, query: &str, limit: usize) -> Vec<Suggestion> {
        state
            .datasets
            .suggest(query, limit)
            .into_iter()
            .map(|s| Suggestion {
                r#type: s.kind,
                code: s.code,
                number: s.number,
                name: s.name,
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
