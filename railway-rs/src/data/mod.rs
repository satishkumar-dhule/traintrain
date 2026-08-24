//! Shared datasets loaded from `data/` at startup (real data only):
//! - `stations.json`        - 8,958 real Indian Railway stations, enriched
//!   offline with optional AskDISHA CDN fields (`name_hi`/`name_gu`/
//!   `district`/`address`/`train_count`/`lat`/`lng`) by the F1 hydrator
//!   (`src/bin/hydrate_stations.rs`, pure merge in [`merge_hydration`])
//! - `trains.json`          - 10,609 real trains (fetched from the NTES master list)
//! - `station_coords.txt`   - 8,773 `CODE\tLAT\tLNG` coordinates (from NTES's own
//!   `station_map_data.js`), used by the train-on-map slice.
//!
//! Both the `stations` and `search` slices read these via `AppState`. The coords
//! file is best-effort: a missing/empty/malformed file is logged and treated as
//! an empty map - it must never fail startup.
//!
//! The lists are *pre-warmed*: at startup every record is normalized once into
//! lowercase indexes (`station_lc`, `train_lc`), so autocomplete / search
//! requests never re-normalize the dataset and stay allocation-light. Station
//! search and autocomplete all go through the single tiered ranking authority
//! (`Datasets::rank_stations`, over the pre-warmed `station_lc` index): exact
//! code > exact name > code prefix (shortest first) > name prefix (shortest
//! name first). Train search is IntelliSense-style: it matches train numbers
//! *and* train names with a score so exact > prefix > contains, and multi-word
//! queries rank all-tokens matches above partial ones.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::error::AppError;

/// One row of `data/stations.json`. `code`/`name`/`state`/`zone` come from the
/// NTES master dataset; the remaining optional fields are AskDISHA CDN
/// hydration (F1, `src/bin/hydrate_stations.rs`) and are absent (`None`)
/// whenever the upstream capture does not know them or leaves them blank.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StationRecord {
    pub code: String,
    pub name: String,
    pub state: String,
    pub zone: String,
    /// Hindi name from the AskDISHA capture, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_hi: Option<String>,
    /// Gujarati name from the AskDISHA capture, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_gu: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Upstream `trainCount` (kept as string, e.g. `"244"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub train_count: Option<String>,
    /// Latitude; upstream emits `""`/`null`/non-numeric for stations without
    /// coordinates -> `None` (lenient parse, see [`de_lenient_f64`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    /// Longitude; see [`Self::lat`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lng: Option<f64>,
}

/// One row of the AskDISHA CDN `stationupdated.json` capture
/// (`testdata/askdisha/stationupdated_full.json`). Only the fields the
/// hydration merge consumes are declared; upstream sends more (`utterances`,
/// `state`, ...) which serde ignores. Upstream `state` is deliberately not
/// modelled: local `state`/`zone` always win (contract F1).
#[derive(Debug, Clone, Deserialize)]
pub struct HydrationRow {
    pub code: String,
    #[serde(default)]
    pub name_hi: Option<String>,
    #[serde(default)]
    pub name_gu: Option<String>,
    #[serde(default)]
    pub district: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default, rename = "trainCount")]
    pub train_count: Option<String>,
    #[serde(default, deserialize_with = "de_lenient_f64")]
    pub latitude: Option<f64>,
    #[serde(default, deserialize_with = "de_lenient_f64")]
    pub longitude: Option<f64>,
}

/// Report returned by [`merge_hydration`].
#[derive(Debug, Clone, PartialEq)]
pub struct HydrationReport {
    /// Distinct upstream codes merged into a local record.
    pub hydrated: usize,
    /// Distinct upstream codes seen (duplicate fixture codes collapse).
    pub total: usize,
    /// Upstream codes with no local station, in first-seen order.
    pub unmatched: Vec<String>,
}

/// Pure F1 hydration merge: enrich `local` records with the optional
/// AskDISHA fields, keyed by station code (case-insensitive, trimmed).
///
/// Collision policy:
/// - local authority always wins - `code`, `name`, `state` and `zone` are
///   never modified, even when upstream disagrees;
/// - blank/whitespace upstream strings count as absent -> `None`;
/// - coordinates parse leniently (`""`/`null`/non-numeric -> `None`);
/// - duplicate upstream codes collapse, last row wins;
/// - deterministic: the same inputs always produce identical records, so
///   re-running the hydrator over its own output is a no-op.
pub fn merge_hydration(
    mut local: Vec<StationRecord>,
    upstream: &[HydrationRow],
) -> (Vec<StationRecord>, HydrationReport) {
    // Deduplicate upstream codes (last row wins), keeping first-seen order so
    // the unmatched report is stable.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut unique: Vec<&HydrationRow> = Vec::new();
    for row in upstream {
        let key = row.code.trim().to_uppercase();
        if key.is_empty() {
            continue;
        }
        match index.get(&key) {
            Some(&i) => unique[i] = row,
            None => {
                index.insert(key, unique.len());
                unique.push(row);
            }
        }
    }
    let total = unique.len();

    let mut hydrated = 0usize;
    let mut matched: HashMap<String, ()> = HashMap::new();
    for record in local.iter_mut() {
        let key = record.code.trim().to_uppercase();
        let row = match index.get(&key).map(|&i| unique[i]) {
            Some(row) => row,
            None => continue,
        };
        record.name_hi = clean_opt(row.name_hi.as_deref());
        record.name_gu = clean_opt(row.name_gu.as_deref());
        record.district = clean_opt(row.district.as_deref());
        record.address = clean_opt(row.address.as_deref());
        record.train_count = clean_opt(row.train_count.as_deref());
        record.lat = row.latitude.filter(|v| v.is_finite());
        record.lng = row.longitude.filter(|v| v.is_finite());
        matched.insert(key, ());
        hydrated += 1;
    }

    let unmatched = unique
        .iter()
        .filter_map(|row| {
            let key = row.code.trim().to_uppercase();
            (!matched.contains_key(&key)).then(|| row.code.trim().to_string())
        })
        .collect();

    let report = HydrationReport {
        hydrated,
        total,
        unmatched,
    };
    (local, report)
}

/// Trim an optional string, treating blank as absent.
fn clean_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Lenient optional `f64` for bulk captures: `null`, blank strings,
/// non-numeric strings and unexpected types all become `None` instead of
/// failing the whole file. Mirrors the tolerance of
/// `core/corover.rs::de_opt_f64`, pushed one step further because a single
/// bad row must never abort a hydration run. Non-finite parses (`NaN`,
/// infinities) are dropped too - they have no JSON representation.
fn de_lenient_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let parsed = match <Value as Deserialize>::deserialize(deserializer)? {
        Value::Null => None,
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    Ok(parsed.filter(|v| v.is_finite()))
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrainRecord {
    pub number: String,
    pub name: String,
}

/// A combined autocomplete hit returned by `Datasets::suggest`.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// `"station"` or `"train"`.
    pub kind: &'static str,
    /// Station code (stations only).
    pub code: Option<String>,
    /// Train number (trains only).
    pub number: Option<String>,
    pub name: String,
    /// Hydrated AskDISHA fields carried by station hits only (F2 subtitle
    /// passthrough); trains always carry `None`.
    pub name_hi: Option<String>,
    pub name_gu: Option<String>,
    pub district: Option<String>,
}

impl Suggestion {
    fn station(s: &StationRecord) -> Self {
        Self {
            kind: "station",
            code: Some(s.code.clone()),
            number: None,
            name: s.name.clone(),
            name_hi: s.name_hi.clone(),
            name_gu: s.name_gu.clone(),
            district: s.district.clone(),
        }
    }

    fn train(number: &str, name: &str) -> Self {
        Self {
            kind: "train",
            code: None,
            number: Some(number.to_string()),
            name: name.to_string(),
            name_hi: None,
            name_gu: None,
            district: None,
        }
    }
}

/// Pre-warmed lowercase station index (built once at load).
#[derive(Debug, Clone)]
struct StationLc {
    code: String,
    name: String,
}

/// Pre-warmed lowercase train index (built once at load).
#[derive(Debug, Clone)]
struct TrainLc {
    name: String,
}

#[derive(Debug, Clone)]
pub struct Datasets {
    pub stations: Arc<Vec<StationRecord>>,
    pub trains: Arc<Vec<TrainRecord>>,
    /// `CODE` -> `(lat, lng)` from `station_coords.txt` (uppercase keys).
    coords: Arc<HashMap<String, (f64, f64)>>,
    station_lc: Arc<Vec<StationLc>>,
    train_lc: Arc<Vec<TrainLc>>,
}

impl Datasets {
    pub fn load(data_dir: &Path) -> Result<Self, AppError> {
        let stations = load_stations(&data_dir.join("stations.json"))?;
        let trains = load_trains(&data_dir.join("trains.json"))?;
        let coords = load_coords(&data_dir.join("station_coords.txt"));
        Ok(Self::new(stations, trains, coords))
    }

    /// BM25 index entries for the AI retrieval layer: every station and train
    /// becomes one searchable document.
    pub fn retrieval_entries(&self) -> Vec<crate::core::retrieval::IndexEntry> {
        use crate::core::retrieval::IndexEntry;
        let mut out = Vec::with_capacity(self.stations.len() + self.trains.len());
        for s in self.stations.iter() {
            out.push(IndexEntry {
                kind: "station",
                code: s.code.clone(),
                title: s.name.clone(),
                detail: format!("{} {}", s.state, s.district.clone().unwrap_or_default()),
            });
        }
        for t in self.trains.iter() {
            out.push(IndexEntry {
                kind: "train",
                code: t.number.clone(),
                title: t.name.clone(),
                detail: String::new(),
            });
        }
        out
    }

    /// Build a `Datasets` and pre-warm the lowercase indexes for both lists.
    /// Doing this once at startup keeps every later autocomplete/search cheap.
    pub fn new(
        stations: Vec<StationRecord>,
        trains: Vec<TrainRecord>,
        coords: HashMap<String, (f64, f64)>,
    ) -> Self {
        let station_lc = stations
            .iter()
            .map(|s| StationLc {
                code: s.code.to_lowercase(),
                name: s.name.to_lowercase(),
            })
            .collect();
        let train_lc = trains
            .iter()
            .map(|t| TrainLc {
                name: t.name.to_lowercase(),
            })
            .collect();
        Self {
            stations: Arc::new(stations),
            trains: Arc::new(trains),
            coords: Arc::new(coords),
            station_lc: Arc::new(station_lc),
            train_lc: Arc::new(train_lc),
        }
    }

    /// `(lat, lng)` for a station code (uppercase lookup), if known.
    pub fn coord(&self, code: &str) -> Option<(f64, f64)> {
        self.coords.get(&code.to_uppercase()).copied()
    }

    /// Official name for a station code (`NDLS` -> `NEW DELHI`), used by the
    /// NTES web forms which need the human-readable station name alongside the code.
    pub fn station_name(&self, code: &str) -> Option<&str> {
        self.stations
            .iter()
            .find(|s| s.code.eq_ignore_ascii_case(code))
            .map(|s| s.name.as_str())
    }

    /// Reverse lookup: code for an exact official station name (`AKOLA` ->
    /// `AK`). Case-insensitive, first match wins; `None` when no station is
    /// named exactly that. Used to sanity-check upstream train schedules
    /// against the stations implied by the local timetable index.
    pub fn station_code_by_name(&self, name: &str) -> Option<&str> {
        let wanted = name.to_ascii_uppercase();
        self.stations
            .iter()
            .find(|s| s.name.to_ascii_uppercase() == wanted)
            .map(|s| s.code.as_str())
    }

    /// Official name for a train number (`12951` -> `NDLS TEJAS RAJ`).
    /// Used when an NTES page does not echo the train identity (e.g. the
    /// no-exception "Train Exception Info" page only shows the requested
    /// number) but the local master list knows it.
    pub fn train_name(&self, number: &str) -> Option<&str> {
        self.trains
            .iter()
            .find(|t| t.number.eq_ignore_ascii_case(number))
            .map(|t| t.name.as_str())
    }

    /// IntelliSense train search over the pre-warmed index: matches number and
    /// name, ranks exact/prefix/contains, all-tokens matches first.
    pub fn search_trains(&self, query: &str, limit: usize) -> Vec<TrainRecord> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let tokens: Vec<&str> = q.split_whitespace().collect();
        let mut scored: Vec<(i64, &TrainRecord)> = Vec::new();
        for (t, lc) in self.trains.iter().zip(self.train_lc.iter()) {
            if let Some(score) = train_score(&t.number, &lc.name, &tokens) {
                scored.push((score, t));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.number.cmp(&b.1.number)));
        scored
            .iter()
            .take(limit)
            .map(|(_, t)| (*t).clone())
            .collect()
    }

    /// Station search over the pre-warmed index, capped at `limit`. Uses the
    /// single tiered ranking authority (`rank_stations`) shared by every
    /// station query path: `/rail-api/search/stations`, `/rail-api/stations`
    /// and the `suggest` autocomplete.
    pub fn search_stations(&self, query: &str, limit: usize) -> Vec<StationRecord> {
        self.rank_stations(query)
            .into_iter()
            .take(limit)
            .map(|s| (*s).clone())
            .collect()
    }

    /// Unified station ranking authority for search and autocomplete. Tier
    /// order:
    /// 1. exact code (case-insensitive) first
    /// 2. exact name (case-insensitive) next
    /// 3. code prefix, shortest code first
    /// 4. name prefix, shortest name first then code
    ///
    /// Anything else is excluded; empty/whitespace query -> empty. Consumes
    /// the pre-warmed lowercase `station_lc` index, so no record is
    /// re-lowercased per request.
    fn rank_stations<'a>(&'a self, query: &str) -> Vec<&'a StationRecord> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut exact_code: Vec<&StationRecord> = Vec::new();
        let mut exact_name: Vec<&StationRecord> = Vec::new();
        let mut code_prefix: Vec<&StationRecord> = Vec::new();
        let mut name_prefix: Vec<&StationRecord> = Vec::new();
        for (s, lc) in self.stations.iter().zip(self.station_lc.iter()) {
            if lc.code == q {
                exact_code.push(s);
            } else if lc.name == q {
                exact_name.push(s);
            } else if lc.code.starts_with(&q) {
                code_prefix.push(s);
            } else if lc.name.starts_with(&q) {
                name_prefix.push(s);
            }
        }
        code_prefix.sort_by(|a, b| {
            a.code
                .len()
                .cmp(&b.code.len())
                .then_with(|| a.code.cmp(&b.code))
        });
        name_prefix.sort_by(|a, b| {
            a.name
                .len()
                .cmp(&b.name.len())
                .then_with(|| a.code.cmp(&b.code))
        });
        let mut out = exact_code;
        out.extend(exact_name);
        out.extend(code_prefix);
        out.extend(name_prefix);
        out
    }

    /// Combined station + train autocomplete. Stations are ranked by the same
    /// unified tiered authority as `search_stations`, so the From/To box and
    /// the search endpoint agree; trains keep the IntelliSense score and fill
    /// the remaining slots. `suggest("1295")` and `suggest("MUMBAI RAJDHANI")`
    /// return the best of both lists in one hit.
    pub fn suggest(&self, query: &str, limit: usize) -> Vec<Suggestion> {
        let mut scored: Vec<(i64, Suggestion)> = Vec::new();

        for (idx, s) in self.rank_stations(query).into_iter().enumerate() {
            scored.push((1000 - idx as i64, Suggestion::station(s)));
        }
        let q = query.trim().to_lowercase();
        let tokens: Vec<&str> = q.split_whitespace().collect();
        for (t, lc) in self.trains.iter().zip(self.train_lc.iter()) {
            if let Some(score) = train_score(&t.number, &lc.name, &tokens) {
                scored.push((score, Suggestion::train(&t.number, &t.name)));
            }
        }

        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.kind.cmp(b.1.kind)));
        scored.iter().take(limit).map(|(_, s)| s.clone()).collect()
    }
}

/// IntelliSense score for a train against a query's tokens. Every token must
/// match the number or the name; all-tokens matches get a strong boost so they
/// always outrank partial matches. Returns `None` when nothing matches.
fn train_score<T: AsRef<str>>(number: &str, name_lc: &str, tokens: &[T]) -> Option<i64> {
    let mut total = 0i64;
    let mut matched = 0usize;
    for raw in tokens {
        let tok = raw.as_ref();
        let mut best = 0i64;
        if number.contains(tok) {
            best = if number.starts_with(tok) { 4 } else { 2 };
        }
        if name_lc.contains(tok) {
            let ns = if name_lc == tok {
                6
            } else if name_lc.starts_with(tok) {
                5
            } else {
                3
            };
            if ns > best {
                best = ns;
            }
        }
        if best == 0 {
            continue;
        }
        matched += 1;
        total += best;
    }
    if matched == 0 {
        return None;
    }
    if matched == tokens.len() && tokens.len() > 1 {
        total += 100;
    }
    Some(total)
}

/// IntelliSense score for a station against a query's tokens. Station codes are
/// weighted above names (a 4-char code is the canonical identifier).
fn station_score<T: AsRef<str>>(code_lc: &str, name_lc: &str, tokens: &[T]) -> Option<i64> {
    let mut total = 0i64;
    let mut matched = 0usize;
    for raw in tokens {
        let tok = raw.as_ref();
        let mut best = 0i64;
        if code_lc.contains(tok) {
            best = if code_lc == tok {
                8
            } else if code_lc.starts_with(tok) {
                6
            } else {
                4
            };
        }
        if name_lc.contains(tok) {
            let ns = if name_lc == tok {
                5
            } else if name_lc.starts_with(tok) {
                4
            } else {
                2
            };
            if ns > best {
                best = ns;
            }
        }
        if best == 0 {
            continue;
        }
        matched += 1;
        total += best;
    }
    if matched == 0 {
        return None;
    }
    if matched == tokens.len() && tokens.len() > 1 {
        total += 100;
    }
    Some(total)
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

/// Load `CODE\tLAT\tLNG` lines into an uppercase-keyed coord map. A missing,
/// empty or malformed file never fails startup - it is logged as a warning and
/// treated as an empty map; individual malformed lines are skipped.
pub fn load_coords(path: &Path) -> HashMap<String, (f64, f64)> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "station coords file unreadable; using an empty map");
            return HashMap::new();
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "station coords file is not UTF-8; using an empty map");
            return HashMap::new();
        }
    };
    let mut map = HashMap::new();
    for line in text.lines() {
        let mut cols = line.split('\t');
        let code = cols.next().unwrap_or("").trim();
        if code.is_empty() {
            continue;
        }
        let lat = match cols.next().and_then(|s| s.trim().parse::<f64>().ok()) {
            Some(v) => v,
            None => continue,
        };
        let lng = match cols.next().and_then(|s| s.trim().parse::<f64>().ok()) {
            Some(v) => v,
            None => continue,
        };
        map.insert(code.to_string(), (lat, lng));
    }
    if map.is_empty() {
        tracing::warn!(path = %path.display(), "no valid station coords parsed; using an empty map");
    }
    map
}

/// Case-insensitive substring search over station code/name, capped at `limit`.
/// Thin wrapper over the pre-warmed index for callers holding raw records.
pub fn filter_stations(
    stations: &[StationRecord],
    query: &str,
    limit: usize,
) -> Vec<StationRecord> {
    let lc: Vec<StationLc> = stations
        .iter()
        .map(|s| StationLc {
            code: s.code.to_lowercase(),
            name: s.name.to_lowercase(),
        })
        .collect();
    let tokens = tokenize(query);
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(i64, &StationRecord)> = Vec::new();
    for (s, l) in stations.iter().zip(lc.iter()) {
        if let Some(score) = station_score(&l.code, &l.name, &tokens) {
            scored.push((score, s));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.code.cmp(&b.1.code)));
    scored
        .iter()
        .take(limit)
        .map(|(_, s)| (*s).clone())
        .collect()
}

/// Case-insensitive substring search over train number/name, capped at `limit`.
/// Thin wrapper over the pre-warmed index for callers holding raw records.
pub fn filter_trains(trains: &[TrainRecord], query: &str, limit: usize) -> Vec<TrainRecord> {
    let lc: Vec<TrainLc> = trains
        .iter()
        .map(|t| TrainLc {
            name: t.name.to_lowercase(),
        })
        .collect();
    let tokens = tokenize(query);
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(i64, &TrainRecord)> = Vec::new();
    for (t, l) in trains.iter().zip(lc.iter()) {
        if let Some(score) = train_score(&t.number, &l.name, &tokens) {
            scored.push((score, t));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.number.cmp(&b.1.number)));
    scored
        .iter()
        .take(limit)
        .map(|(_, t)| (*t).clone())
        .collect()
}

fn tokenize(query: &str) -> Vec<String> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    q.split_whitespace().map(str::to_string).collect()
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

    fn sample_datasets() -> Datasets {
        Datasets::new(sample_stations(), sample_trains(), HashMap::new())
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

    #[test]
    fn trains_search_matches_number_and_name() {
        let d = sample_datasets();
        assert_eq!(d.search_trains("12951", 5)[0].number, "12951");
        assert_eq!(d.search_trains("rajdhani", 5).len(), 2);
        assert_eq!(d.search_trains("SHATABDI", 5)[0].name, "NDLS SHATABDI");
    }

    #[test]
    fn trains_multiword_ranks_all_tokens_first() {
        let d = sample_datasets();
        let hits = d.search_trains("mumbai rajdhani", 5);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|t| t.name.contains("RAJDHANI")));
    }

    #[test]
    fn stations_code_prefix_outranks_name_contains() {
        let d = sample_datasets();
        // "mu" matches code-less names (Mumbai Central / Mumbai, name-prefix) and
        // "MUM" code-prefix; the code hit must rank first.
        let hits = d.search_stations("MUM", 5);
        assert_eq!(hits[0].code, "MUM");
    }

    #[test]
    fn suggest_returns_both_kinds() {
        let d = sample_datasets();
        let hits = d.suggest("12951", 10);
        assert!(hits
            .iter()
            .any(|s| s.kind == "train" && s.number.as_deref() == Some("12951")));

        let hits = d.suggest("NDLS", 10);
        assert!(hits
            .iter()
            .any(|s| s.kind == "station" && s.code.as_deref() == Some("NDLS")));
    }

    #[test]
    fn suggest_empty_query_is_empty() {
        let d = sample_datasets();
        assert!(d.suggest("   ", 10).is_empty());
        assert!(d.suggest("zzzznothing", 10).is_empty());
    }

    #[test]
    fn coord_lookup_is_uppercase_insensitive() {
        let mut coords = HashMap::new();
        coords.insert("NDLS".to_string(), (28.6426, 77.2197));
        let d = Datasets::new(Vec::new(), Vec::new(), coords);
        assert_eq!(d.coord("NDLS"), Some((28.6426, 77.2197)));
        assert_eq!(d.coord("ndls"), Some((28.6426, 77.2197)));
        assert_eq!(d.coord("DDN"), None);
    }

    #[test]
    fn load_coords_skips_malformed_lines() {
        let dir = std::env::temp_dir().join(format!("railway_coords_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("coords.txt");
        std::fs::write(
            &path,
            "NDLS\t28.6426\t77.2197\nBAD\tnot-a-lat\t77.2197\nJUSTCODE\n\nDDN\t30.3165\t78.0322\n",
        )
        .unwrap();
        let map = load_coords(&path);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(map.get("NDLS"), Some(&(28.6426, 77.2197)));
        assert_eq!(map.get("DDN"), Some(&(30.3165, 78.0322)));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn load_coords_missing_file_is_empty() {
        let path = std::path::Path::new("/definitely/not/here/coords.txt");
        assert!(load_coords(path).is_empty());
    }

    fn hydration_row(json: serde_json::Value) -> HydrationRow {
        serde_json::from_value(json).unwrap()
    }

    /// Local authority: `code`/`name`/`state`/`zone` survive the merge even
    /// when upstream knows the station; only the optional fields are set.
    #[test]
    fn hydration_keeps_local_identity_fields() {
        let local = vec![StationRecord {
            code: "NDLS".into(),
            name: "NEW DELHI".into(),
            state: "Delhi".into(),
            zone: "NR".into(),
            ..Default::default()
        }];
        let upstream = vec![hydration_row(serde_json::json!({
            "code": "ndls",
            "name": "SOME OTHER NAME",
            "name_hi": "नई दिल्ली",
            "district": "Central",
            "state": "Upstream State",
            "trainCount": "244"
        }))];
        let (records, report) = merge_hydration(local, &upstream);
        assert_eq!(report.hydrated, 1);
        assert_eq!(report.total, 1);
        assert!(report.unmatched.is_empty());
        let ndls = &records[0];
        assert_eq!(ndls.code, "NDLS");
        assert_eq!(ndls.name, "NEW DELHI");
        assert_eq!(ndls.state, "Delhi", "local state always wins");
        assert_eq!(ndls.zone, "NR", "local zone always wins");
        assert_eq!(ndls.name_hi.as_deref(), Some("नई दिल्ली"));
        assert_eq!(ndls.district.as_deref(), Some("Central"));
        assert_eq!(ndls.train_count.as_deref(), Some("244"));
    }

    #[test]
    fn hydration_reports_unmatched_codes() {
        let local = vec![StationRecord {
            code: "NDLS".into(),
            name: "NEW DELHI".into(),
            state: "Delhi".into(),
            zone: "NR".into(),
            ..Default::default()
        }];
        let upstream = vec![
            hydration_row(serde_json::json!({ "code": "NDLS" })),
            hydration_row(serde_json::json!({ "code": "ZZZZ" })),
            hydration_row(serde_json::json!({ "code": "YYYY" })),
        ];
        let (records, report) = merge_hydration(local, &upstream);
        assert_eq!(report.hydrated, 1);
        assert_eq!(report.total, 3);
        assert_eq!(report.unmatched, vec!["ZZZZ", "YYYY"]);
        assert!(records[0].name_hi.is_none());
    }

    /// Tolerance: blank strings count as absent, coordinates parse leniently
    /// (`""`/null/non-numeric -> None), numeric strings are accepted and
    /// non-finite parses are dropped.
    #[test]
    fn hydration_tolerates_blank_fields_and_bad_coordinates() {
        let code = |c: &str| c.to_string();
        let local: Vec<StationRecord> = ["AA", "BB", "CC", "DD"]
            .iter()
            .map(|c| StationRecord {
                code: code(c),
                name: "X".into(),
                state: String::new(),
                zone: String::new(),
                ..Default::default()
            })
            .collect();
        let upstream = vec![
            hydration_row(serde_json::json!({
                "code": "aa", "name_hi": "", "district": "  ",
                "latitude": "", "longitude": null
            })),
            hydration_row(serde_json::json!({
                "code": "BB", "address": "",
                "latitude": "not-a-number", "longitude": "28.64"
            })),
            hydration_row(serde_json::json!({
                "code": "cc", "trainCount": "0", "latitude": "NaN"
            })),
        ];
        let (records, report) = merge_hydration(local, &upstream);
        assert_eq!(report.hydrated, 3);
        assert!(records[0].name_hi.is_none());
        assert!(records[0].district.is_none());
        assert_eq!(records[0].lat, None);
        assert_eq!(records[0].lng, None);
        assert!(records[1].address.is_none());
        assert_eq!(records[1].lat, None, "non-numeric string -> None");
        assert_eq!(records[1].lng, Some(28.64), "numeric string accepted");
        assert_eq!(records[2].train_count.as_deref(), Some("0"));
        assert_eq!(records[2].lat, None, "non-finite parse dropped");
    }

    /// Duplicate upstream codes collapse (last row wins).
    #[test]
    fn hydration_collapses_duplicate_upstream_codes() {
        let local = sample_stations();
        let upstream = vec![
            hydration_row(serde_json::json!({ "code": "NDLS", "district": "Wrong" })),
            hydration_row(serde_json::json!({ "code": "NDLS", "district": "Central" })),
        ];
        let (records, report) = merge_hydration(local, &upstream);
        assert_eq!(report.total, 1);
        assert_eq!(report.hydrated, 1);
        assert_eq!(records[0].district.as_deref(), Some("Central"));
    }

    /// Idempotence: merging over already-hydrated data reproduces byte-identical
    /// JSON, so re-running the hydrator is a no-op.
    #[test]
    fn hydration_is_idempotent() {
        let local = sample_stations();
        let upstream = vec![
            hydration_row(serde_json::json!({
                "code": "NDLS", "name_hi": "नई दिल्ली", "district": "Central",
                "latitude": 28.6426, "longitude": 77.2197
            })),
            hydration_row(serde_json::json!({ "code": "BCT", "address": "Mumbai" })),
        ];
        let (once, _) = merge_hydration(local.clone(), &upstream);
        let first = serde_json::to_vec(&once).unwrap();
        let (twice, report) = merge_hydration(once, &upstream);
        let second = serde_json::to_vec(&twice).unwrap();
        assert_eq!(first, second, "re-run must be byte-identical");
        assert_eq!(report.hydrated, 2);
    }

    /// Old four-field `stations.json` rows must keep loading unchanged, and
    /// hydrated records serialize without the absent optional keys.
    #[test]
    fn station_record_roundtrips_old_four_field_json() {
        let old = serde_json::json!([
            { "code": "NDLS", "name": "New Delhi", "state": "Delhi", "zone": "NR" }
        ]);
        let records: Vec<StationRecord> = serde_json::from_value(old).unwrap();
        assert_eq!(records.len(), 1);
        let r = &records[0];
        assert!(r.name_hi.is_none());
        assert!(r.name_gu.is_none());
        assert!(r.district.is_none());
        assert!(r.address.is_none());
        assert!(r.train_count.is_none());
        assert!(r.lat.is_none());
        assert!(r.lng.is_none());

        // Absent optionals are omitted on Serialize, so the wire shape of an
        // unhydrated row stays exactly the old four fields.
        let wire = serde_json::to_string(&r).unwrap();
        assert_eq!(
            wire,
            r#"{"code":"NDLS","name":"New Delhi","state":"Delhi","zone":"NR"}"#
        );

        // A fully hydrated record round-trips losslessly.
        let hydrated = StationRecord {
            name_hi: Some("नई दिल्ली".into()),
            name_gu: Some("નઈ દિલ્લી".into()),
            district: Some("Central".into()),
            address: Some("Paharganj, New Delhi".into()),
            train_count: Some("244".into()),
            lat: Some(28.6426),
            lng: Some(77.2197),
            ..r.clone()
        };
        let bytes = serde_json::to_vec(&hydrated).unwrap();
        let back: StationRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, hydrated);
    }
}
