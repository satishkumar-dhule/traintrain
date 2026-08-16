//! Tolerant normalizers for IRCTC no-login API payloads.
//!
//! IRCTC's JSON has no public schema; the field names below are the ones the
//! mobile booking API and the online-charts UI emit and that community
//! clients read. Extraction is defensive (every field falls back to an
//! empty/absent value) but a missing or empty train/coach list is an honest
//! `AppError::SourceUnavailable`, never fabricated data.
use serde_json::{json, Value};

use crate::core::error::AppError;

use super::client::SOURCE;

/// Navigate `v` along `path` without panicking.
pub fn deep_get<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur)
}

/// `2026-08-20` / `20-08-2026` / `20/08/2026` / `20260820` -> `20260820`
/// (the `jrnyDate` format the altAvlEnq API expects). Unparseable input is
/// passed through so the upstream rejects it and we fail honestly.
pub fn date_compact(date: &str) -> String {
    date_parse(date)
        .map(|d| d.format("%Y%m%d").to_string())
        .unwrap_or_else(|| date.trim().to_string())
}

/// Same inputs -> `2026-08-20` (the `jDate` format online-charts expects).
pub fn date_iso(date: &str) -> String {
    date_parse(date)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| date.trim().to_string())
}

fn date_parse(date: &str) -> Option<chrono::NaiveDate> {
    let s = date.trim();
    for fmt in ["%Y-%m-%d", "%Y%m%d", "%d-%m-%Y", "%d/%m/%Y"] {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return Some(d);
        }
    }
    None
}

/// Normalize an `altAvlEnq` response into the shared `{ "trains": [...] }`
/// intermediate consumed by both the availability slice and the
/// trains-between fallback. Each train carries the wire fields plus `runs_on`
/// (7 booleans, Monday-first).
pub fn availability_trains(data: &Value) -> Result<Value, AppError> {
    let list = ["trainBtwnStnsList", "trainList"]
        .iter()
        .find_map(|k| data.get(*k).and_then(Value::as_array))
        .filter(|a| !a.is_empty())
        .ok_or_else(|| {
            AppError::source_unavailable(SOURCE, "unexpected altAvlEnq response shape")
        })?;

    let trains: Vec<Value> = list
        .iter()
        .map(|t| {
            json!({
                "number": num_field(t, &["trainNumber", "trainNo"]),
                "name": field(t, &["trainName"]),
                "from_code": field(t, &["fromStnCode", "srcStnCode", "fromStationCode"]),
                "from_name": field(t, &["fromStnName", "srcStnName"]),
                "to_code": field(t, &["toStnCode", "destStnCode", "toStationCode"]),
                "to_name": field(t, &["toStnName", "destStnName"]),
                "departure_time": field(t, &["departureTime", "depTime"]),
                "arrival_time": field(t, &["arrivalTime", "arrTime"]),
                "duration": field(t, &["duration"]),
                "distance": field(t, &["distance"]),
                "classes": list_field(t, &["avlClasses", "availableClasses"]),
                "train_type": field(t, &["trainType"]),
                "runs_on": day_bools(t),
            })
        })
        .collect();

    Ok(json!({ "trains": trains }))
}

/// Normalize a `trainComposition` response into
/// `{ train_number, train_name, coaches: [{ code, class_code, berths }] }`.
///
/// The envelope is undocumented; the known client (the online-charts UI)
/// nests the coach list under `trainData.coachList` with per-coach
/// `coachCode` / `classCode` / `berthList`, each berth `{ berthNo, status }`.
/// Extraction accepts the documented shape and a flat top-level list, and
/// stays honest when neither is present.
pub fn chart(data: &Value) -> Result<Value, AppError> {
    let train_data = deep_get(data, &["trainData"]).unwrap_or(data);
    let list = ["coachList", "coachDetails", "coaches"]
        .iter()
        .find_map(|k| train_data.get(*k).and_then(Value::as_array))
        .filter(|a| !a.is_empty())
        .ok_or_else(|| {
            AppError::source_unavailable(SOURCE, "unexpected trainComposition response shape")
        })?;

    let coaches: Vec<Value> = list
        .iter()
        .map(|c| {
            let berths = c
                .get("berthList")
                .and_then(Value::as_array)
                .or_else(|| c.get("berths").and_then(Value::as_array))
                .map(|arr| {
                    arr.iter()
                        .map(|b| {
                            json!({
                                "number": int_field(b, &["berthNo", "berthNumber"]).unwrap_or(0),
                                "status": berth_status(b),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            json!({
                "code": field(c, &["coachCode", "coachNo", "coach"]),
                "class_code": field(c, &["classCode", "coachClass", "class"]),
                "berths": berths,
            })
        })
        .collect();

    Ok(json!({
        "train_number": field(train_data, &["trainNumber", "trainNo"]),
        "train_name": field(train_data, &["trainName", "trainNameHn"]),
        "coaches": coaches,
    }))
}

/// `runs_on` (7 booleans, Monday-first) from an IRCTC train entry.
///
/// IRCTC lists run days as `runDays: ["MON", "WED"]` (full names) or
/// `["MO", "WE"]` (short names); NTES-style `runOnMon..runOnSun` booleans are
/// accepted too. Defaults to all-false for a missing/empty field.
fn day_bools(entry: &Value) -> Vec<bool> {
    const FULL: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    const SHORT: [&str; 7] = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];

    let mut out = vec![false; 7];
    if let Some(days) = entry.get("runDays").and_then(Value::as_array) {
        for d in days {
            let d = d.as_str().unwrap_or_default().to_uppercase();
            if let Some(i) = FULL
                .iter()
                .position(|x| *x == d)
                .or_else(|| SHORT.iter().position(|x| *x == d))
            {
                out[i] = true;
            }
        }
        if out.iter().any(|b| *b) {
            return out;
        }
    }
    for (i, day) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        .iter()
        .enumerate()
    {
        out[i] = entry
            .get(format!("runOn{day}"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || entry
                .get(format!("runsOn{day}"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
    }
    out
}

fn berth_status(b: &Value) -> String {
    let code = field(b, &["status", "avail", "available", "bookingStatus"]);
    if !code.is_empty() {
        return code.to_lowercase();
    }
    if b.get("vacant").and_then(Value::as_bool) == Some(true) {
        return "vacant".to_string();
    }
    if b.get("booked").and_then(Value::as_bool) == Some(true) {
        return "occupied".to_string();
    }
    "unknown".to_string()
}

fn field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

/// String-or-number field (train numbers arrive as both in the wild).
fn num_field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| match v.get(*k) {
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn list_field(v: &Value, keys: &[&str]) -> Vec<String> {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_array))
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn int_field(v: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|k| match v.get(*k) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_compact_normalizes_human_formats() {
        assert_eq!(date_compact("2026-08-20"), "20260820");
        assert_eq!(date_compact("20-08-2026"), "20260820");
        assert_eq!(date_compact("20/08/2026"), "20260820");
        assert_eq!(date_compact("20260820"), "20260820");
        assert_eq!(date_iso("20260820"), "2026-08-20");
        assert_eq!(date_iso("20-08-2026"), "2026-08-20");
        assert_eq!(date_iso("2026-08-20"), "2026-08-20");
        assert_eq!(date_compact("not-a-date"), "not-a-date");
    }

    #[test]
    fn availability_accepts_documented_shape() {
        let data = json!({
            "trainBtwnStnsList": [
                {
                    "trainNumber": "12951",
                    "trainName": "MUMBAI RAJDHANI",
                    "fromStnCode": "MMCT",
                    "fromStnName": "MUMBAI CENTRAL",
                    "toStnCode": "NDLS",
                    "toStnName": "NEW DELHI",
                    "departureTime": "17:40",
                    "arrivalTime": "08:32",
                    "duration": "14:52",
                    "distance": "1384",
                    "avlClasses": ["3A", "2A"],
                    "trainType": "SUF",
                    "runDays": ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
                },
                {
                    "trainNumber": "12009",
                    "trainName": "SHATABDI EXP",
                    "departureTime": "05:40",
                    "arrivalTime": "21:55",
                    "runDays": ["MON", "WED", "FRI"]
                }
            ]
        });
        let norm = availability_trains(&data).unwrap();
        let trains = norm["trains"].as_array().unwrap();
        assert_eq!(trains.len(), 2);
        assert_eq!(trains[0]["number"], "12951");
        assert_eq!(trains[0]["from_code"], "MMCT");
        assert_eq!(trains[0]["classes"], json!(["3A", "2A"]));
        assert_eq!(
            trains[0]["runs_on"],
            json!([true, true, true, true, true, true, true])
        );
        assert_eq!(
            trains[1]["runs_on"],
            json!([true, false, true, false, true, false, false])
        );
    }

    #[test]
    fn availability_accepts_short_run_days_and_numeric_codes() {
        let data = json!({
            "trainList": [
                {"trainNumber": "12951", "trainName": "X", "runDays": ["MO", "WE"]},
                {"trainNo": 12009, "trainName": "Y", "runDays": ["SA", "SU"]}
            ]
        });
        let norm = availability_trains(&data).unwrap();
        let trains = norm["trains"].as_array().unwrap();
        assert_eq!(trains[0]["number"], "12951");
        assert_eq!(
            trains[0]["runs_on"],
            json!([true, false, true, false, false, false, false])
        );
        assert_eq!(
            trains[1]["number"], "12009",
            "numeric trainNo must stringify"
        );
        assert_eq!(
            trains[1]["runs_on"],
            json!([false, false, false, false, false, true, true])
        );
    }

    #[test]
    fn availability_rejects_missing_or_empty_list() {
        for data in [
            json!({ "some": "thing" }),
            json!({ "trainBtwnStnsList": [] }),
        ] {
            let err = availability_trains(&data).unwrap_err();
            assert!(
                matches!(&err, AppError::SourceUnavailable { source, .. } if source == SOURCE),
                "expected SourceUnavailable, got {err:?}"
            );
        }
    }

    #[test]
    fn chart_accepts_documented_shape() {
        let data = json!({
            "trainData": {
                "trainNumber": "12951",
                "trainName": "MUMBAI RAJDHANI",
                "coachList": [
                    {
                        "coachCode": "B1",
                        "classCode": "3A",
                        "berthList": [
                            {"berthNo": 1, "status": "vacant"},
                            {"berthNo": 2, "status": "occupied"},
                            {"berthNo": 3, "status": "not_reserved"}
                        ]
                    },
                    {"coachCode": "B2", "classCode": "3A", "berthList": []}
                ]
            }
        });
        let norm = chart(&data).unwrap();
        assert_eq!(norm["train_number"], "12951");
        let coaches = norm["coaches"].as_array().unwrap();
        assert_eq!(coaches.len(), 2);
        assert_eq!(coaches[0]["code"], "B1");
        assert_eq!(coaches[0]["class_code"], "3A");
        let berths = coaches[0]["berths"].as_array().unwrap();
        assert_eq!(berths.len(), 3);
        assert_eq!(berths[0]["number"], 1);
        assert_eq!(berths[0]["status"], "vacant");
        assert_eq!(berths[1]["status"], "occupied");
        assert_eq!(coaches[1]["berths"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn chart_accepts_flat_list_and_boolean_status() {
        let data = json!({
            "coaches": [
                {"coach": "S1", "class": "SL", "berths": [
                    {"berthNo": 1, "vacant": true},
                    {"berthNo": 2, "booked": true}
                ]}
            ]
        });
        let norm = chart(&data).unwrap();
        let berths = norm["coaches"][0]["berths"].as_array().unwrap();
        assert_eq!(berths[0]["status"], "vacant");
        assert_eq!(berths[1]["status"], "occupied");
    }

    #[test]
    fn chart_rejects_unrecognized_shape() {
        let err = chart(&json!({ "trainData": { "message": "x" } })).unwrap_err();
        assert!(matches!(&err, AppError::SourceUnavailable { source, .. } if source == SOURCE));
    }
}
