//! HTTP client for Paytm Travel's public train-search API.
//!
//! See module docs in `super` for the wire protocol. The search endpoint is a
//! plain unauthenticated GET; the odd-looking fixed query parameters
//! (`designVersion`, `dimension114`, `isH5`, ...) mirror what the web app
//! sends and are required for the API to answer.
use serde_json::Value;

use crate::core::error::AppError;
use crate::core::http::HttpClient;

/// Human label used in errors and metrics (matches the source-status UI).
pub const SOURCE: &str = "Paytm";

const SEARCH_PATH: &str = "/api/trains/v5/search";

#[derive(Clone)]
pub struct PaytmClient {
    http: HttpClient,
    base: String,
}

impl PaytmClient {
    pub fn new(http: &HttpClient, base: &str) -> Self {
        Self {
            http: http.clone(),
            base: base.trim_end_matches('/').to_string(),
        }
    }

    /// Direct trains with availability between `src` and `dst` on `date`
    /// (`YYYY-MM-DD`, `DD-MM-YYYY` or `YYYYMMDD`; normalized to `YYYYMMDD`
    /// for the API). General quota `GN`.
    pub async fn search(&self, src: &str, dst: &str, date: &str) -> Result<Value, AppError> {
        let url = search_url(&self.base, src, dst, date);
        let res = self
            .http
            .inner()
            .get(&url)
            .header("accept", "application/json")
            .send()
            .await
            .map_err(|e| AppError::source_unavailable(SOURCE, format!("GET {url}: {e}")))?;

        let status = res.status();
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable(SOURCE, format!("read body of {url}: {e}"))
        })?;
        let data: Value = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::source_unavailable(SOURCE, format!("invalid JSON from {url} ({status}): {e}"))
        })?;

        // Upstream signals failures with HTTP 400/4xx plus a JSON envelope
        // (`status.result == "failure"`); surface its real message.
        if !status.is_success() {
            let reason = data
                .pointer("/status/message/message")
                .and_then(Value::as_str)
                .or_else(|| data.get("error").and_then(Value::as_str))
                .unwrap_or("upstream rejected the search");
            return Err(no_trains_or_outage(
                src,
                dst,
                date,
                status.as_u16() == 451,
                &format!("GET {url} returned {status}: {reason}"),
                reason,
            ));
        }
        if data.pointer("/status/result").and_then(Value::as_str) == Some("failure") {
            let reason = data
                .pointer("/status/message/message")
                .and_then(Value::as_str)
                .unwrap_or("search failed");
            return Err(no_trains_or_outage(
                src,
                dst,
                date,
                false,
                &format!("GET {url}: {reason}"),
                reason,
            ));
        }
        Ok(data)
    }
}

/// Paytm answers "no direct trains between these stations" with HTTP 451 /
/// a failure envelope carrying that message. That is a definitive empty
/// result for the route + date — not a source outage — so it maps to
/// `NotFound` with a clean, user-facing message instead of raw upstream
/// URL noise.
fn no_trains_or_outage(
    src: &str,
    dst: &str,
    date: &str,
    definitive_no_trains: bool,
    detail: &str,
    reason: &str,
) -> AppError {
    if definitive_no_trains || reason.to_lowercase().contains("no direct trains") {
        return AppError::NotFound(format!(
            "No direct trains run between {src} and {dst} on {date}. Try a nearby station pair or a different date."
        ));
    }
    tracing::warn!(source = SOURCE, %detail, "paytm search rejected");
    AppError::source_unavailable(SOURCE, detail)
}

/// Full search URL for one query. `source` is the origin station and
/// `destination` the target — mirroring the travel.paytm.com web app.
fn search_url(base: &str, src: &str, dst: &str, date: &str) -> String {
    format!(
        "{}{}?departureDate={}&designVersion=v3&destination={}&dimension114=seo-home&isAscOfferEligible=false&isH5=true&is_new_user=null&quota=GN&show_empty=true&source={}&client=web&deviceIdentifier=Mozilla%20Firefox-151.0.0.0",
        base.trim_end_matches('/'),
        SEARCH_PATH,
        super::normalize::date_compact(date),
        urlencode(dst),
        urlencode(src),
    )
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_leaves_station_codes_untouched() {
        assert_eq!(urlencode("MAO"), "MAO");
        assert_eq!(urlencode("ndls"), "ndls");
        assert_eq!(urlencode("4 XYZ"), "4%20XYZ");
    }

    #[test]
    fn no_direct_trains_message_maps_to_clean_not_found() {
        let e = no_trains_or_outage(
            "HYB",
            "AL",
            "2026-08-29",
            false,
            "GET https://travel.paytm.com/api/trains/v5/search returned 451 Unavailable For Legal Reasons: There are no direct trains running between these two stations for your travel date.",
            "There are no direct trains running between these two stations for your travel date. Please try an alternative route or a different date.",
        );
        assert!(matches!(e, AppError::NotFound(_)));
        let msg = e.message();
        assert!(msg.contains("No direct trains"), "{msg}");
        assert!(
            !msg.contains("http"),
            "no upstream URL noise in user error: {msg}"
        );
    }

    #[test]
    fn http_451_alone_counts_as_definitive_no_trains() {
        let e = no_trains_or_outage(
            "HYB",
            "AL",
            "2026-08-29",
            true,
            "GET … returned 451",
            "nope",
        );
        assert!(matches!(e, AppError::NotFound(_)));
    }

    #[test]
    fn genuine_paytm_rejection_stays_source_unavailable() {
        let e = no_trains_or_outage(
            "MAO",
            "NDLS",
            "2026-10-20",
            false,
            "GET https://travel.paytm.com returned 500 Internal Server Error: boom",
            "boom",
        );
        assert!(matches!(e, AppError::SourceUnavailable { .. }));
    }

    #[test]
    fn search_url_maps_source_and_destination_correctly() {
        let url = search_url("https://travel.paytm.com", "MAO", "NDLS", "2026-10-20");
        assert!(url.starts_with("https://travel.paytm.com/api/trains/v5/search?"));
        assert!(url.contains("departureDate=20261020"));
        assert!(
            url.contains("destination=NDLS"),
            "dst must map to destination: {url}"
        );
        assert!(url.contains("source=MAO"), "src must map to source: {url}");
        assert!(!url.contains("source=NDLS") && !url.contains("destination=MAO"));
    }
}
