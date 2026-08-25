use async_trait::async_trait;
use serde_json::Value;

use super::error::{AppError, CaptchaContext};
use super::http::HttpClient;

pub mod labels {
    pub const NTES: &str = "NTES";
    pub const RAILYATRI: &str = "Railyatri";
    pub const IRCTC: &str = "IRCTC";
    pub const PAYTM: &str = "Paytm";
    pub const COROVER: &str = "CoRover";
    pub const INDIAN_RAILWAYS: &str = "Indian Railways";
    pub const ETRAIN: &str = "etrain";
}
pub mod metric {
    pub const NTES: &str = "ntes";
    pub const RAILYATRI: &str = "railyatri";
    pub const IRCTC: &str = "irctc";
    pub const PAYTM: &str = "paytm";
    pub const COROVER_API: &str = "corover-api";
    pub const COROVER_CDN: &str = "corover-cdn";
}
pub const RETRY_DELAY_MS: u64 = 400;
pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

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
