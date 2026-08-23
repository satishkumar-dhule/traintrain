use std::time::Instant;

use serde_json::Value;

use crate::core::error::AppError;
use crate::core::{irctc, paytm};
use crate::models::{AvailabilityClass, AvailabilityResponse, AvailabilityTrain};
use crate::slices::availability::SourcePref;
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Direct trains with availability between `src` and `dst` on `date`
    /// (`YYYY-MM-DD`). `source` picks the upstream: Paytm (default primary,
    /// per-class status + fares), IRCTC (fallback / explicit), or auto
    /// (Paytm first, IRCTC on failure). `data_source` names whoever answered.
    pub async fn get_availability(
        state: &AppState,
        src: &str,
        dst: &str,
        date: &str,
        source: SourcePref,
    ) -> Result<AvailabilityResponse, AppError> {
        let cache_key = format!(
            "availability:{src}:{dst}:{date}:{}",
            match source {
                SourcePref::Auto => "auto",
                SourcePref::PaytmOnly => "paytm",
                SourcePref::IrctcOnly => "irctc",
            }
        );
        if let Some(cached) = state.cache.get(&cache_key) {
            if let Ok(resp) = serde_json::from_value(cached) {
                return Ok(resp);
            }
        }

        let (primary, fallback) = match source {
            SourcePref::Auto => ("paytm", Some("irctc")),
            SourcePref::PaytmOnly => ("paytm", None),
            SourcePref::IrctcOnly => ("irctc", None),
        };

        let primary_err = match fetch_from(state, primary, src, dst, date).await {
            Ok(resp) => {
                state.cache.set(&cache_key, serde_json::to_value(&resp)?);
                return Ok(resp);
            }
            Err(e) => e,
        };

        if let Some(fallback) = fallback {
            // A definitive "no direct trains" answer settles the question —
            // the fallback would only repeat it. Return the clean message
            // instead of merging outage details from both sources.
            if matches!(primary_err, AppError::NotFound(_)) {
                return Err(primary_err);
            }
            tracing::warn!(
                %src,
                %dst,
                source = primary,
                error = %primary_err,
                "primary availability source failed; trying fallback"
            );
            match fetch_from(state, fallback, src, dst, date).await {
                Ok(resp) => {
                    state.cache.set(&cache_key, serde_json::to_value(&resp)?);
                    return Ok(resp);
                }
                Err(e) => {
                    return Err(AppError::source_unavailable(
                        format!("{primary} + {fallback}"),
                        format!("{}; {}", primary_err.message(), e.message()),
                    ));
                }
            }
        }

        Err(primary_err)
    }
}

/// Fetch from one named source (`"paytm"` / `"irctc"`) and normalize to the
/// shared DTO. Latency is recorded under the source's lowercase key so the
/// observability source-status panel sees it.
async fn fetch_from(
    state: &AppState,
    source: &str,
    src: &str,
    dst: &str,
    date: &str,
) -> Result<AvailabilityResponse, AppError> {
    let start = Instant::now();
    let data = match source {
        "paytm" => state.paytm.search(src, dst, date).await?,
        "irctc" => state.irctc.availability(src, dst, date).await?,
        other => return Err(AppError::internal(format!("unknown source {other}"))),
    };
    state.metrics.record_source_latency(source, start.elapsed());
    map_response(source, data, src, dst, date)
}

fn map_response(
    source: &str,
    data: Value,
    src: &str,
    dst: &str,
    date: &str,
) -> Result<AvailabilityResponse, AppError> {
    let norm = match source {
        "paytm" => paytm::normalize::availability_trains(&data)?,
        _ => irctc::normalize::availability_trains(&data)?,
    };
    let trains: Vec<AvailabilityTrain> = norm["trains"]
        .as_array()
        .map(|list| list.iter().map(map_train).collect())
        .unwrap_or_default();

    if trains.is_empty() {
        return Err(AppError::source_unavailable(
            source_label(source),
            "no trains with availability in response",
        ));
    }

    let (label, notice) = match source {
        "paytm" => (
            paytm::client::SOURCE,
            "Live availability from Paytm Travel (travel.paytm.com), reflecting IRCTC booking status with class-wise waitlist and fare details.",
        ),
        _ => (
            irctc::client::SOURCE,
            "Live availability from IRCTC (www.irctc.co.in), the official Indian Railways booking portal. IRCTC is IP-geofenced to India.",
        ),
    };

    Ok(AvailabilityResponse {
        src: Some(src.to_string()),
        dst: Some(dst.to_string()),
        date: Some(date.to_string()),
        trains: Some(trains),
        data_source: Some(label.to_string()),
        notice: Some(notice.to_string()),
    })
}

fn map_train(t: &Value) -> AvailabilityTrain {
    AvailabilityTrain {
        number: str_field(t, "number"),
        name: str_field(t, "name"),
        from_code: str_field(t, "from_code"),
        from_name: str_field(t, "from_name"),
        to_code: str_field(t, "to_code"),
        to_name: str_field(t, "to_name"),
        departure_time: str_field(t, "departure_time"),
        arrival_time: str_field(t, "arrival_time"),
        duration: str_field(t, "duration"),
        distance: str_field(t, "distance"),
        classes: t["classes"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default(),
        train_type: str_field(t, "train_type"),
        runs_on: t["runs_on"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_bool).collect())
            .unwrap_or_default(),
        availability: t["availability"]
            .as_array()
            .map(|a| a.iter().map(map_class).collect())
            .unwrap_or_default(),
    }
}

fn map_class(a: &Value) -> AvailabilityClass {
    AvailabilityClass {
        class: str_field(a, "class"),
        class_name: str_field(a, "class_name"),
        status: str_field(a, "status"),
        available: a.get("available").and_then(Value::as_bool),
        fare: a.get("fare").and_then(Value::as_i64),
        quota: a.get("quota").and_then(Value::as_str).map(String::from),
        prediction: a.get("prediction").and_then(Value::as_i64),
    }
}

fn source_label(source: &str) -> String {
    match source {
        "paytm" => paytm::client::SOURCE.to_string(),
        _ => irctc::client::SOURCE.to_string(),
    }
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
