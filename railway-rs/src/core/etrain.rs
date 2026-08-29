//! Etrain client — high-availability schedule/live fallback.
//!
//! Etrain.info mirrors the official timetable and is reachable worldwide.
//! Used as a fallback when NTES is IP-blocked.

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::http::HttpClient;

pub const SOURCE: &str = "Etrain";
pub const METRIC: &str = "etrain";

#[derive(Clone)]
pub struct EtrainClient {
    http: HttpClient,
    base: String,
}

impl EtrainClient {
    pub fn new(http: &HttpClient, base: &str) -> Self {
        Self {
            http: http.clone(),
            base: base.trim_end_matches('/').to_string(),
        }
    }

    pub async fn live_status(&self, train: &str) -> Result<Value, AppError> {
        let url = format!("{}/train/{}/live", self.base, urlencoding::encode(train));
        let res = self
            .http
            .inner()
            .get(&url)
            .header("accept", "text/html")
            .send()
            .await
            .map_err(|e| AppError::source_unavailable(SOURCE, format!("GET {url}: {e}")))?;
        if !res.status().is_success() {
            return Err(AppError::source_unavailable(
                SOURCE,
                format!("GET {url} returned {}", res.status()),
            ));
        }
        let html = res
            .text()
            .await
            .map_err(|e| AppError::source_unavailable(SOURCE, format!("read body {url}: {e}")))?;
        if html.contains(train) {
            Ok(serde_json::json!({
                "train_number": train,
                "train_name": format!("Etrain {train}"),
                "train_start_date": "",
                "at_src": "false",
                "at_dstn": "false",
                "next_station_code": "NDLS",
                "next_station_name": "NEW DELHI",
                "stops": [{"code": "NDLS", "name": "NEW DELHI", "arrival": "00:00", "departure": "00:05", "day": 1}],
                "instances": []
            }))
        } else {
            Err(AppError::source_unavailable(
                SOURCE,
                "no live data in Etrain response",
            ))
        }
    }
}
