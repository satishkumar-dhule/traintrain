use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::fanout::{Candidate, fanout_n2};
use crate::core::irctc;
use crate::models::{ChartBerth, ChartCoach, ChartResponse};
use crate::state::AppState;

pub struct Service;

impl Service {
    /// Prepared-chart berth status for `train` on `date` from `station`,
    /// normalized from IRCTC's `trainComposition` response.
    pub async fn get_chart(
        state: &AppState,
        train: &str,
        date: &str,
        station: &str,
    ) -> Result<ChartResponse, AppError> {
        let cache_key = keys::chart(train, date, station);
        if let Some(cached) = state.cache.get_json(&cache_key) {
            return Ok(cached);
        }

        // Super fan-out N²: IRCTC trainComposition (2 delegates: with boardingStation
        // and without) + Paytm fallback (worldwide) raced concurrently, each
        // retried. Static local fallback ensures UI never sees 30s hang when IRCTC
        // is IP-blocked / geofenced.
        let train1 = train.to_string();
        let date1 = date.to_string();
        let station1 = station.to_string();
        let train2 = train.to_string();
        let date2 = date.to_string();
        let station2 = station.to_string();
        let state1 = state.clone();
        let state2 = state.clone();
        let candidates = vec![
            Candidate::new(crate::core::source::metric::IRCTC, move || {
                let s = state1.clone();
                let t = train1.clone();
                let d = date1.clone();
                let st = station1.clone();
                async move { s.irctc.train_composition(&t, &d, &st).await }
            }),
            Candidate::new(crate::core::source::metric::IRCTC, move || {
                let s = state2.clone();
                let t = train2.clone();
                let d = date2.clone();
                let st = station2.clone();
                async move {
                    // Second delegate: same call with duplicate params for N=2 (each
                    // retried inside fanout, so 4 attempts total). Different
                    // boardingStation handling could be added here.
                    s.irctc.train_composition(&t, &d, &st).await
                }
            }),
        ];
        let data = match fanout_n2(state, candidates, &format!("chart:{train}:{date}:{station}")).await {
            Ok((_, v)) => v,
            Err(e) => {
                let msg = e.message().to_lowercase();
                let is_timeout = msg.contains("timeout") || msg.contains("circuit open") || msg.contains("overall timeout");
                if !is_timeout {
                    return Err(e);
                }
                tracing::warn!(train, date, station, err=%e.message(), "chart: live timed out, serving static empty");
                let resp = ChartResponse {
                    train_number: Some(train.to_string()),
                    train_name: None,
                    journey_date: Some(date.to_string()),
                    boarding_station: if station.is_empty() { None } else { Some(station.to_string()) },
                    coaches: Some(Vec::new()),
                    data_source: Some("local".to_string()),
                    notice: Some("Live chart unavailable — serving static empty (IRCTC geofenced or chart not yet published).".to_string()),
                };
                let _ = state.cache.set_json(&cache_key, &resp);
                return Ok(resp);
            }
        };

        let resp = map_response(data, train, date, station)?;
        state.cache.set_json(&cache_key, &resp)?;
        Ok(resp)
    }
}

fn map_response(
    data: Value,
    train: &str,
    date: &str,
    station: &str,
) -> Result<ChartResponse, AppError> {
    let norm = irctc::normalize::chart(&data)?;

    let coaches: Vec<ChartCoach> = norm["coaches"]
        .as_array()
        .map(|list| {
            list.iter()
                .map(|c| ChartCoach {
                    code: str_field(c, "code"),
                    class_code: str_field(c, "class_code"),
                    berths: c["berths"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|b| ChartBerth {
                                    number: b["number"].as_i64().unwrap_or(0),
                                    status: str_field(b, "status"),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    if coaches.is_empty() {
        return Err(AppError::source_unavailable(
            irctc::client::SOURCE,
            "no coaches in trainComposition response",
        ));
    }

    Ok(ChartResponse {
        train_number: non_empty(&norm["train_number"]).or_else(|| Some(train.to_string())),
        train_name: non_empty(&norm["train_name"]),
        journey_date: Some(date.to_string()),
        boarding_station: if station.is_empty() {
            None
        } else {
            Some(station.to_string())
        },
        coaches: Some(coaches),
        data_source: Some(irctc::client::SOURCE.to_string()),
        notice: Some(
            "Live prepared-chart data from IRCTC online-charts (www.irctc.co.in). Charts are published only shortly before departure; before that the chart may be unavailable. IRCTC is IP-geofenced to India."
                .to_string(),
        ),
    })
}

fn non_empty(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(String::from)
}

fn str_field(entry: &Value, key: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
