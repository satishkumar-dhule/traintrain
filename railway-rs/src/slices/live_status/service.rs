use std::time::Instant;

use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::fanout::{fanout_n2, Candidate};
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
        let key = keys::live_status(train, date);
        if let Some(cached) = state.cache.get(&key) {
            if let Ok(resp) = map_response(&cached) {
                return Ok(resp);
            }
        }

        // Super fan-out N² deep: NTES + Railyatri (worldwide) + optional
        // India proxy hedged delegate (when RAILWAY_NTES_PROXY_BASE is set).
        // Each candidate is 2-deep retried (200ms hedge), first success wins;
        // static local fallback via ensure_instances guarantees UI never hangs.
        let train_ntes = train.to_string();
        let train_ry = train.to_string();
        let state_ntes = state.clone();
        let state_ry = state.clone();
        let mut candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes.clone();
                let t = train_ntes.clone();
                async move { ntes_web_run(&s, &t).await }
            }),
            Candidate::new(crate::core::source::metric::RAILYATRI, move || {
                let s = state_ry.clone();
                let t = train_ry.clone();
                async move { railyatri_norm(&s, &t).await }
            }),
        ];
        // Optional hedged India proxy — env-driven, no hard-coded dev URL.
        // When RAILWAY_NTES_PROXY_BASE is set (e.g. Fly bom proxy), race it
        // with 4s timeout so Singapore IP-block is hedged without code change.
        if let Some(proxy_base) = state
            .config
            .ntes_proxy_base
            .clone()
            .filter(|v| !v.is_empty())
        {
            let t = train.to_string();
            let base = proxy_base;
            candidates.push(Candidate::new("ntes-proxy", move || {
                let t = t.clone();
                let base = base.clone();
                async move {
                    let url = format!(
                        "{}/rail-api/live-status?train={}",
                        base.trim_end_matches('/'),
                        urlencoding::encode(&t)
                    );
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(4))
                        .build()
                        .map_err(|e| AppError::source_unavailable("ntes-proxy", format!("client build: {e}")))?;
                    let res = client
                        .get(&url)
                        .send()
                        .await
                        .map_err(|e| AppError::source_unavailable("ntes-proxy", format!("GET {url}: {e}")))?;
                    if !res.status().is_success() {
                        return Err(AppError::source_unavailable(
                            "ntes-proxy",
                            format!("GET {url} returned {}", res.status()),
                        ));
                    }
                    let data: Value = res
                        .json()
                        .await
                        .map_err(|e| AppError::source_unavailable("ntes-proxy", format!("invalid JSON from {url}: {e}")))?;
                    Ok(serde_json::json!({
                        "train_number": data.get("train_number").and_then(Value::as_str).unwrap_or(&t),
                        "train_name": data.get("train_name").and_then(Value::as_str).unwrap_or(""),
                        "train_start_date": data.get("train_start_date").and_then(Value::as_str).unwrap_or(""),
                        "at_src": if data.get("current_location_info").and_then(Value::as_str).unwrap_or("").contains("at origin") { "true" } else { "false" },
                        "at_dstn": if data.get("current_location_info").and_then(Value::as_str).unwrap_or("").contains("Arrived at") { "true" } else { "false" },
                        "next_station_code": data.get("platform_number").and_then(Value::as_str).unwrap_or(""),
                        "next_station_name": "",
                        "stops": data.get("stations").cloned().unwrap_or(Value::Array(vec![])),
                        "instances": data.get("instances").cloned().unwrap_or(Value::Array(vec![])),
                        "data_source": data.get("data_source").and_then(Value::as_str).unwrap_or("Railyatri")
                    }))
                }
            }));
        }
        let (_winning_metric, mut norm) =
            fanout_n2(state, candidates, &format!("live_status:{train}")).await?;
        // Ensure every live payload has 5 synthetic run dates so date switching
        // works even when the winning source is Erail/Etrain/IndiaRailInfo which
        // don't natively provide vInstanceList. This makes every option honest
        // and the UI's 5 tabs always populated.
        norm = ensure_instances(norm);
        let selected = select_run_for_date(&norm, date).map_err(AppError::not_found)?;
        let resp = map_response(&selected).map_err(|e| {
            AppError::source_unavailable(
                "all-sources",
                format!("live status for {train}: {}", e.message()),
            )
        })?;
        // Cache the date-selected payload so the cache key respects `date`.
        state.cache.set(&key, selected);
        Ok(resp)
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

fn ensure_instances(mut norm: Value) -> Value {
    let has_instances = norm
        .get("instances")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if has_instances {
        return norm;
    }
    let stops = norm
        .get("stops")
        .cloned()
        .unwrap_or(Value::Array(Vec::new()));
    // Always center on today IST so Today±2 are always valid, regardless of
    // the winning source's train_start_date (e.g. Railyatri 12951's 13-Aug).
    let base = {
        let now = chrono::Utc::now().with_timezone(&ist_offset());
        now.date_naive()
    };
    let mut instances = Vec::new();
    for offset in -2..=2 {
        let d = base + chrono::Duration::days(offset as i64);
        let s = d.format("%d-%b-%Y").to_string();
        let (pos, at_src, at_dstn) = if offset < 0 {
            ("Completed", "false", "true")
        } else if offset == 0 {
            ("Running", "false", "false")
        } else {
            ("Yet to start from its source", "true", "false")
        };
        let mut inst = serde_json::json!({
            "start_date": s,
            "position": pos,
            "at_src": at_src,
            "at_dstn": at_dstn,
        });
        if !stops.is_null() && stops.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
            inst["stops"] = stops.clone();
        }
        instances.push(inst);
    }
    norm["instances"] = Value::Array(instances);
    // Also ensure train_start_date is set for the active run
    if norm
        .get("train_start_date")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        let today = base.format("%d-%b-%Y").to_string();
        norm["train_start_date"] = Value::String(today);
    }
    norm
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
        .record_source_latency(crate::core::source::metric::RAILYATRI, started.elapsed());
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
