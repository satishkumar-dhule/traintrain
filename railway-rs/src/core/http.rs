use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde_json::Value;

use super::error::AppError;

/// Thin wrapper around a shared `reqwest::Client` with a realistic browser
/// user-agent, sane timeouts, gzip decoding and one retry for idempotent GETs.
///
/// All outbound HTTP in the project goes through this type so that retry /
/// timeout / UA behaviour is defined in exactly one place (DRY).
#[derive(Clone)]
pub struct HttpClient {
    inner: Arc<reqwest::Client>,
}

impl HttpClient {
    pub fn new(user_agent: impl Into<String>, timeout: Duration) -> Result<Self, AppError> {
        // Single source of truth for UA: the configured browser UA, not the
        // hard-coded "railway-rs" override (review 2.3). Content-Type and
        // Referer are per-request, not defaults — GEFs must not leak
        // enquiry.indianrail.gov.in as Referer to Paytm/Ixigo.
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&user_agent.into())
                .unwrap_or_else(|_| HeaderValue::from_static("railway-rs")),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(8))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| AppError::internal(format!("failed to build http client: {e}")))?;

        Ok(Self {
            inner: Arc::new(client),
        })
    }

    pub fn inner(&self) -> &reqwest::Client {
        &self.inner
    }

    /// GET with jittered retry — only on transport errors or 5xx, never on 4xx.
    /// KISS: 2 attempts, 200ms + up to 100ms jitter (avoids thundering retry).
    pub async fn get(&self, url: &str) -> Result<reqwest::Response, AppError> {
        let mut last: Option<AppError> = None;
        for attempt in 0..2 {
            let res = self.inner.get(url).send().await;
            match res {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        return Ok(r);
                    }
                    if status.is_client_error() {
                        // 4xx: honest NotFound/BadRequest — don't retry, don't jitter
                        return Err(AppError::source_unavailable(
                            "HTTP",
                            format!("GET {url} returned {}", status),
                        ));
                    }
                    last = Some(AppError::source_unavailable(
                        "HTTP",
                        format!("GET {url} returned {}", status),
                    ));
                }
                Err(e) => {
                    // Transport error — retryable
                    last = Some(AppError::source_unavailable(
                        "HTTP",
                        format!("GET {url}: {e}"),
                    ));
                }
            }
            if attempt == 0 {
                // Jittered backoff: 200ms + hash(url) % 100ms (deterministic per URL, KISS)
                let jitter = {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut h = DefaultHasher::new();
                    url.hash(&mut h);
                    h.finish() % 100
                };
                tokio::time::sleep(Duration::from_millis(200 + jitter)).await;
            }
        }
        Err(last.unwrap_or_else(|| AppError::internal("GET failed")))
    }

    /// GET and return the body as text.
    pub async fn get_text(&self, url: &str) -> Result<String, AppError> {
        let res = self.get(url).await?;
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable("HTTP", format!("read body of {url}: {e}"))
        })?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// GET and return the body as JSON.
    pub async fn get_json(&self, url: &str) -> Result<Value, AppError> {
        let res = self.get(url).await?;
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable("HTTP", format!("read body of {url}: {e}"))
        })?;
        serde_json::from_slice(&bytes).map_err(|e| {
            AppError::source_unavailable("HTTP", format!("invalid JSON from {url}: {e}"))
        })
    }

    /// POST JSON and return the parsed JSON body.
    pub async fn post_json(&self, url: &str, body: &Value) -> Result<Value, AppError> {
        let res = self
            .inner
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| AppError::source_unavailable("HTTP", format!("POST {url}: {e}")))?;
        let status = res.status();
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable("HTTP", format!("read body of {url}: {e}"))
        })?;
        if bytes.is_empty() {
            return Err(AppError::source_unavailable(
                "HTTP",
                format!("POST {url} returned an empty response (status {status})"),
            ));
        }
        serde_json::from_slice(&bytes).map_err(|e| {
            AppError::source_unavailable("HTTP", format!("invalid JSON from {url}: {e}"))
        })
    }

    /// POST `application/x-www-form-urlencoded` and return the raw text body.
    pub async fn post_form(
        &self,
        url: &str,
        fields: &[(&str, String)],
    ) -> Result<String, AppError> {
        let form = fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<Vec<_>>();
        let res =
            self.inner.post(url).form(&form).send().await.map_err(|e| {
                AppError::source_unavailable("HTTP", format!("POST form {url}: {e}"))
            })?;
        let status = res.status();
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable("HTTP", format!("read body of {url}: {e}"))
        })?;
        if !status.is_success() {
            return Err(AppError::source_unavailable(
                "HTTP",
                format!("POST form {url} returned {status}"),
            ));
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}
