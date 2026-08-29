use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::AppError;
use crate::core::fanout::{fanout_n2_singleflight, Candidate};
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

        // Super fan-out N²: NTES + IRCTC + Paytm raced concurrently.
        // NTES is primary; IRCTC/Paytm are worldwide-ish booking APIs that
        // answer from any IP (IRCTC geofenced but less aggressively than NTES).
        let from_name = state.datasets.station_name(src).unwrap_or(src).to_string();
        let to_name = state.datasets.station_name(dst).unwrap_or(dst).to_string();
        let src_ntes = src.to_string();
        let dst_ntes = dst.to_string();
        let from_ntes = from_name.clone();
        let to_ntes = to_name.clone();
        let state_ntes = state.clone();

        let src_irctc = src.to_string();
        let dst_irctc = dst.to_string();
        let state_irctc = state.clone();

        let src_paytm = src.to_string();
        let dst_paytm = dst.to_string();
        let state_paytm = state.clone();

        let src_ct = src.to_string();
        let dst_ct = dst.to_string();
        let state_ct = state.clone();
        let src_ix = src.to_string();
        let dst_ix = dst.to_string();
        let state_ix = state.clone();
        let src_er = src.to_string();
        let dst_er = dst.to_string();
        let state_er = state.clone();

        let candidates = vec![
            Candidate::new(crate::core::source::metric::NTES, move || {
                let s = state_ntes.clone();
                let src = src_ntes.clone();
                let dst = dst_ntes.clone();
                let from = from_ntes.clone();
                let to = to_ntes.clone();
                async move { s.ntes_web.trains_between(&src, &from, &dst, &to).await }
            }),
            Candidate::new(crate::core::source::metric::IRCTC, move || {
                let s = state_irctc.clone();
                let src = src_irctc.clone();
                let dst = dst_irctc.clone();
                async move {
                    let today = today_ist();
                    let data = s.irctc.availability(&src, &dst, &today).await?;
                    let norm = irctc::normalize::availability_trains(&data)?;
                    Ok::<Value, AppError>(norm)
                }
            }),
            Candidate::new(crate::core::source::metric::PAYTM, move || {
                let s = state_paytm.clone();
                let src = src_paytm.clone();
                let dst = dst_paytm.clone();
                async move {
                    let today = today_ist();
                    let data = s.paytm.search(&src, &dst, &today).await?;
                    let norm = crate::core::paytm::normalize::availability_trains(&data)?;
                    Ok::<Value, AppError>(norm)
                }
            }),
            Candidate::new(crate::core::source::metric::CONFIRMTKT, move || {
                let s = state_ct.clone();
                let src = src_ct.clone();
                let dst = dst_ct.clone();
                async move {
                    let today = today_ist();
                    let data = s.confirmtkt.availability(&src, &dst, &today).await?;
                    Ok::<Value, AppError>(data)
                }
            }),
            Candidate::new(crate::core::source::metric::IXIGO, move || {
                let s = state_ix.clone();
                let src = src_ix.clone();
                let dst = dst_ix.clone();
                async move {
                    let today = today_ist();
                    let data = s.ixigo.availability(&src, &dst, &today).await?;
                    Ok::<Value, AppError>(data)
                }
            }),
            Candidate::new(crate::core::source::metric::ERAIL, move || {
                let s = state_er.clone();
                let src = src_er.clone();
                let dst = dst_er.clone();
                async move {
                    let data = s.erail.trains_between(&src, &dst).await?;
                    // Erail returns trainBtwStationList, map directly via map_ntes
                    Ok::<Value, AppError>(data)
                }
            }),
        ];

        let (metric, data) =
            fanout_n2_singleflight(state, candidates, &format!("trains_between:{src}:{dst}"))
                .await?;

        let resp = if metric == crate::core::source::metric::NTES
            || metric == crate::core::source::metric::ERAIL
        {
            map_ntes(data, src, dst)?
        } else {
            // IRCTC/Paytm/ConfirmTkt/Ixigo normalized shape -> BetweenTrain
            let trains: Vec<BetweenTrain> = data["trains"]
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
            if trains.is_empty() {
                return Err(AppError::source_unavailable(
                    metric.clone(),
                    "no trains in response",
                ));
            }
            TrainsBetweenResponse {
                src: Some(src.to_string()),
                dst: Some(dst.to_string()),
                trains: Some(trains),
                data_source: Some(match metric.as_str() {
                    "irctc" => irctc::client::SOURCE.to_string(),
                    "confirmtkt" => crate::core::confirmtkt::SOURCE.to_string(),
                    "ixigo" => crate::core::ixigo::SOURCE.to_string(),
                    "erail" => crate::core::erail::SOURCE.to_string(),
                    _ => crate::core::paytm::client::SOURCE.to_string(),
                }),
            }
        };

        state.cache.set_json(&cache_key, &resp)?;
        Ok(resp)
    }
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
