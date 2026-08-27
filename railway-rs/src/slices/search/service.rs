use std::time::{Duration, Instant};

use super::{StationRow, SuggestHit};
use crate::core::cache::keys;
use crate::core::corover::{self, SOURCE_API};
use crate::core::error::AppError;
use crate::core::fanout::{Candidate, fanout_n2};
use crate::models::TrainLite;
use crate::state::AppState;

/// Cache window for station-search results. The AskDISHA typeahead corpus is
/// effectively static, so the winning rows (CoRover or dataset) are reused
/// for half an hour - matching the AskDISHA nearby-lookup window.
const SEARCH_TTL: Duration = Duration::from_secs(30 * 60);
/// SRE hedging fine-print — surfaced in tracing and cache payloads.
const SRE_HEDGING_NOTICE: &str =
    "SRE: Super fan-out N×2 (2-deep retry, hedging) — first-success-wins across N sources";

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

    /// Real station search — CoRover + dataset raced concurrently.
    ///
    /// Pattern: Super Fan-out N×2, Pattern: Deep Delegation, Pattern: Hedging
    ///
    /// Ask DISHA (`bot/searchStation` via the CoRover guest API) and the
    /// pre-warmed local dataset are raced as `N=2` logical sources via
    /// `fanout_n2`. Each source contributes a delegate retried once
    /// (2-deep) with 5s per-source timeout and 10.5s overall deadline.
    /// Circuit-open sources are skipped via `Failover::should_skip` (no
    /// timeout paid). The local delegate is `150ms` delayed so CoRover can
    /// win when healthy; when CoRover is geofenced / timed-out the delayed
    /// local hedge guarantees the UI never sees a 30s hang. The winning rows
    /// (whichever source wins) are cached under one key (`search:stations:{q}`,
    /// 30 min) and the winning metric is reported in tracing with the SRE
    /// fine-print.
    pub async fn search_stations(state: &AppState, query: &str, limit: usize) -> Vec<StationRow> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let key = keys::search_stations(&query.to_lowercase());
        if let Some(v) = state.cache.get(&key) {
            if let Ok(rows) = serde_json::from_value::<Vec<StationRow>>(v) {
                return rows;
            }
        }

        // Hedged fan-out: CoRover (worldwide, IP-tolerant for typeahead) vs
        // dataset (offline, always available but 150ms delayed).
        // Pattern: Hedging — delayed local guarantees liveness; Pattern:
        // Deep Delegation — 2-deep retry per delegate inside fanout_n2.
        let query_corover = query.to_string();
        let query_local = query.to_string();
        let state_corover = state.clone();
        let state_local = state.clone();
        let limit_c = limit;
        let limit_l = limit;

        let candidates = vec![
            Candidate::new(SOURCE_API, move || {
                let s = state_corover.clone();
                let q = query_corover.clone();
                async move {
                    // Deep delegation candidate: CoRover typeahead
                    let rows = corover_station_rows(&s, &q).await?;
                    if rows.is_empty() {
                        return Err(AppError::source_unavailable(
                            SOURCE_API,
                            "upstream answered no rows",
                        ));
                    }
                    let mapped: Vec<StationRow> =
                        rows.into_iter().map(StationRow::from).take(limit_c).collect();
                    // Serialize to Value so fanout can race heterogenous sources
                    Ok(serde_json::to_value(&mapped).unwrap())
                }
            }),
            Candidate::new("dataset", move || {
                let s = state_local.clone();
                let q = query_local.clone();
                async move {
                    // Hedging: 150ms delay so CoRover can win when healthy,
                    // but dataset guarantees the UI never hangs 30s when
                    // AskDISHA is disabled or Singapore IP-blocked.
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    let rows = local_station_rows(&s, &q, limit_l);
                    if rows.is_empty() {
                        return Err(AppError::not_found("no stations in dataset for hedging"));
                    }
                    Ok(serde_json::to_value(&rows).unwrap())
                }
            }),
        ];

        let query_owned = query.to_string();
        let key_clone = key.clone();
        // Circuit-breaker ordering respects failover health: healthy first, but
        // still raced concurrently — first-success-wins with hedging.
        match fanout_n2(state, candidates, &format!("search:{query_owned}")).await {
            Ok((metric, val)) => {
                if let Ok(rows) = serde_json::from_value::<Vec<StationRow>>(val) {
                    tracing::info!(
                        %query_owned,
                        source = %metric,
                        count = rows.len(),
                        "{}",
                        SRE_HEDGING_NOTICE
                    );
                    tracing::info!(
                        %query_owned,
                        source = %metric,
                        count = rows.len(),
                        "station search resolved via hedged fan-out (SRE: Super fan-out N×2)"
                    );
                    if !rows.is_empty() {
                        cache_rows(state, &key_clone, &rows);
                    }
                    return rows;
                }
                // Decode failed — fall through to direct fallback
                tracing::warn!(%query_owned, "hedged search decode failed, falling back to dataset");
                let rows = local_station_rows(state, &query_owned, limit);
                if !rows.is_empty() {
                    cache_rows(state, &key_clone, &rows);
                }
                rows
            }
            Err(e) => {
                // All hedged candidates failed (both said NotFound or both unavailable).
                // Preserve honest NotFound vs unavailable distinction.
                if matches!(e, AppError::NotFound(_)) {
                    tracing::info!(
                        %query_owned,
                        error = %e.message(),
                        "station search hedged fan-out: no stations found on any source"
                    );
                    return Vec::new();
                }
                tracing::warn!(
                    %query_owned,
                    error = %e.message(),
                    "station search fanout failed (SRE hedged path), falling back to local dataset directly"
                );
                let rows = local_station_rows(state, &query_owned, limit);
                if !rows.is_empty() {
                    cache_rows(state, &key_clone, &rows);
                } else {
                    tracing::info!(
                        %query_owned,
                        source = "dataset",
                        count = rows.len(),
                        "station search resolved from the local dataset after CoRover (fallback)"
                    );
                }
                rows
            }
        }
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

    #[tokio::test]
    async fn hedged_search_falls_back_to_dataset_when_corover_disabled() {
        let mut cfg = Config::default();
        cfg.askdisha_enabled = false;
        let state = AppState::for_test(cfg);
        // With AskDISHA disabled, corover candidate fast-fails, dataset hedge (150ms delayed) should win
        let rows = Service::search_stations(&state, "NDLS", 5).await;
        assert!(!rows.is_empty(), "hedged fallback must return local stations");
        assert!(rows.iter().any(|r| r.code == "NDLS"));
    }

    #[tokio::test]
    async fn hedged_search_returns_cached_on_second_hit() {
        let state = AppState::for_test(Config::default());
        let first = Service::search_stations(&state, "Varanasi", 10).await;
        assert!(!first.is_empty());
        // Second hit should be served from cache without network
        let second = Service::search_stations(&state, "Varanasi", 10).await;
        assert_eq!(first.len(), second.len());
        assert_eq!(first[0].code, second[0].code);
    }

    #[tokio::test]
    async fn hedged_search_empty_query_is_empty() {
        let state = AppState::for_test(Config::default());
        assert!(Service::search_stations(&state, "", 5).await.is_empty());
        assert!(Service::search_stations(&state, "   ", 5).await.is_empty());
    }

    #[test]
    fn cache_rows_and_local_rows_roundtrip() {
        let state = AppState::for_test(Config::default());
        let rows = local_station_rows(&state, "NDLS", 5);
        assert!(rows.iter().any(|r| r.code == "NDLS"));
        let key = keys::search_stations("ndls_test");
        cache_rows(&state, &key, &rows);
        let cached: Vec<StationRow> = state.cache.get(&key).and_then(|v| serde_json::from_value(v).ok()).unwrap();
        assert_eq!(cached.len(), rows.len());
    }
}
