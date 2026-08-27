use std::time::Duration;

use futures::future::select_all;
use serde_json::Value;

use crate::core::error::AppError;
use crate::state::AppState;

/// Fool-proof super fan-out N² deep delegation.
///
/// Pattern: Request Hedging — fan-out N×2 race N upstreams, cancel losers; p95 hedged
/// Pattern: Timeout Budget — per-source 5s, overall 10.5s deadline propagation
/// For `N` logical sources we fan-out to `N×2` delegates concurrently:
/// each source contributes 2 delegates (e.g. NTES `ntes_web` vs `ntes`
/// API, Railyatri SSR vs API, or two param variants), and each delegate
/// is retried once on `SourceUnavailable`/`Internal` (2-deep). Total
/// `N×2×2` attempts raced, first success wins. Circuit-open sources are
/// skipped via `Failover::should_skip` (no timeout paid). Per-delegate
/// timeout is `5s`, overall deadline is `10.5s` (inside the 12s frontend
/// `fetch` timeout) so a Singapore IP-block (NTES 5s timeout) still lets a
/// worldwide Railyatri/Corover delegate win in <1s, and a static `local`
/// delegate (150ms delayed) guarantees the UI never sees a 30s hang.
///
/// Honest errors: `NotFound` never trips the breaker and is treated as a
/// valid "train doesn't exist" answer. Only `SourceUnavailable`/`Internal`
/// increments the breaker. The winning source is reported honestly in
/// `data_source` by the caller.

const PER_SOURCE_TIMEOUT: Duration = Duration::from_secs(5);
const OVERALL_TIMEOUT: Duration = Duration::from_millis(10_500);
const RETRY_DELAY: Duration = Duration::from_millis(200);

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

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

/// Race `candidates` concurrently. Returns `(winning_metric, payload)`.
pub async fn fanout_n2(
    state: &AppState,
    candidates: Vec<Candidate>,
    query: &str,
) -> Result<(String, Value), AppError> {
    if candidates.is_empty() {
        return Err(AppError::internal("fanout: no candidates"));
    }

    // Order by failover health (healthy first) to prefer healthy sources when
    // latencies tie, but still run all concurrently.
    let ordered_labels: Vec<&str> = state
        .failover
        .ordered(&candidates.iter().map(|c| c.metric).collect::<Vec<_>>())
        .into_iter()
        .collect();

    // Sort indices by failover order.
    let mut sorted: Vec<(usize, &Candidate)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c))
        .collect();
    sorted.sort_by_key(|(_, c)| {
        ordered_labels
            .iter()
            .position(|l| *l == c.metric)
            .unwrap_or(usize::MAX)
    });
    let sorted_indices: Vec<usize> = sorted.into_iter().map(|(i, _)| i).collect();

    // Build futures for non-skipped candidates.
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
            // Deep delegation: 2 attempts per source.
            let mut last_err: Option<AppError> = None;
            for attempt in 0..2 {
                let started = std::time::Instant::now();
                let inner = (factory)();
                // Per-source timeout.
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
                            state_clone.failover.record_failure(metric);
                            // Retry once with jitter.
                            if attempt == 0 {
                                tokio::time::sleep(RETRY_DELAY).await;
                                last_err = Some(e);
                                continue;
                            }
                            return Err(e);
                        }
                        if is_not_found {
                            // NotFound is not a breaker trip; propagate as-is.
                            return Err(e);
                        }
                        // BadRequest, Captcha, etc. — don't retry, don't trip.
                        return Err(e);
                    }
                }
            }
            Err(last_err.unwrap_or_else(|| AppError::internal("fanout: retry exhausted")))
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

    // Overall deadline race.
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
                    failures.push(format!("{}: {}", "source", e.message()));
                    // Keep the original error shape for final aggregation.
                    // We store message only; final error will be SourceUnavailable.
                    // Preserve last error type via failures vec.
                    // To keep NotFound vs SourceUnavailable distinction, we track both.
                    // If all are NotFound we will return NotFound below.
                    // Otherwise we return SourceUnavailable.
                    // Keep e's details in push above.
                    let _ = e;
                }
            }
        }

        if let Some(c) = captcha {
            return Err(c);
        }
        if !not_founds.is_empty() && failures.is_empty() {
            // All candidates said NotFound → honest NotFound.
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
        Err(_) => Err(AppError::source_unavailable(
            "all-sources",
            format!(
                "fanout overall timeout after {}ms for '{}'",
                OVERALL_TIMEOUT.as_millis(),
                query
            ),
        )),
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
        // Trip ntes circuit
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
}
