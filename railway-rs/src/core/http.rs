use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, REFERER, USER_AGENT};
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
    user_agent: String,
}

impl HttpClient {
    pub fn new(user_agent: impl Into<String>, timeout: Duration) -> Result<Self, AppError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&user_agent.into())
                .unwrap_or_else(|_| HeaderValue::from_static("railway-rs")),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://enquiry.indianrail.gov.in/"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(8))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent("railway-rs")
            .build()
            .map_err(|e| AppError::internal(format!("failed to build http client: {e}")))?;

        Ok(Self {
            inner: Arc::new(client),
            user_agent: "".into(),
        })
    }

    pub fn inner(&self) -> &reqwest::Client {
        &self.inner
    }

    /// GET with one retry on transport errors. Returns the raw response.
    pub async fn get(&self, url: &str) -> Result<reqwest::Response, AppError> {
        let mut last: Option<AppError> = None;
        for attempt in 0..2 {
            let res = self.inner.get(url).send().await;
            match res {
                Ok(r) => {
                    if r.status().is_success() {
                        return Ok(r);
                    }
                    last = Some(AppError::source_unavailable(
                        "HTTP",
                        format!("GET {url} returned {}", r.status()),
                    ));
                }
                Err(e) => {
                    last = Some(AppError::source_unavailable(
                        "HTTP",
                        format!("GET {url}: {e}"),
                    ));
                }
            }
            if attempt == 0 {
                tokio::time::sleep(Duration::from_millis(400)).await;
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

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
}
