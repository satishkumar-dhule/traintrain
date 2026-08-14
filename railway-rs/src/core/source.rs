use async_trait::async_trait;
use serde_json::Value;

use super::error::{AppError, CaptchaContext};
use super::http::HttpClient;

/// Result of a single source fetch.
#[derive(Debug)]
pub struct SourceOutcome {
    /// Name of the source that produced the data (shown to the client as `data_source`).
    pub source: String,
    /// Structured payload, later normalised by the owning vertical slice.
    pub data: Value,
}

/// A live data source (sub-agent) participating in the fan-out pattern.
///
/// Implementations must return real upstream data or an honest error - never
/// fabricated values.
#[async_trait]
pub trait DataSource: Send + Sync {
    fn name(&self) -> &'static str;

    /// Fetch and normalise the data for `query`.
    ///
    /// `captcha` is `Some` only when a prior attempt surfaced a
    /// `CaptchaRequiredError` and the client solved it; `text == "REFRESH"`
    /// asks the source to issue a fresh challenge.
    async fn fetch(
        &self,
        client: &HttpClient,
        query: &str,
        captcha: Option<&CaptchaContext>,
    ) -> Result<Value, AppError>;
}
