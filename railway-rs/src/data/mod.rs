//! Shared datasets loaded from `data/` at startup (real data only):
//! - `stations.json`  - 8,958 real Indian Railway stations
//! - `trains.json`    - 10,609 real trains (fetched from the NTES master list)
//!
//! Both the `stations` and `search` slices read these via `AppState`.

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::core::error::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct StationRecord {
    pub code: String,
    pub name: String,
    pub state: String,
    pub zone: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrainRecord {
    pub number: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Datasets {
    pub stations: Arc<Vec<StationRecord>>,
    pub trains: Arc<Vec<TrainRecord>>,
}

impl Datasets {
    pub fn load(data_dir: &Path) -> Result<Self, AppError> {
        let stations = load_stations(&data_dir.join("stations.json"))?;
        let trains = load_trains(&data_dir.join("trains.json"))?;
        Ok(Self {
            stations: Arc::new(stations),
            trains: Arc::new(trains),
        })
    }
}

pub fn load_stations(path: &Path) -> Result<Vec<StationRecord>, AppError> {
    let bytes = std::fs::read(path).map_err(|e| {
        AppError::internal(format!("cannot read station data {}: {e}", path.display()))
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| AppError::internal(format!("invalid station data {}: {e}", path.display())))
}

pub fn load_trains(path: &Path) -> Result<Vec<TrainRecord>, AppError> {
    let bytes = std::fs::read(path).map_err(|e| {
        AppError::internal(format!("cannot read train data {}: {e}", path.display()))
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| AppError::internal(format!("invalid train data {}: {e}", path.display())))
}

/// Case-insensitive substring search over station code/name, capped at `limit`.
pub fn filter_stations(
    stations: &[StationRecord],
    query: &str,
    limit: usize,
) -> Vec<StationRecord> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut matches: Vec<&StationRecord> = stations
        .iter()
        .filter(|s| s.code.to_lowercase().contains(&q) || s.name.to_lowercase().contains(&q))
        .collect();
    matches.sort_by_key(|s| {
        let lc = s.name.to_lowercase();
        let code = s.code.to_lowercase();
        (
            // exact code/name first, then prefix, then substring
            !code.eq(&q) && !lc.eq(&q),
            !code.starts_with(&q) && !lc.starts_with(&q),
            s.code.clone(),
        )
    });
    matches.iter().take(limit).map(|s| (*s).clone()).collect()
}

/// Case-insensitive substring search over train number/name, capped at `limit`.
pub fn filter_trains(trains: &[TrainRecord], query: &str, limit: usize) -> Vec<TrainRecord> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut matches: Vec<&TrainRecord> = trains
        .iter()
        .filter(|t| t.number.contains(&q) || t.name.to_lowercase().contains(&q))
        .collect();
    matches.sort_by_key(|t| {
        let name = t.name.to_lowercase();
        (
            !t.number.eq(&q) && !name.eq(&q),
            !t.number.starts_with(&q),
            t.number.clone(),
        )
    });
    matches.iter().take(limit).map(|t| (*t).clone()).collect()
}

/// Raw deserialisation helper used by slices that read arbitrary JSON.
pub fn parse_value(bytes: &[u8]) -> Result<Value, AppError> {
    serde_json::from_slice(bytes).map_err(|e| AppError::internal(format!("bad JSON: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_stations() -> Vec<StationRecord> {
        serde_json::from_value(serde_json::json!([
            { "code": "NDLS", "name": "New Delhi", "state": "Delhi", "zone": "NR" },
            { "code": "BCT", "name": "Mumbai Central", "state": "Maharashtra", "zone": "WR" },
            { "code": "MUM", "name": "Mumbai", "state": "Maharashtra", "zone": "CR" },
            { "code": "NZM", "name": "Hazrat Nizamuddin", "state": "Delhi", "zone": "NR" },
        ]))
        .unwrap()
    }

    fn sample_trains() -> Vec<TrainRecord> {
        serde_json::from_value(serde_json::json!([
            { "number": "12951", "name": "MUMBAI RAJDHANI" },
            { "number": "12952", "name": "MUMBAI RAJDHANI" },
            { "number": "12001", "name": "NDLS SHATABDI" },
        ]))
        .unwrap()
    }

    #[test]
    fn stations_search_by_code_and_name() {
        let s = sample_stations();
        assert_eq!(filter_stations(&s, "NDLS", 5).len(), 1);
        assert_eq!(filter_stations(&s, "mumbai", 5).len(), 2);
        assert_eq!(filter_stations(&s, "delhi", 5).len(), 1);
        assert_eq!(filter_stations(&s, "  ", 5).len(), 0);
        assert_eq!(filter_stations(&s, "zzz", 5).len(), 0);
    }

    #[test]
    fn trains_search_and_limit() {
        let t = sample_trains();
        assert_eq!(filter_trains(&t, "1295", 5).len(), 2);
        assert_eq!(filter_trains(&t, "rajdhani", 1).len(), 1);
        assert_eq!(filter_trains(&t, "12951", 5).len(), 1);
    }
}
