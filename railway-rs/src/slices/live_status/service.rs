use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::models::LiveStatusResponse;
use crate::slices::live_status::mapping::map_response;
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Resolve the live position of a running train.
    ///
    /// `date` is `YYYY-MM-DD` (optional; empty means today). A non-empty date
    /// that is neither today (IST) nor the train's `train_start_date` is
    /// rejected on every source path - a past-day position is never invented.
    ///
    /// NTES (`enquiry.indianrail.gov.in`) is the primary source; Railyatri's
    /// SSR page is the fallback. The winning source is reported in
    /// `data_source`, and the full list of run dates NTES reports for the
    /// train is surfaced in `instances` (like NTES "Spot Train (Live Status)").
    pub async fn get_live_status(
        state: &AppState,
        train: &str,
        date: &str,
    ) -> Result<LiveStatusResponse, AppError> {
        let key = format!("live_status:{train}:{date}");
        if let Some(cached) = state.cache.get(&key) {
            if let Ok(resp) = map_response(&cached) {
                return Ok(resp);
            }
        }

        // NTES primary (proven path): the spot-train web form returns one run
        // instance per tab with a station-by-station live timeline, so the
        // whole position resolves in a single round trip. A best-effort
        // failure still falls back to Railyatri, matching prior behaviour.
        let ntes_started = Instant::now();
        let ntes_failure = match ntes_web_run(state, train).await {
            Ok(norm) => {
                state
                    .metrics
                    .record_source_latency("ntes", ntes_started.elapsed());
                if !date.is_empty() && !matches_date(date, &norm) {
                    return Err(AppError::not_found(
                        "Live position is only available for today's run.",
                    ));
                }
                match map_response(&norm) {
                    Ok(resp) => {
                        tracing::info!(
                            %train,
                            source = "NTES",
                            latency_ms = ntes_started.elapsed().as_millis(),
                            "live status resolved from NTES"
                        );
                        state.cache.set(&key, norm);
                        return Ok(resp);
                    }
                    Err(e) => e.message(),
                }
            }
            Err(e) => e.message(),
        };

        match railyatri_norm(state, train).await {
            Ok(norm) => {
                if !date.is_empty() && !matches_date(date, &norm) {
                    return Err(AppError::not_found(
                        "Live position is only available for today's run.",
                    ));
                }
                tracing::warn!(
                    %train,
                    source = "Railyatri",
                    %ntes_failure,
                    "live status resolved from Railyatri after NTES failure"
                );
                let resp = map_response(&norm).map_err(|e| {
                    AppError::source_unavailable(
                        "all-sources",
                        format!(
                            "live status for {train} failed: NTES: {ntes_failure} | Railyatri: {}",
                            e.message()
                        ),
                    )
                })?;
                state.cache.set(&key, norm);
                Ok(resp)
            }
            Err(AppError::NotFound(msg)) => Err(AppError::not_found(msg)),
            Err(ry_err) => Err(AppError::source_unavailable(
                "all-sources",
                format!(
                    "live status for {train} failed: NTES: {ntes_failure} | Railyatri: {}",
                    ry_err.message()
                ),
            )),
        }
    }
}

/// NTES fetch via the spot-train web form (`FindRunningInstancePop`): the
/// active run tab carries the live timeline plus every reported run date.
async fn ntes_web_run(state: &AppState, train: &str) -> Result<Value, AppError> {
    state.ntes_web.train_status(train).await
}

/// Fetch and normalize the Railyatri SSR live-status page.
async fn railyatri_norm(state: &AppState, train: &str) -> Result<Value, AppError> {
    let url = state.config.source_url(
        &state.config.railyatri_base,
        &format!("/live-train-status/{train}"),
    );
    let started = Instant::now();
    let res = state
        .http
        .inner()
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::source_unavailable("Railyatri", format!("GET {url}: {e}")))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::not_found(format!("Train {train} not found.")));
    }
    if !status.is_success() {
        return Err(AppError::source_unavailable(
            "Railyatri",
            format!("GET {url} returned {status}"),
        ));
    }
    let html = res.text().await.map_err(|e| {
        AppError::source_unavailable("Railyatri", format!("read body of {url}: {e}"))
    })?;

    let norm = crate::core::railyatri::parse_live_status(&html)
        .map_err(|e| AppError::source_unavailable("Railyatri", e.message()))?;
    state
        .metrics
        .record_source_latency("railyatri", started.elapsed());
    Ok(norm)
}

/// `date` matches when it equals today (IST, Indian Railways time) or the
/// train's `train_start_date`. Both sides are normalized to `YYYY-MM-DD` so
/// NTES `DD-MMM-YYYY` / `YYYYMMDD` spellings compare correctly.
fn matches_date(date: &str, norm: &Value) -> bool {
    let now = chrono::Utc::now().with_timezone(&ist_offset());
    let today = now.format("%Y-%m-%d").to_string();
    let date = normalize_date(date);
    if date == today {
        return true;
    }
    match norm.get("train_start_date").and_then(Value::as_str) {
        Some(start) => date == normalize_date(start),
        None => false,
    }
}

/// Normalize a date to `YYYY-MM-DD`, accepting the NTES `DD-MMM-YYYY` and
/// `YYYYMMDD` spellings; anything unrecognized is passed through untouched.
fn normalize_date(s: &str) -> String {
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%d-%b-%Y") {
        return naive.format("%Y-%m-%d").to_string();
    }
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y%m%d") {
            return naive.format("%Y-%m-%d").to_string();
        }
    }
    s.to_string()
}

/// Indian Standard Time offset (UTC+05:30) - the railway day Indian Railways
/// quotes train start dates in.
fn ist_offset() -> chrono::FixedOffset {
    chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or_else(|| {
        // Never happens in practice; keeps the date math total.
        chrono::FixedOffset::east_opt(0).unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_date_accepts_ntes_spellings() {
        assert_eq!(normalize_date("02-May-2026"), "2026-05-02");
        assert_eq!(normalize_date("20260502"), "2026-05-02");
        assert_eq!(normalize_date("2026-05-02"), "2026-05-02");
        assert_eq!(normalize_date("garbage"), "garbage");
    }
}
