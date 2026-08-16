use crate::models::Station;
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Case-insensitive IntelliSense search over the pre-warmed station dataset.
    pub fn search(state: &AppState, query: &str, limit: usize) -> Vec<Station> {
        state
            .datasets
            .search_stations(query, limit)
            .into_iter()
            .map(|s| Station {
                code: s.code,
                name: s.name,
                city: s.state,
                zone: s.zone,
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
    fn search_returns_real_stations() {
        let state = AppState::for_test(Config::default());
        let hits = Service::search(&state, "NDLS", 5);
        assert!(!hits.is_empty(), "expected real stations for NDLS");
        assert!(hits.iter().any(|s| s.code == "NDLS"));
    }
}
