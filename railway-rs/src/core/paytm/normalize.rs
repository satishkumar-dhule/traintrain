//! Tolerant normalizer for Paytm Travel search payloads.
//!
//! Maps `body.trains[]` into the same intermediate `{ "trains": [...] }`
//! shape the IRCTC availability normalizer emits, plus a per-class
//! `availability` list carrying booking status / fare / PNR prediction.
use serde_json::{json, Value};

use crate::core::error::AppError;

use super::client::SOURCE;

/// `2026-08-20` / `20-08-2026` / `20/08/2026` / `20260820` -> `20260820`
/// (the `departureDate` format the search API expects). Unparseable input is
/// passed through so the upstream rejects it and we fail honestly.
pub fn date_compact(date: &str) -> String {
    let s = date.trim();
    for fmt in ["%Y-%m-%d", "%Y%m%d", "%d-%m-%Y", "%d/%m/%Y"] {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return d.format("%Y%m%d").to_string();
        }
    }
    s.to_string()
}

/// Normalize a search response into `{ "trains": [...] }`. Each train carries
/// the shared wire fields (`number`, `name`, times as `HH:MM`, `classes`,
/// Monday-first `runs_on` booleans) plus `availability`: one entry per class
/// with `{ class, class_name, status, available, fare, quota, prediction }`.
pub fn availability_trains(data: &Value) -> Result<Value, AppError> {
    let list = data
        .pointer("/body/trains")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppError::source_unavailable(SOURCE, "unexpected search response shape"))?;

    let type_labels = data
        .pointer("/meta/smartFilterTrainType")
        .and_then(Value::as_object);

    let trains: Vec<Value> = list
        .iter()
        .map(|t| normalize_train(t, type_labels))
        .collect();

    Ok(json!({ "trains": trains }))
}

fn normalize_train(t: &Value, type_labels: Option<&serde_json::Map<String, Value>>) -> Value {
    json!({
        "number": str_field(t, &["trainNumber"]),
        "name": str_field(t, &["trainName"]),
        "from_code": str_field(t, &["source"]),
        "from_name": str_field(t, &["source_name"]),
        "to_code": str_field(t, &["destination"]),
        "to_name": str_field(t, &["destination_name"]),
        "departure_time": iso_time(&str_field(t, &["departure"])),
        "arrival_time": iso_time(&str_field(t, &["arrival"])),
        "duration": str_field(t, &["duration"]),
        "distance": "",
        "classes": t.get("classes")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        "train_type": type_label(t, type_labels),
        "runs_on": day_bools(t),
        "availability": class_availability(t),
    })
}

/// Per-class availability entries; missing/odd entries are skipped.
fn class_availability(t: &Value) -> Vec<Value> {
    t.get("availability")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let class = str_field(a, &["code"]);
                    if class.is_empty() {
                        return None;
                    }
                    let mut entry = json!({
                        "class": class,
                        "class_name": str_field(a, &["name"]),
                        "status": str_field(a, &["non_formatted_status", "status"]),
                    });
                    if let Some(flag) = bool_field(a, "available_flag") {
                        entry["available"] = json!(flag);
                    }
                    if let Some(fare) = int_field(a, "fare") {
                        entry["fare"] = json!(fare);
                    }
                    let quota = str_field(a, &["quota"]);
                    if !quota.is_empty() {
                        entry["quota"] = json!(quota);
                    }
                    if let Some(p) = int_field_pointer(a, &["pnr_prediction", "value"]) {
                        entry["prediction"] = json!(p);
                    }
                    Some(entry)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `runs_on` (7 booleans, Monday-first) parsed from the human text
/// (`"Runs on Tue, Wed"`); `"Runs on All Days"`/daily variants mark all seven
/// days; defaults to all-false when absent/unparseable.
fn day_bools(t: &Value) -> Vec<bool> {
    const DAYS: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let text = t
        .pointer("/runs_on/text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_uppercase();
    if text.contains("ALL DAYS") || text.contains("DAILY") {
        return vec![true; 7];
    }
    let mut out = vec![false; 7];
    for (i, day) in DAYS.iter().enumerate() {
        out[i] = text.contains(day);
    }
    out
}

/// Human-readable train-type label resolved against the response's own
/// `meta.smartFilterTrainType` map (`"o"` -> `"Other Trains"`), falling back
/// to the raw code uppercased.
fn type_label(t: &Value, labels: Option<&serde_json::Map<String, Value>>) -> String {
    let code = str_field(t, &["train_type"]).to_lowercase();
    if code.is_empty() {
        return String::new();
    }
    labels
        .and_then(|m| m.get(&code))
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| code.to_uppercase())
}

/// `"2026-10-20T10:00:00+00:00"` -> `"10:00"` (Paytm emits local wall-clock
/// times mislabeled with a UTC offset; only the time-of-day is meaningful).
fn iso_time(iso: &str) -> String {
    let rest = iso.split_once('T').map(|(_, r)| r).unwrap_or(iso);
    let mut parts = rest.split(':').take(2);
    match (parts.next(), parts.next()) {
        (Some(h), Some(m)) if is_two_digits(h) && is_two_digits(m) => format!("{h}:{m}"),
        _ => String::new(),
    }
}

fn is_two_digits(s: &str) -> bool {
    s.len() == 2 && s.bytes().all(|b| b.is_ascii_digit())
}

fn str_field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

/// Boolean field that may arrive as real JSON boolean or as `"true"/"false"`
/// strings (the live API does both).
fn bool_field(v: &Value, key: &str) -> Option<bool> {
    match v.get(key) {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::String(s)) => match s.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn int_field(v: &Value, key: &str) -> Option<i64> {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn int_field_pointer(v: &Value, path: &[&str]) -> Option<i64> {
    let mut cur = v;
    for key in path {
        cur = cur.get(*key)?;
    }
    match cur {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({
            "status": {"result": "success"},
            "body": {
                "trains": [
                    {
                        "departure": "2026-10-20T10:00:00+00:00",
                        "arrival": "2026-10-21T14:10:00+00:00",
                        "trainName": "GOA SMPRK K",
                        "trainNumber": "12449",
                        "source": "MAO",
                        "destination": "NDLS",
                        "source_name": "Madgaon",
                        "destination_name": "New Delhi",
                        "duration": "28:10",
                        "classes": ["SL", "3E", "3A"],
                        "train_type": "o",
                        "runs_on": {"text": "Runs on Tue, Wed"},
                        "availability": [
                            {
                                "code": "SL",
                                "name": "Sleeper Class",
                                "non_formatted_status": "GNWL82/WL59",
                                "status": "GNWL82/WL59",
                                "available_flag": "false",
                                "fare": 875,
                                "quota": "GN",
                                "pnr_prediction": {"value": 95}
                            },
                            {
                                "code": "3A",
                                "name": "AC 3 Tier",
                                "status": "AVAILABLE 0022",
                                "available_flag": true,
                                "fare": "2195"
                            }
                        ]
                    }
                ]
            },
            "meta": {"smartFilterTrainType": {"o": "Other Trains"}}
        })
    }

    #[test]
    fn date_compact_normalizes_human_formats() {
        assert_eq!(date_compact("2026-08-20"), "20260820");
        assert_eq!(date_compact("20-08-2026"), "20260820");
        assert_eq!(date_compact("20/08/2026"), "20260820");
        assert_eq!(date_compact("20260820"), "20260820");
        assert_eq!(date_compact("not-a-date"), "not-a-date");
    }

    #[test]
    fn normalizes_trains_with_availability_classes() {
        let norm = availability_trains(&sample()).unwrap();
        let trains = norm["trains"].as_array().unwrap();
        assert_eq!(trains.len(), 1);

        let t = &trains[0];
        assert_eq!(t["number"], "12449");
        assert_eq!(t["name"], "GOA SMPRK K");
        assert_eq!(t["from_code"], "MAO");
        assert_eq!(t["from_name"], "Madgaon");
        assert_eq!(t["to_code"], "NDLS");
        assert_eq!(t["to_name"], "New Delhi");
        assert_eq!(t["departure_time"], "10:00");
        assert_eq!(t["arrival_time"], "14:10");
        assert_eq!(t["duration"], "28:10");
        assert_eq!(t["classes"], json!(["SL", "3E", "3A"]));
        assert_eq!(t["train_type"], "Other Trains");
        assert_eq!(
            t["runs_on"],
            json!([false, true, true, false, false, false, false])
        );

        let avl = t["availability"].as_array().unwrap();
        assert_eq!(avl.len(), 2);
        assert_eq!(avl[0]["class"], "SL");
        assert_eq!(avl[0]["class_name"], "Sleeper Class");
        assert_eq!(avl[0]["status"], "GNWL82/WL59");
        assert_eq!(avl[0]["available"], json!(false));
        assert_eq!(avl[0]["fare"], json!(875));
        assert_eq!(avl[0]["quota"], "GN");
        assert_eq!(avl[0]["prediction"], json!(95));
        assert_eq!(avl[1]["class"], "3A");
        assert_eq!(avl[1]["status"], "AVAILABLE 0022");
        assert_eq!(avl[1]["available"], json!(true));
        assert_eq!(avl[1]["fare"], json!(2195), "numeric-string fares parse");
        assert!(
            avl[1].get("quota").is_none(),
            "absent quota must stay absent"
        );
    }

    #[test]
    fn rejects_missing_or_empty_train_list() {
        for data in [
            json!({"status": {"result": "success"}}),
            json!({"body": {"trains": []}}),
        ] {
            let err = availability_trains(&data).unwrap_err();
            assert!(
                matches!(&err, AppError::SourceUnavailable { source, .. } if source == SOURCE),
                "expected SourceUnavailable, got {err:?}"
            );
        }
    }

    #[test]
    fn day_bools_handles_all_days_and_daily() {
        let all = day_bools(&json!({"runs_on": {"text": "Runs on All Days"}}));
        assert_eq!(all, vec![true; 7]);
        let daily = day_bools(&json!({"runs_on": {"text": "Runs Daily"}}));
        assert_eq!(daily, vec![true; 7]);
        let some = day_bools(&json!({"runs_on": {"text": "Runs on Tue, Wed"}}));
        assert_eq!(some, vec![false, true, true, false, false, false, false]);
        let none = day_bools(&json!({}));
        assert_eq!(none, vec![false; 7]);
    }

    #[test]
    fn iso_time_extracts_wall_clock() {
        assert_eq!(iso_time("2026-10-20T10:00:00+00:00"), "10:00");
        assert_eq!(iso_time("2026-10-21T14:10:00+00:00"), "14:10");
        assert_eq!(iso_time("17:40"), "17:40");
        assert_eq!(iso_time(""), "");
        assert_eq!(iso_time("garbage"), "");
    }

    #[test]
    fn type_label_falls_back_to_raw_code() {
        let data = json!({
            "body": {"trains": [{"trainNumber": "1", "trainName": "X", "train_type": "vb"}]}
        });
        let norm = availability_trains(&data).unwrap();
        assert_eq!(norm["trains"][0]["train_type"], "VB");
    }
}
