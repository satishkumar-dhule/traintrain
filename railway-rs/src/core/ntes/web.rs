//! NTES public website (enquiry.indianrail.gov.in/mntes) form client.
//!
//! The `/q` endpoints are CSRF-protected and answer with an HTML/JSON mix.
//! From the sandbox they are blocked (`Request Rejected` / 404), so every call
//! normally surfaces `AppError::SourceUnavailable`. Callers must propagate
//! that honestly - never fabricate train data.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::super::error::AppError;
use super::super::http::HttpClient;

/// Characters of a body to keep when reporting a decode failure.
const SNIPPET_CHARS: usize = 120;

/// Web-form client for the public NTES query endpoint.
#[derive(Clone)]
pub struct NtesWebClient {
    http: HttpClient,
    base_url: String,
}

impl NtesWebClient {
    /// Build a client rooted at `base_url` (e.g.
    /// `https://enquiry.indianrail.gov.in/mntes` or the configured
    /// `ntes_base`). A trailing slash is stripped.
    pub fn new(http: &HttpClient, base_url: &str) -> Self {
        Self {
            http: http.clone(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// Fetch the current CSRF token guarding the `/q` forms. Reserved for
    /// callers that need to replay the token-bearing form fields.
    #[allow(dead_code)]
    async fn csrf_token(&self) -> Result<String, AppError> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AppError::internal(format!("ntes: system clock before unix epoch: {e}")))?
            .as_millis();
        let url = format!("{}/ntesApi/GetCSRFToken?t={millis}", self.base_url);
        let res =
            self.http.get(&url).await.map_err(|e| {
                AppError::source_unavailable("ntes", format!("request failed: {e}"))
            })?;
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable("ntes", format!("read CSRF token response: {e}"))
        })?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if text.trim().is_empty() {
            return Err(AppError::source_unavailable("ntes", "empty response"));
        }
        extract_csrf_token(&text).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                format!("no token in CSRF response: {}", body_snippet(&text)),
            )
        })
    }

    /// Exceptional trains list (`kind` = `cancelled`, `rescheduled`,
    /// `diverted`); `trainNo` stays empty for the full list.
    pub async fn exceptional(&self, kind: &str) -> Result<Value, AppError> {
        self.post_form(&[
            ("opt", "ExcpTrains".to_string()),
            ("subOpt", "show".to_string()),
            ("excpType", kind.to_string()),
            ("trainNo", String::new()),
        ])
        .await
    }

    /// Trains expected at `station_code` within the next `hours` hours.
    pub async fn live_station(&self, station_code: &str, hours: u32) -> Result<Value, AppError> {
        self.post_form(&[
            ("opt", "LiveStation".to_string()),
            ("subOpt", "show".to_string()),
            ("jStation", station_code.to_string()),
            ("nHr", hours.to_string()),
        ])
        .await
    }

    /// Trains running between `from` and `to` stations.
    pub async fn trains_between(&self, from: &str, to: &str) -> Result<Value, AppError> {
        self.post_form(&[
            ("opt", "TrainBtwStn".to_string()),
            ("subOpt", "show".to_string()),
            ("stnfrom", from.to_string()),
            ("stnto", to.to_string()),
        ])
        .await
    }

    async fn post_form(&self, fields: &[(&str, String)]) -> Result<Value, AppError> {
        let url = format!("{}/q", self.base_url);
        let body =
            self.http.post_form(&url, fields).await.map_err(|e| {
                AppError::source_unavailable("ntes", format!("request failed: {e}"))
            })?;
        if body.trim().is_empty() {
            return Err(AppError::source_unavailable("ntes", "empty response"));
        }
        serde_json::from_str(&body).map_err(|e| {
            AppError::source_unavailable(
                "ntes",
                format!("response is not JSON ({e}): {}", body_snippet(&body)),
            )
        })
    }
}

#[allow(dead_code)]
fn extract_csrf_token(body: &str) -> Option<String> {
    for needle in ["value='", "value=\"", "value= '", "value= \""] {
        if let Some(idx) = body.find(needle) {
            let rest = &body[idx + needle.len()..];
            if let Some(end) = rest.find(['"', '\'']) {
                let value = rest[..end].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

fn body_snippet(body: &str) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    let snippet: String = chars.by_ref().take(SNIPPET_CHARS).collect();
    if chars.next().is_some() {
        format!("{snippet}...")
    } else {
        snippet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn http() -> HttpClient {
        HttpClient::new("railway-rs-test", Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn new_joins_and_trims_base_url() {
        let c = NtesWebClient::new(&http(), "https://enquiry.indianrail.gov.in/mntes/");
        assert_eq!(c.base_url, "https://enquiry.indianrail.gov.in/mntes");
        let c = NtesWebClient::new(&http(), "https://enquiry.indianrail.gov.in");
        assert_eq!(c.base_url, "https://enquiry.indianrail.gov.in");
    }

    #[test]
    fn csrf_token_extraction_handles_hidden_input() {
        let body =
            "<input type='hidden' name='-zr1hgfgigick1786620354' value='-1ageb3318ns5329777005'>";
        assert_eq!(extract_csrf_token(body).unwrap(), "-1ageb3318ns5329777005");
        let body =
            "<input type=\"hidden\" name=\"csrfToken\" value=\"dce6e4e056319e36dac78a98842e5432\">";
        assert_eq!(
            extract_csrf_token(body).unwrap(),
            "dce6e4e056319e36dac78a98842e5432"
        );
        assert_eq!(extract_csrf_token("Request Rejected"), None);
    }

    #[tokio::test]
    async fn blocked_endpoints_are_honest_source_unavailable() {
        // localhost port is closed: hermetic, fails fast, no real network
        let c = NtesWebClient::new(&http(), "http://127.0.0.1:1");
        assert!(matches!(
            c.exceptional("cancelled").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.live_station("NDLS", 2).await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.trains_between("NDLS", "BCT").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
    }
}
