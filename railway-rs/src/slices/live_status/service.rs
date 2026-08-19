use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::models::LiveStatusResponse;
use crate::slices::live_status::mapping::{map_response, str_at};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Resolve the live position of a running train.
    ///
    /// `date` is `YYYY-MM-DD` (optional; empty means today). A non-empty date
    /// resolves to the active run (today or the train's `train_start_date`) or
    /// to one of the exact run dates NTES reports in `instances`, whose own
    /// timeline (real arrivals for a past run, "at origin" for an upcoming
    /// one) replaces the active run's. Any other date is rejected on every
    /// source path - a past-day position is never invented.
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
                match select_run_for_date(&norm, date) {
                    Err(msg) => return Err(AppError::not_found(msg)),
                    Ok(selected) => match map_response(&selected) {
                        Ok(resp) => {
                            tracing::info!(
                                %train,
                                %date,
                                source = "NTES",
                                latency_ms = ntes_started.elapsed().as_millis(),
                                "live status resolved from NTES"
                            );
                            state.cache.set(&key, selected);
                            return Ok(resp);
                        }
                        Err(e) => e.message(),
                    },
                }
            }
            Err(e) => e.message(),
        };

        match railyatri_norm(state, train).await {
            Ok(norm) => match select_run_for_date(&norm, date) {
                Err(msg) => Err(AppError::not_found(msg)),
                Ok(selected) => {
                    tracing::warn!(
                        %train,
                        %date,
                        source = "Railyatri",
                        %ntes_failure,
                        "live status resolved from Railyatri after NTES failure"
                    );
                    let resp = map_response(&selected).map_err(|e| {
                        AppError::source_unavailable(
                            "all-sources",
                            format!(
                                "live status for {train} failed: NTES: {ntes_failure} | Railyatri: {}",
                                e.message()
                            ),
                        )
                    })?;
                    state.cache.set(&key, selected);
                    Ok(resp)
                }
            },
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

/// Pick the run a requested `date` refers to. Empty/today/active-start dates
/// keep the active run; an exact NTES run instance (from `instances`, which
/// now carries its own parsed timeline and position fields) replaces the
/// top-level stops and position so a past run shows its real arrivals and an
/// upcoming run shows "at origin". Anything else is rejected - a past-day
/// position is never invented.
fn select_run_for_date(norm: &Value, date: &str) -> Result<Value, String> {
    if date.is_empty() || matches_date(date, norm) {
        return Ok(norm.clone());
    }
    let target = normalize_date(date);
    let instance = norm
        .get("instances")
        .and_then(Value::as_array)
        .and_then(|instances| {
            instances
                .iter()
                .find(|i| normalize_date(&str_at(i, "start_date")) == target)
        });
    let Some(instance) = instance else {
        return Err(
            "Live position is only available for today's run or one of the reported run dates."
                .to_string(),
        );
    };
    let stops = instance.get("stops").and_then(Value::as_array);
    if stops.is_none_or(Vec::is_empty) {
        return Err("That run instance carries no station timeline.".to_string());
    }

    let mut sel = norm.clone();
    sel["stops"] = Value::Array(stops.cloned().unwrap_or_default());
    for (field, into) in [
        ("start_date", "train_start_date"),
        ("at_src", "at_src"),
        ("at_dstn", "at_dstn"),
        ("next_station_code", "next_station_code"),
        ("next_station_name", "next_station_name"),
        ("platform_number", "platform_number"),
    ] {
        if let Some(v) = instance.get(field) {
            sel[into] = v.clone();
        }
    }
    Ok(sel)
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

    let norm = crate::core::railyatri::parse_live_status(&html).map_err(|e| {
        // Preserve the error type (e.g. NotFound for expired/invalid trains)
        match e {
            AppError::NotFound(msg) => AppError::not_found(format!("Railyatri: {msg}")),
            other => AppError::source_unavailable("Railyatri", other.message()),
        }
    })?;
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
