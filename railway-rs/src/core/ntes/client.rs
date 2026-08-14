//! NTES (enquiry.indianrail.gov.in) mobile API client.
//!
//! The sub-service payloads below match the official Android app protocol.
//! Responses arrive encrypted; `NtesClient` transparently decrypts and returns
//! the plain JSON for the owning vertical slice to normalise.
//!
//! NOTE: from the sandbox the endpoint answers with an empty body, so every
//! call may surface `AppError::SourceUnavailable`. Callers must propagate that
//! honestly - never fabricate train data.

use serde_json::Value;

use super::super::error::AppError;
use super::super::http::HttpClient;
use super::crypto::NtesCrypto;

const SERVICE: &str = "TrainRunningMob";

#[derive(Clone)]
pub struct NtesClient {
    http: HttpClient,
    endpoint: String,
}

impl NtesClient {
    pub fn new(http: &HttpClient, base_url: &str) -> Self {
        Self {
            http: http.clone(),
            endpoint: format!("{}/crisns/AppServAnd", base_url.trim_end_matches('/')),
        }
    }

    async fn request(&self, sub_service: &str, params: &[(&str, &str)]) -> Result<Value, AppError> {
        let mut payload = format!("service={SERVICE}&subService={sub_service}");
        for (k, v) in params {
            payload.push_str(&format!("&{k}={v}"));
        }
        let body = serde_json::json!({ "jsonIn": NtesCrypto::build(&payload) });
        let resp = self
            .http
            .post_json(&self.endpoint, &body)
            .await
            .map_err(|e| AppError::source_unavailable("ntes", format!("request failed: {e}")))?;
        let enc = resp
            .get("jsonIn")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::source_unavailable("ntes", "response missing jsonIn"))?;
        NtesCrypto::decode_json(enc).map_err(|e| {
            AppError::source_unavailable("ntes", format!("ciphertext could not be decoded: {e}"))
        })
    }

    /// `FindTrainJson` - exact train number lookup.
    pub async fn search_train(&self, query: &str) -> Result<Value, AppError> {
        self.request("FindTrainJson", &[("trainNo", query)]).await
    }

    /// `GetTrainInstance` - train identity (number, name, type).
    pub async fn train_info(&self, train_no: &str) -> Result<Value, AppError> {
        self.request("GetTrainInstance", &[("trainNo", train_no)])
            .await
    }

    /// `GetTrainSchedule` - full scheduled route. `start_date` optional (`YYYYMMDD`).
    pub async fn schedule(&self, train_no: &str, start_date: &str) -> Result<Value, AppError> {
        self.request(
            "GetTrainSchedule",
            &[("trainNo", train_no), ("startDate", start_date)],
        )
        .await
    }

    /// `ShowFullRunJson` - live position of a running train for a start date.
    pub async fn live_status(&self, train_no: &str, start_date: &str) -> Result<Value, AppError> {
        self.request(
            "ShowFullRunJson",
            &[("trainNo", train_no), ("startDate", start_date)],
        )
        .await
    }

    /// `TrainsAtStationJson` - trains expected at a station within `hours`.
    pub async fn station_live(&self, station_code: &str, hours: u32) -> Result<Value, AppError> {
        self.request(
            "TrainsAtStationJson",
            &[
                ("jStation", station_code),
                ("nHr", &hours.to_string()),
                ("jToStation", ""),
            ],
        )
        .await
    }

    /// `TrainBtwStnJson` - trains between two stations.
    pub async fn trains_between(
        &self,
        from: &str,
        to: &str,
        train_type: &str,
    ) -> Result<Value, AppError> {
        self.request(
            "TrainBtwStnJson",
            &[("stnFrom", from), ("stnTo", to), ("trainType", train_type)],
        )
        .await
    }

    /// `TrainExcpInfo` - exceptional running info for a single train.
    pub async fn train_exceptions(&self, train_no: &str) -> Result<Value, AppError> {
        self.request("TrainExcpInfo", &[("trainNo", train_no)])
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn client_builds_endpoint_from_base() {
        let http = HttpClient::new("railway-rs-test", Duration::from_secs(5)).unwrap();
        let c = NtesClient::new(&http, "https://example.in");
        assert_eq!(c.endpoint, "https://example.in/crisns/AppServAnd");
    }

    #[tokio::test]
    async fn unreachable_base_is_honest_source_unavailable() {
        let http = HttpClient::new("railway-rs-test", Duration::from_secs(5)).unwrap();
        // localhost port is closed: hermetic, fails fast, no real network
        let c = NtesClient::new(&http, "http://127.0.0.1:1");
        let res = c.schedule("12951", "").await;
        assert!(matches!(
            res,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
    }
}
