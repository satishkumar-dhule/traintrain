use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use futures::future::select_all;
use serde_json::Value;

use crate::core::error::AppError;
use crate::state::AppState;

/// State-of-art super fan-out N² deep delegation — DRY, KISS, hedged.
///
/// Patterns:
/// - **Request Hedging (availability)** — fan-out N×2 race N different hosts
///   (NTES vs Railyatri vs Paytm…), cancel losers; first success wins.
///   Patent-safe vs Google US9703890B2 replica hedging (heterogeneous hosts).
/// - **Timeout Budget** — per-source 5s, overall 10.5s (<12s frontend fetch).
/// - **Retry with Jitter** — deterministic 200ms + 0..100ms hash jitter.
/// - **Bounded Hedging** — global semaphore (`RAILWAY_FANOUT_CONCURRENCY`,
///   default 48) caps N×2×2 amplification; no unbounded upstream.
/// - **Circuit Breaker** — `Failover::should_skip` skips open sources with no
///   timeout paid; NotFound never trips breaker.
/// - **Single logical failure** — 2 attempts per candidate, one breaker bump.
///
/// Total attempts for N logical sources:
/// `N×2 delegates × 2 retries = N×4` raced, first success wins. Overall
/// deadline 10.5s ensures Singapore IP-block (NTES 5s) still lets a
/// worldwide delegate win in <1s. Metrics track every win/failure/latency
/// for state-of-art observability.
///
/// KISS: one function `fanout_n2`, one struct `Candidate`, one builder
/// `FanoutBuilder`. DRY: slices build candidates via builder, not 20-line
/// clone boilerplate each time.
const PER_SOURCE_TIMEOUT: Duration = Duration::from_secs(5);
const OVERALL_TIMEOUT: Duration = Duration::from_millis(10_500);
const RETRY_DELAY: Duration = Duration::from_millis(200);
const RETRY_JITTER_MAX_MS: u64 = 100;

/// Deterministic jitter per query+metric — no SystemTime, test-stable, KISS.
fn jitter_delay(query: &str, metric: &str) -> Duration {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    query.hash(&mut h);
    metric.hash(&mut h);
    let jitter = h.finish() % RETRY_JITTER_MAX_MS;
    RETRY_DELAY + Duration::from_millis(jitter)
}

type BoxFut = Pin<Box<dyn Future<Output = Result<Value, AppError>> + Send + 'static>>;
type Factory = Arc<dyn Fn() -> BoxFut + Send + Sync>;

pub struct Candidate {
    pub metric: &'static str,
    pub factory: Factory,
}

impl Candidate {
    pub fn new<F, Fut>(metric: &'static str, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, AppError>> + Send + 'static,
    {
        Self {
            metric,
            factory: Arc::new(move || Box::pin(f()) as BoxFut),
        }
    }
}

/// DRY builder — eliminates the 10-line per-candidate clone boilerplate
/// that was duplicated in 6 slices. KISS: one method `add`, then `run`.
///
/// ```ignore
/// let (metric, data) = FanoutBuilder::new(state, "live_status:12951")
///     .add("ntes", |s| s.ntes_web.train_status("12951"))
///     .add("railyatri", |s| async { railyatri_norm(s, "12951").await })
///     .run().await?;
/// ```
pub struct FanoutBuilder<'a> {
    state: &'a AppState,
    query: String,
    candidates: Vec<Candidate>,
}

impl<'a> FanoutBuilder<'a> {
    pub fn new(state: &'a AppState, query: impl Into<String>) -> Self {
        Self {
            state,
            query: query.into(),
            candidates: Vec::new(),
        }
    }

    /// Add a hedged candidate. `f` is `Fn(&AppState) -> Future<Result<Value,AppError>>`.
    /// DRY: clones state internally, caller just writes the fetch.
    pub fn add<F, Fut>(mut self, metric: &'static str, f: F) -> Self
    where
        F: Fn(AppState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, AppError>> + Send + 'static,
    {
        let state = self.state.clone();
        self.candidates.push(Candidate::new(metric, move || {
            let s = state.clone();
            f(s)
        }));
        self
    }

    /// Add a pre-built Candidate (escape hatch for custom closures needing captures).
    pub fn add_candidate(mut self, c: Candidate) -> Self {
        self.candidates.push(c);
        self
    }

    pub async fn run(self) -> Result<(String, Value), AppError> {
        fanout_n2(self.state, self.candidates, &self.query).await
    }
}

/// DRY free helper — alias for `fanout_n2` with clearer hedging name.
pub async fn hedge(
    state: &AppState,
    query: &str,
    candidates: Vec<Candidate>,
) -> Result<(String, Value), AppError> {
    fanout_n2(state, candidates, query).await
}

/// Race `candidates` concurrently. Returns `(winning_metric, payload)`.
///
/// State-of-art guarantees:
/// - Healthy-first ordering via `Failover::ordered` (no timeout paid for open circuits)
/// - Bounded concurrency via `state.fanout_limiter` (default 48)
/// - Per-source 5s, overall 10.5s budgets
/// - 2-deep retry with jitter, single logical breaker bump
/// - Honest error taxonomy: NotFound never trips breaker, Captcha surfaced as 428
/// - Full observability: fanout_total, win, failure, latency, overall timeout
pub async fn fanout_n2(
    state: &AppState,
    candidates: Vec<Candidate>,
    query: &str,
) -> Result<(String, Value), AppError> {
    if candidates.is_empty() {
        return Err(AppError::internal("fanout: no candidates"));
    }

    // Healthy-first ordering (stable sort, preserves caller's preference among healthy).
    let ordered_labels: Vec<&str> = state
        .failover
        .ordered(&candidates.iter().map(|c| c.metric).collect::<Vec<_>>())
        .into_iter()
        .collect();

    let mut sorted: Vec<(usize, &Candidate)> = candidates.iter().enumerate().collect();
    sorted.sort_by_key(|(_, c)| {
        ordered_labels
            .iter()
            .position(|l| *l == c.metric)
            .unwrap_or(usize::MAX)
    });
    let sorted_indices: Vec<usize> = sorted.into_iter().map(|(i, _)| i).collect();

    state.metrics.record_fanout();

    let mut futures = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for idx in sorted_indices {
        let cand = &candidates[idx];
        if state.failover.should_skip(cand.metric) {
            tracing::warn!(source = cand.metric, query = %query, "fanout: circuit open — skipped");
            skipped.push(format!("{}: circuit open (cooldown)", cand.metric));
            continue;
        }
        let metric = cand.metric;
        let factory = cand.factory.clone();
        let state_clone = state.clone();
        let query_owned = query.to_string();
        let fut = async move {
            let mut first_err: Option<AppError> = None;
            for attempt in 0..2 {
                // Bounded hedging: acquire global fanout permit for this attempt,
                // but never burn the whole 10.5s deadline queueing. A short
                // 800ms acquire budget degrades to a fast per-candidate fail
                // that lets the other hedged candidates win (fleet finding).
                let _permit = match tokio::time::timeout(
                    Duration::from_millis(800),
                    state_clone.fanout_limiter.acquire(),
                )
                .await
                {
                    Ok(p) => p.unwrap(),
                    Err(_) => {
                        tracing::warn!(source = metric, query = %query_owned, attempt, "fanout: semaphore acquire timed out");
                        // do not trip the circuit breaker — this is back-pressure, not a health signal
                        return Err(AppError::source_unavailable(
                            metric,
                            "semaphore acquire timeout (fanout back-pressure)",
                        ));
                    }
                };
                let started = std::time::Instant::now();
                let inner = (factory)();
                let res = tokio::time::timeout(PER_SOURCE_TIMEOUT, inner).await;
                let res = match res {
                    Ok(r) => r,
                    Err(_) => Err(AppError::source_unavailable(
                        metric,
                        format!("timeout after {}ms", PER_SOURCE_TIMEOUT.as_millis()),
                    )),
                };
                match res {
                    Ok(v) => {
                        state_clone
                            .metrics
                            .record_source_latency(metric, started.elapsed());
                        state_clone.failover.record_success(metric);
                        state_clone.metrics.record_fanout_win(metric);
                        tracing::info!(source = metric, query = %query_owned, attempt, "fanout: source won");
                        return Ok::<(String, Value), AppError>((metric.to_string(), v));
                    }
                    Err(e) => {
                        let is_live_failure = matches!(
                            e,
                            AppError::SourceUnavailable { .. } | AppError::Internal(_)
                        );
                        let is_not_found = matches!(e, AppError::NotFound(_));
                        if is_live_failure {
                            if attempt == 0 {
                                // Drop permit before jitter sleep so we don't hold concurrency
                                drop(_permit);
                                let d = jitter_delay(&query_owned, metric);
                                tokio::time::sleep(d).await;
                                first_err = Some(e);
                                continue;
                            }
                            state_clone.failover.record_failure(metric);
                            state_clone.metrics.record_source_failure(metric);
                            return Err(e);
                        }
                        if is_not_found {
                            return Err(e);
                        }
                        // BadRequest, Captcha, etc. — don't retry, don't trip.
                        return Err(e);
                    }
                }
            }
            if let Some(e) = first_err {
                state_clone.failover.record_failure(metric);
                state_clone.metrics.record_source_failure(metric);
                return Err(e);
            }
            Err(AppError::internal("fanout: retry exhausted"))
        };
        futures.push(Box::pin(fut));
    }

    if futures.is_empty() {
        let msg = if skipped.is_empty() {
            "all candidates skipped".to_string()
        } else {
            format!("all candidates circuit-open: {}", skipped.join(" | "))
        };
        return Err(AppError::source_unavailable("all-sources", msg));
    }

    let overall = async {
        let mut failures: Vec<String> = skipped;
        let mut not_founds: Vec<String> = Vec::new();
        let mut captcha: Option<AppError> = None;

        let mut pending = futures;
        while !pending.is_empty() {
            let (res, _idx, remaining) = select_all(pending).await;
            pending = remaining;
            match res {
                Ok((metric, val)) => return Ok::<(String, Value), AppError>((metric, val)),
                Err(AppError::NotFound(msg)) => {
                    not_founds.push(msg);
                }
                Err(AppError::CaptchaRequired(e)) => {
                    captcha = Some(AppError::CaptchaRequired(e));
                }
                Err(e) => {
                    let label = match &e {
                        crate::core::error::AppError::SourceUnavailable { source, .. } => {
                            source.clone()
                        }
                        _ => "source".to_string(),
                    };
                    failures.push(format!("{}: {}", label, e.message()));
                    let _ = e;
                }
            }
        }

        if let Some(c) = captcha {
            return Err(c);
        }
        if !not_founds.is_empty() && failures.is_empty() {
            return Err(AppError::not_found(not_founds.join(" | ")));
        }
        if !not_founds.is_empty() {
            failures.extend(not_founds.into_iter().map(|m| format!("not_found: {m}")));
        }
        Err(AppError::source_unavailable(
            "all-sources",
            format!(
                "fanout: {} candidates failed for '{}': {}",
                failures.len(),
                query,
                failures.join(" | ")
            ),
        ))
    };

    match tokio::time::timeout(OVERALL_TIMEOUT, overall).await {
        Ok(r) => r,
        Err(_) => {
            state.metrics.record_fanout_overall_timeout();
            Err(AppError::source_unavailable(
                "all-sources",
                format!(
                    "fanout overall timeout after {}ms for '{}'",
                    OVERALL_TIMEOUT.as_millis(),
                    query
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::state::AppState;
    use serde_json::json;

    fn test_state() -> AppState {
        let mut cfg = Config::default();
        cfg.http_timeout = std::time::Duration::from_secs(2);
        AppState::for_test(cfg)
    }

    #[tokio::test]
    async fn fanout_picks_first_success() {
        let state = test_state();
        let c1 = Candidate::new("ntes", || async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            Err::<Value, AppError>(AppError::source_unavailable("ntes", "boom"))
        });
        let c2 = Candidate::new("railyatri", || async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<Value, AppError>(json!({"ok": 1}))
        });
        let (m, v) = fanout_n2(&state, vec![c1, c2], "q").await.unwrap();
        assert_eq!(m, "railyatri");
        assert_eq!(v["ok"], 1);
    }

    #[tokio::test]
    async fn fanout_skips_open_circuit() {
        let state = test_state();
        for _ in 0..3 {
            state.failover.record_failure("ntes");
        }
        assert!(state.failover.should_skip("ntes"));
        let c1 = Candidate::new("ntes", || async {
            Ok::<Value, AppError>(json!({"from": "ntes"}))
        });
        let c2 = Candidate::new("railyatri", || async {
            Ok::<Value, AppError>(json!({"from": "railyatri"}))
        });
        let (m, v) = fanout_n2(&state, vec![c1, c2], "q").await.unwrap();
        assert_eq!(m, "railyatri");
        assert_eq!(v["from"], "railyatri");
    }

    #[tokio::test]
    async fn fanout_all_fail_returns_source_unavailable() {
        let state = test_state();
        let c1 = Candidate::new("ntes", || async {
            Err::<Value, AppError>(AppError::source_unavailable("ntes", "down"))
        });
        let c2 = Candidate::new("railyatri", || async {
            Err::<Value, AppError>(AppError::source_unavailable("railyatri", "down"))
        });
        let err = fanout_n2(&state, vec![c1, c2], "q").await.unwrap_err();
        assert!(matches!(err, AppError::SourceUnavailable { .. }));
    }

    #[tokio::test]
    async fn fanout_notfound_when_all_notfound() {
        let state = test_state();
        let c1 = Candidate::new("ntes", || async {
            Err::<Value, AppError>(AppError::not_found("no train"))
        });
        let c2 = Candidate::new("railyatri", || async {
            Err::<Value, AppError>(AppError::not_found("no train"))
        });
        let err = fanout_n2(&state, vec![c1, c2], "q").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn fanout_prefers_success_over_notfound() {
        let state = test_state();
        let c1 = Candidate::new("ntes", || async {
            Err::<Value, AppError>(AppError::not_found("no train"))
        });
        let c2 = Candidate::new("railyatri", || async {
            Ok::<Value, AppError>(json!({"found": true}))
        });
        let (m, v) = fanout_n2(&state, vec![c1, c2], "q").await.unwrap();
        assert_eq!(m, "railyatri");
        assert_eq!(v["found"], true);
    }

    #[tokio::test]
    async fn fanout_retries_once_records_single_logical_failure() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let state = test_state();
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let calls_for_closure = calls.clone();
        let c = Candidate::new("ntes", move || {
            let c = calls_for_closure.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Err::<Value, AppError>(AppError::source_unavailable("ntes", "down"))
            }
        });
        let err = fanout_n2(&state, vec![c], "q").await.unwrap_err();
        assert!(matches!(err, AppError::SourceUnavailable { .. }));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "deep delegation retries each candidate exactly once"
        );
        let snap = state.failover.snapshot();
        let ntes = snap
            .iter()
            .find(|s| s.source == "ntes")
            .expect("ntes tracked");
        assert_eq!(
            ntes.consecutive_failures, 1,
            "two attempts on the same source = one logical breaker failure"
        );
    }

    #[tokio::test]
    async fn builder_is_dry_equivalent() {
        let state = test_state();
        let (m, v) = FanoutBuilder::new(&state, "q")
            .add("ntes", |_| async {
                Err::<Value, AppError>(AppError::source_unavailable("ntes", "down"))
            })
            .add("railyatri", |_| async {
                Ok::<Value, AppError>(json!({"ok": 2}))
            })
            .run()
            .await
            .unwrap();
        assert_eq!(m, "railyatri");
        assert_eq!(v["ok"], 2);
    }
}
