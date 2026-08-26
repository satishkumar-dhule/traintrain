use std::time::Instant;

use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::irctc;
use crate::models::{BetweenTrain, TrainsBetweenResponse};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Direct trains between two station codes.
    ///
    /// NTES (`TrainBtwStnJson`) is the primary source; IRCTC's no-login
    /// availability API (`altAvlEnq/TC`) is the fallback when NTES is
    /// unreachable or malformed. The winning source is reported honestly in
    /// `data_source`. The final DTO (not the raw upstream payload) is cached,
    /// so a later hit works regardless of which source produced it.
    pub async fn get_trains_between(
        state: &AppState,
        src: &str,
        dst: &str,
    ) -> Result<TrainsBetweenResponse, AppError> {
        let cache_key = keys::trains_between(src, dst);
        if let Some(cached) = state.cache.get_json(&cache_key) {
            return Ok(cached);
        }

        let ntes_started = Instant::now();
        let from_name = state.datasets.station_name(src).unwrap_or(src).to_string();
        let to_name = state.datasets.station_name(dst).unwrap_or(dst).to_string();
        let ntes_failure = if state
            .failover
            .should_skip(crate::core::source::metric::NTES)
        {
            tracing::warn!(%src, %dst, source = "NTES", "circuit open — flip-flop skipped NTES");
            "circuit open (cooldown)".to_string()
        } else {
            match state
                .ntes_web
                .trains_between(src, &from_name, dst, &to_name)
                .await
            {
                Ok(data) => {
                    state.metrics.record_source_latency(
                        crate::core::source::metric::NTES,
                        ntes_started.elapsed(),
                    );
                    state
                        .failover
                        .record_success(crate::core::source::metric::NTES);
                    match map_ntes(data, src, dst) {
                        Ok(resp) => {
                            tracing::info!(
                                %src,
                                %dst,
                                source = "NTES",
                                latency_ms = ntes_started.elapsed().as_millis(),
                                "trains-between resolved from NTES"
                            );
                            state.cache.set_json(&cache_key, &resp)?;
                            return Ok(resp);
                        }
                        Err(e) => e.message(),
                    }
                }
                Err(e) => {
                    let msg = e.message();
                    if matches!(
                        e,
                        AppError::SourceUnavailable { .. } | AppError::Internal(_)
                    ) {
                        state
                            .failover
                            .record_failure(crate::core::source::metric::NTES);
                    }
                    msg
                }
            }
        };

        match irctc_fallback(state, src, dst, &ntes_failure).await {
            Ok(resp) => {
                state.cache.set_json(&cache_key, &resp)?;
                Ok(resp)
            }
            Err(e) => Err(AppError::source_unavailable(
                "all-sources",
                format!(
                    "trains between {src} and {dst} failed: NTES: {ntes_failure} | IRCTC: {}",
                    e.message()
                ),
            )),
        }
    }
}

/// IRCTC fallback: direct trains with availability from the no-login
/// `altAvlEnq/TC` API for today's date (IST), normalized to the same
/// `BetweenTrain` shape as the NTES payload.
async fn irctc_fallback(
    state: &AppState,
    src: &str,
    dst: &str,
    ntes_failure: &str,
) -> Result<TrainsBetweenResponse, AppError> {
    if state
        .failover
        .should_skip(crate::core::source::metric::IRCTC)
    {
        return Err(AppError::source_unavailable(
            crate::core::source::labels::IRCTC,
            "circuit open (cooldown)",
        ));
    }
    let start = Instant::now();
    let today = today_ist();
    let data = state
        .irctc
        .availability(src, dst, &today)
        .await
        .map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state
                    .failover
                    .record_failure(crate::core::source::metric::IRCTC);
            }
            e
        })?;
    state
        .metrics
        .record_source_latency(crate::core::source::metric::IRCTC, start.elapsed());
    state
        .failover
        .record_success(crate::core::source::metric::IRCTC);

    let norm = irctc::normalize::availability_trains(&data)?;
    let trains: Vec<BetweenTrain> = norm["trains"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|t| BetweenTrain {
                    number: str_field(t, "number"),
                    name: str_field(t, "name"),
                    departure_time: str_field(t, "departure_time"),
                    arrival_time: str_field(t, "arrival_time"),
                    runs_on: t["runs_on"]
                        .as_array()
                        .map(|a| a.iter().filter_map(Value::as_bool).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    tracing::warn!(
        %src,
        %dst,
        source = "IRCTC",
        %ntes_failure,
        latency_ms = start.elapsed().as_millis(),
        "trains-between resolved from IRCTC after NTES failure"
    );

    Ok(TrainsBetweenResponse {
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        trains: Some(trains),
        data_source: Some(irctc::client::SOURCE.to_string()),
    })
}

/// Today's date in IST (UTC+05:30), which is what IRCTC bookings are quoted in.
fn today_ist() -> String {
    let offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or_else(|| {
        // Fallback to UTC if the offset constant cannot be built (never in practice).
        chrono::FixedOffset::east_opt(0).unwrap()
    });
    chrono::Utc::now()
        .with_timezone(&offset)
        .date_naive()
        .to_string()
}

fn map_ntes(data: Value, src: &str, dst: &str) -> Result<TrainsBetweenResponse, AppError> {
    let list = data
        .get("trainBtwStationList")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .or_else(|| {
            data.get("trainList")
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
        })
        .ok_or_else(|| AppError::internal("NTES: unexpected TrainBtwStnJson shape"))?;

    let trains = list.iter().map(map_train).collect();
    Ok(TrainsBetweenResponse {
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        trains: Some(trains),
        data_source: Some(crate::core::source::labels::NTES.to_string()),
    })
}

fn map_train(entry: &Value) -> BetweenTrain {
    BetweenTrain {
        number: str_field(entry, "trainNo"),
        name: str_field(entry, "trainName"),
        departure_time: str_field(entry, "depTime"),
        arrival_time: str_field(entry, "arrTime"),
        runs_on: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .into_iter()
            .map(|day| day_bool(entry, day))
            .collect(),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Accept both the documented `runOn<Day>` and community `runsOn<Day>` spellings.
fn day_bool(entry: &Value, day: &str) -> bool {
    entry
        .get(format!("runOn{day}"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || entry
            .get(format!("runsOn{day}"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}
