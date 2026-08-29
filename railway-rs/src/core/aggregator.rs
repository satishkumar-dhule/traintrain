use std::sync::Arc;

use futures::future::select_all;

use super::error::{AppError, CaptchaContext, CaptchaRequiredError};
use super::http::HttpClient;
use super::source::{DataSource, SourceOutcome};

/// Fan-out aggregator ("sub-agent swarm") — legacy engine.
///
/// **DRY / KISS note:** the canonical hedged fan-out is now
/// `crate::core::fanout::fanout_n2` (N×2 deep delegation, bounded
/// concurrency, jitter, circuit-breaker, state-of-art metrics).
/// `AgentAggregator` is kept only for backward compatibility and for the
/// hermetic unit tests that inject a custom `HttpClient`. New code must use
/// `fanout::FanoutBuilder` / `fanout::hedge` — one engine, one truth.
///
/// Executes every registered source concurrently and returns the first
/// successful result. If all sources fail, it surfaces a CAPTCHA challenge if
/// any source requested one, otherwise a combined honest error.
#[derive(Clone)]
pub struct AgentAggregator {
    sources: Arc<Vec<Box<dyn DataSource>>>,
}

impl AgentAggregator {
    pub fn new(sources: Vec<Box<dyn DataSource>>) -> Self {
        Self {
            sources: Arc::new(sources),
        }
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.sources.iter().map(|s| s.name()).collect()
    }

    /// Race all sources; return the winner as `(source_name, payload)`.
    ///
    /// `target` routes the request to exactly one named source (used to solve
    /// a CAPTCHA on the same agent that raised it).
    pub async fn execute(
        &self,
        client: &HttpClient,
        query: &str,
        target: Option<&str>,
        captcha: Option<&CaptchaContext>,
    ) -> Result<SourceOutcome, AppError> {
        if self.sources.is_empty() {
            return Err(AppError::internal("no data sources configured"));
        }

        if let Some(target) = target {
            let source = self
                .sources
                .iter()
                .find(|s| s.name() == target)
                .ok_or_else(|| AppError::bad_request(format!("Source {target} not found")))?;
            let data = source.fetch(client, query, captcha).await?;
            return Ok(SourceOutcome {
                source: source.name().to_string(),
                data,
            });
        }

        let mut captcha_challenge: Option<CaptchaRequiredError> = None;
        let mut failures: Vec<String> = Vec::new();

        // Fan out once to every source and race them; each future is polled
        // exactly once. The first success wins; otherwise we collect failures
        // and surface a captcha challenge if any source asked for one.
        let mut futures = self
            .sources
            .iter()
            .map(|s| {
                let name = s.name();
                let q = query.to_string();
                Box::pin(async move {
                    match s.fetch(client, &q, None).await {
                        Ok(data) => Ok((name.to_string(), data)),
                        Err(AppError::CaptchaRequired(e)) => Err(AppError::CaptchaRequired(e)),
                        Err(e) => Err(AppError::internal(e.message())),
                    }
                })
            })
            .collect::<Vec<_>>();

        loop {
            if futures.is_empty() {
                break;
            }
            let (result, _idx, remaining) = select_all(futures).await;
            futures = remaining;

            match result {
                Ok((source, data)) => {
                    return Ok(SourceOutcome { source, data });
                }
                Err(AppError::CaptchaRequired(e)) => {
                    captcha_challenge = Some(e);
                }
                Err(e) => {
                    failures.push(e.message());
                }
            }
        }

        if let Some(challenge) = captcha_challenge {
            return Err(AppError::CaptchaRequired(challenge));
        }

        Err(AppError::source_unavailable(
            "all-sources",
            format!(
                "{} sub-agents failed to resolve query '{}': {}",
                self.sources.len(),
                query,
                failures.join(" | ")
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::http::HttpClient;
    use crate::core::source::DataSource;
    use async_trait::async_trait;
    use serde_json::Value;

    struct OkSource(&'static str);
    #[async_trait]
    impl DataSource for OkSource {
        fn name(&self) -> &'static str {
            self.0
        }
        async fn fetch(
            &self,
            _c: &HttpClient,
            q: &str,
            _cap: Option<&CaptchaContext>,
        ) -> Result<Value, AppError> {
            Ok(Value::String(format!("{q}-data")))
        }
    }

    struct FailSource;
    #[async_trait]
    impl DataSource for FailSource {
        fn name(&self) -> &'static str {
            "fail"
        }
        async fn fetch(
            &self,
            _c: &HttpClient,
            _q: &str,
            _cap: Option<&CaptchaContext>,
        ) -> Result<Value, AppError> {
            Err(AppError::internal("boom"))
        }
    }

    #[tokio::test]
    async fn returns_fastest_success() {
        let agg = AgentAggregator::new(vec![Box::new(FailSource), Box::new(OkSource("fast"))]);
        let client = HttpClient::new("t", std::time::Duration::from_secs(5)).unwrap();
        let out = agg.execute(&client, "q", None, None).await.unwrap();
        assert_eq!(out.source, "fast");
        assert_eq!(out.data, Value::String("q-data".into()));
    }

    #[tokio::test]
    async fn all_fail_returns_combined_error() {
        let agg = AgentAggregator::new(vec![Box::new(FailSource), Box::new(FailSource)]);
        let client = HttpClient::new("t", std::time::Duration::from_secs(5)).unwrap();
        let err = agg.execute(&client, "q", None, None).await.unwrap_err();
        assert!(
            matches!(err, AppError::SourceUnavailable { .. }),
            "unexpected: {err:?}"
        );
    }

    #[tokio::test]
    async fn routes_to_target_source() {
        let agg = AgentAggregator::new(vec![Box::new(OkSource("a")), Box::new(OkSource("b"))]);
        let client = HttpClient::new("t", std::time::Duration::from_secs(5)).unwrap();
        let out = agg.execute(&client, "q", Some("b"), None).await.unwrap();
        assert_eq!(out.source, "b");
    }

    #[tokio::test]
    async fn unknown_target_is_bad_request() {
        let agg = AgentAggregator::new(vec![Box::new(OkSource("a"))]);
        let client = HttpClient::new("t", std::time::Duration::from_secs(5)).unwrap();
        let err = agg
            .execute(&client, "q", Some("nope"), None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }
}
