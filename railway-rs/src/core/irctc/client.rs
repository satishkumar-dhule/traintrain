//! No-login IRCTC client (www.irctc.co.in).
//!
//! Endpoint protocol (reverse-engineered from the mobile booking app, which
//! works without a login):
//!
//! - `GET  /` harvests the Akamai `TS018d84e5=...` cookie from `Set-Cookie`.
//! - `POST /eticketing/protected/mapps1/altAvlEnq/TC` returns trains with
//!   availability between two stations on a date (response key
//!   `trainBtwnStnsList`).
//! - `POST /online-charts/api/trainComposition` returns the prepared-chart
//!   berth status for one train/date/boarding-station.
//!
//! Every call is signed with `Greq` (epoch ms), the originating page `Referer`
//! and `Origin`. The Akamai bootstrap is best-effort: a blocked/missing root
//! page just means no cookie is sent, and the API call then fails honestly
//! with its real status.
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use reqwest::header::{COOKIE, ORIGIN, REFERER, SET_COOKIE};
use reqwest::RequestBuilder;
use serde_json::{json, Value};

use super::normalize;
use crate::core::error::AppError;
use crate::core::http::HttpClient;

/// Human label used in errors and metrics (matches the source-status UI).
pub const SOURCE: &str = "IRCTC";

const TRAIN_SEARCH_REFERER: &str = "/nget/train-search";
const ONLINE_CHARTS_REFERER: &str = "/online-charts";

/// Cookies harvested from the Akamai bootstrap (`TS018d84e5`, app cookies).
/// Shared via `Arc<Mutex<..>>` so the `Clone` client still carries one session.
#[derive(Clone)]
pub struct IrctcClient {
    http: HttpClient,
    base: String,
    cookies: Arc<Mutex<Vec<(String, String)>>>,
}

impl IrctcClient {
    pub fn new(http: &HttpClient, base: &str) -> Self {
        Self {
            http: http.clone(),
            base: base.trim_end_matches('/').to_string(),
            cookies: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Trains with availability between `src` and `dst` on `date`
    /// (`YYYY-MM-DD`, `DD-MM-YYYY` or `YYYYMMDD`; normalized to `YYYYMMDD`
    /// for the API). General quota `GN`, all classes.
    pub async fn availability(&self, src: &str, dst: &str, date: &str) -> Result<Value, AppError> {
        let body = json!({
            "concessionBooking": false,
            "srcStn": src,
            "destStn": dst,
            "jrnyClass": "",
            "jrnyDate": normalize::date_compact(date),
            "quotaCode": "GN",
            "currentBooking": "false",
            "flexiFlag": false,
            "handicapFlag": false,
            "ticketType": "E",
            "loyaltyRedemptionBooking": false,
            "ftBooking": false
        });
        self.signed_post(
            "/eticketing/protected/mapps1/altAvlEnq/TC",
            &body,
            TRAIN_SEARCH_REFERER,
        )
        .await
    }

    /// Prepared-chart berth status for `train` on `date` from `station`.
    /// `station` is the boarding-station code the online-charts UI passes;
    /// an empty string is forwarded verbatim and rejected by IRCTC.
    pub async fn train_composition(
        &self,
        train: &str,
        date: &str,
        station: &str,
    ) -> Result<Value, AppError> {
        let body = json!({
            "trainNo": train,
            "jDate": normalize::date_iso(date),
            "boardingStation": station
        });
        self.signed_post(
            "/online-charts/api/trainComposition",
            &body,
            ONLINE_CHARTS_REFERER,
        )
        .await
    }

    async fn signed_post(
        &self,
        path: &str,
        body: &Value,
        referer_path: &str,
    ) -> Result<Value, AppError> {
        self.ensure_session().await;
        let url = format!("{}{}", self.base, path);
        let req = self.signed(self.http.inner().post(&url).json(body), referer_path);
        let res = req
            .send()
            .await
            .map_err(|e| AppError::source_unavailable(SOURCE, format!("POST {url}: {e}")))?;
        let status = res.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(AppError::source_unavailable(
                SOURCE,
                format!(
                    "POST {url} returned {status} (IRCTC is Akamai-protected and IP-geofenced; run from an Indian residential IP)"
                ),
            ));
        }
        if !status.is_success() {
            return Err(AppError::source_unavailable(
                SOURCE,
                format!("POST {url} returned {status}"),
            ));
        }
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable(SOURCE, format!("read body of {url}: {e}"))
        })?;
        if bytes.is_empty() {
            return Err(AppError::source_unavailable(
                SOURCE,
                format!("POST {url} returned an empty body"),
            ));
        }
        serde_json::from_slice(&bytes).map_err(|e| {
            AppError::source_unavailable(SOURCE, format!("invalid JSON from {url}: {e}"))
        })
    }

    /// Attach the Akamai signature headers (`Greq`, page `Referer`, `Origin`,
    /// harvested cookies) to a request.
    fn signed(&self, req: RequestBuilder, referer_path: &str) -> RequestBuilder {
        let greq = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let cookie = cookie_str(&self.cookies.lock().unwrap());
        let mut req = req
            .header("Greq", greq.to_string())
            .header(REFERER, format!("{}{}", self.base, referer_path))
            .header(ORIGIN, self.base.clone());
        if !cookie.is_empty() {
            req = req.header(COOKIE, cookie);
        }
        req
    }

    /// Lazy one-time session bootstrap: harvest cookies from the root page and
    /// the train-search page. Best-effort - a 403/404 root just leaves the
    /// cookie jar empty and the API call fails with its real status.
    async fn ensure_session(&self) {
        if !self.cookies.lock().unwrap().is_empty() {
            return;
        }
        if let Ok(res) = self.http.inner().get(&self.base).send().await {
            self.merge_cookies(&res);
        }
        let search_url = format!("{}{}", self.base, TRAIN_SEARCH_REFERER);
        if let Ok(res) = self.http.inner().get(&search_url).send().await {
            self.merge_cookies(&res);
        }
    }

    /// Append `Set-Cookie` headers, replacing any same-named cookie so the
    /// newest value (the live Akamai token) wins.
    fn merge_cookies(&self, res: &reqwest::Response) {
        let mut cookies = self.cookies.lock().unwrap();
        for (name, value) in res
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|s| s.split(';').next())
            .filter_map(|pair| pair.split_once('='))
            .map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))
        {
            merge_cookie(&mut cookies, name, value);
        }
    }
}

/// Insert/replace one cookie in the jar (newest value wins).
fn merge_cookie(jar: &mut Vec<(String, String)>, name: String, value: String) {
    if name.is_empty() {
        return;
    }
    if let Some(existing) = jar.iter_mut().find(|(k, _)| *k == name) {
        existing.1 = value;
    } else {
        jar.push((name, value));
    }
}

fn cookie_str(cookies: &[(String, String)]) -> String {
    cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_jar_replaces_same_name_keeping_order() {
        let mut jar = vec![("TS018d84e5".to_string(), "old".to_string())];
        merge_cookie(&mut jar, "TS018d84e5".to_string(), "new".to_string());
        merge_cookie(&mut jar, "JSESSIONID".to_string(), "s1".to_string());
        merge_cookie(&mut jar, "".to_string(), "ignored".to_string());
        assert_eq!(jar.len(), 2);
        assert_eq!(jar[0], ("TS018d84e5".to_string(), "new".to_string()));
        assert_eq!(jar[1], ("JSESSIONID".to_string(), "s1".to_string()));
    }

    #[test]
    fn signed_headers_include_greq_and_origin() {
        let http = HttpClient::new("railway-rs-test", std::time::Duration::from_secs(5)).unwrap();
        let client = IrctcClient::new(&http, "https://www.irctc.co.in");
        client
            .cookies
            .lock()
            .unwrap()
            .push(("TS018d84e5".to_string(), "abc".to_string()));
        let req = client.signed(
            http.inner().post("https://www.irctc.co.in/x"),
            TRAIN_SEARCH_REFERER,
        );
        let headers = req.build().unwrap().headers().clone();
        assert_eq!(
            headers["referer"],
            "https://www.irctc.co.in/nget/train-search"
        );
        assert_eq!(headers["origin"], "https://www.irctc.co.in");
        assert!(headers["greq"].to_str().unwrap().parse::<u128>().is_ok());
        assert_eq!(
            headers["cookie"], "TS018d84e5=abc",
            "harvested Akamai cookie must be echoed"
        );
    }
}
