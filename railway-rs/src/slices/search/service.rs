use std::time::{Duration, Instant};

use super::{StationRow, SuggestHit};
use crate::core::corover::{self, SOURCE_API};
use crate::core::error::AppError;
use crate::models::TrainLite;
use crate::state::AppState;

/// Cache window for station-search results. The AskDISHA typeahead corpus is
/// effectively static, so the winning rows (CoRover or dataset) are reused
/// for half an hour - matching the AskDISHA nearby-lookup window.
const SEARCH_TTL: Duration = Duration::from_secs(30 * 60);

pub struct Service;

impl Service {
    /// Real train search over the pre-warmed NTES master list. Matches train
    /// number and name (IntelliSense-style ranking). CoRover exposes no
    /// train-search endpoint, so this stays offline-only by design.
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

    /// Real station search - Ask DISHA (`bot/searchStation` via the CoRover
    /// guest API) is the primary origin; the pre-warmed local dataset is the
    /// offline fallback taken when the module is disabled, upstream fails or
    /// answers an empty list. Rows are capped at `limit` before caching and
    /// the winning source wins the shared cache key, so a degraded upstream
    /// never re-hits on every keystroke.
    pub async fn search_stations(state: &AppState, query: &str, limit: usize) -> Vec<StationRow> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let key = format!("search:stations:{}", query.to_lowercase());
        if let Some(v) = state.cache.get(&key) {
            if let Ok(rows) = serde_json::from_value::<Vec<StationRow>>(v) {
                return rows;
            }
        }

        let corover_failure = match corover_station_rows(state, query).await {
            Ok(rows) => {
                let rows: Vec<StationRow> =
                    rows.into_iter().map(StationRow::from).take(limit).collect();
                tracing::info!(
                    %query,
                    source = "CoRover",
                    count = rows.len(),
                    "station search resolved from CoRover"
                );
                // CoRover typeahead never answers empty for real stations; an
                // empty list means the corpus has no such entry yet, so fall
                // through to the local authority instead of caching nothing.
                if !rows.is_empty() {
                    cache_rows(state, &key, &rows);
                    return rows;
                }
                format!("upstream answered no rows ({SOURCE_API})")
            }
            Err(e) => e.message(),
        };

        let rows = local_station_rows(state, query, limit);
        tracing::info!(
            %query,
            %corover_failure,
            source = "dataset",
            count = rows.len(),
            "station search resolved from the local dataset after CoRover"
        );
        // Only real hits are cached, so an all-miss query retries CoRover on
        // its next occurrence rather than being pinned empty for the TTL.
        if !rows.is_empty() {
            cache_rows(state, &key, &rows);
        }
        rows
    }

    /// Combined station + train IntelliSense suggestions from the pre-warmed
    /// datasets, interleaved by relevance. Deliberately offline: trains have
    /// no CoRover equivalent, and keeping both halves on one ranking
    /// authority preserves the single ordering invariant shared with
    /// `search_trains`.
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

/// Ask DISHA `bot/searchStation` leg of the chain. No-op when the module is
/// disabled (`state.askdisha` is `None`) - reports a source-unavailable error
/// without any outbound call, exactly like the schedule slice's primary leg.
async fn corover_station_rows(
    state: &AppState,
    query: &str,
) -> Result<Vec<corover::StationRow>, AppError> {
    let client = state
        .askdisha
        .as_deref()
        .ok_or_else(|| AppError::source_unavailable(SOURCE_API, "askdisha module disabled"))?;

    let started = Instant::now();
    let rows = client.search_station(query).await?;
    state
        .metrics
        .record_source_latency(SOURCE_API, started.elapsed());
    Ok(rows)
}

fn local_station_rows(state: &AppState, query: &str, limit: usize) -> Vec<StationRow> {
    state
        .datasets
        .search_stations(query, limit)
        .into_iter()
        .map(StationRow::from)
        .collect()
}

fn cache_rows(state: &AppState, key: &str, rows: &[StationRow]) {
    if let Ok(v) = serde_json::to_value(rows) {
        state.cache.set_with_ttl(key, v, SEARCH_TTL);
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
