use crate::models::{StationLite, TrainLite};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Real train search over the NTES master list.
    pub fn search_trains(state: &AppState, query: &str, limit: usize) -> Vec<TrainLite> {
        crate::data::filter_trains(&state.datasets.trains, query, limit)
            .into_iter()
            .map(|t| TrainLite {
                number: t.number,
                name: t.name,
            })
            .collect()
    }

    /// Real station search over the station dataset.
    pub fn search_stations(state: &AppState, query: &str, limit: usize) -> Vec<StationLite> {
        crate::data::filter_stations(&state.datasets.stations, query, limit)
            .into_iter()
            .map(|s| StationLite {
                code: s.code,
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
}
