//! Railyatri HTML/JSON extraction helpers shared by the pnr, schedule and
//! live-status vertical slices (DRY - parsing lives here, not per slice).

use serde_json::{json, Value};

use super::error::AppError;

/// Extract the JSON embedded in `<script id="__NEXT_DATA__" type="application/json">`.
/// Returns the parsed top-level object (the `props.pageProps` object is at
/// `.props.pageProps` for Next.js pages).
pub fn extract_next_data(html: &str) -> Result<Value, AppError> {
    let start_tag = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    let end_tag = "</script>";
    let start = html
        .find(start_tag)
        .ok_or_else(|| AppError::internal("Railyatri: __NEXT_DATA__ script not found"))?
        + start_tag.len();
    let end = html[start..]
        .find(end_tag)
        .ok_or_else(|| AppError::internal("Railyatri: __NEXT_DATA__ script not closed"))?;
    serde_json::from_str(&html[start..start + end])
        .map_err(|e| AppError::internal(format!("Railyatri: __NEXT_DATA__ is not valid JSON: {e}")))
}

/// Deep-get a value by a dotted path (e.g. `"props.pageProps.trainTimeTable"`).
pub fn deep_get<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

pub fn get_string(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.to_string())
}

pub fn get_i64(v: &Value) -> Option<i64> {
    v.as_i64()
}

/// Minutes past midnight -> `"HH:MM"` (or empty string for null/missing).
/// Railyatri timetables use `sta_min` / `std_min`.
pub fn minutes_to_hhmm(minutes: Option<i64>) -> String {
    let m = match minutes {
        Some(m) => m,
        None => return String::new(),
    };
    let m = ((m % 1440) + 1440) % 1440;
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Normalise a `run_days` value into `["MON","TUE",...]`, handling both the
/// modern array form and legacy object form.
pub fn normalize_run_days(v: &Value) -> Vec<String> {
    match v {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|d| d.as_str().map(|s| s.to_uppercase()))
            .collect(),
        Value::Object(map) => map
            .iter()
            .filter(|(_, val)| val.as_bool().unwrap_or(false))
            .map(|(k, _)| k.to_uppercase())
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract the `station_code`/`station_name` pair from a timetable stop entry.
pub fn stop_pair(stop: &Value) -> (String, String) {
    let code = stop
        .get("station_code")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let name = stop
        .get("station_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    (code, name)
}

/// Wrap a payload so `{"data": ...}` responses are handled consistently.
pub fn wrap_ok(data: Value) -> Value {
    json!({ "data": data })
}

/// Parse a Railyatri timetable page (`__NEXT_DATA__` ->
/// `props.pageProps.trainTimeTable`) into a normalized schedule object:
/// `{"train_number","train_name","run_days","route_description","stops":[...]}`.
/// Each stop is normalized to `{"code","name","arrival","departure","day","stop"}`
/// where `arrival`/`departure` are `"HH:MM"` strings (empty when the source omits
/// the value). `route_description` is `<source> - <destination> Express`, or `""`
/// when the source fields are missing. Never panics; missing fields fall back to
/// sensible defaults.
pub fn parse_schedule(html: &str) -> Result<Value, AppError> {
    let nd = extract_next_data(html)?;
    let Some(ttt) = deep_get(&nd, "props.pageProps.trainTimeTable") else {
        return Ok(json!({
            "train_number": "",
            "train_name": "",
            "run_days": [],
            "route_description": "",
            "stops": [],
        }));
    };
    let train_number = ttt
        .get("train_number")
        .and_then(get_string)
        .unwrap_or_default();
    let train_name = ttt
        .get("train_name")
        .and_then(get_string)
        .unwrap_or_default();
    let run_days = ttt
        .get("run_days")
        .map(normalize_run_days)
        .unwrap_or_default();
    let source = ttt
        .get("source_station")
        .and_then(get_string)
        .unwrap_or_default();
    let destination = ttt
        .get("destination_station")
        .and_then(get_string)
        .unwrap_or_default();
    let route_description = if source.is_empty() || destination.is_empty() {
        String::new()
    } else {
        format!("{source} - {destination} Express")
    };
    let stops: Vec<Value> = timetable_stops(ttt).iter().map(stop_to_value).collect();
    Ok(json!({
        "train_number": train_number,
        "train_name": train_name,
        "run_days": run_days,
        "route_description": route_description,
        "stops": stops,
    }))
}

/// Parse a Railyatri live status page into a normalized object: the
/// `props.pageProps.ltsData` fields (primitives stringified, strings kept as-is)
/// plus a `stops` array built from `props.pageProps.timeTableData[0].route`
/// (each stop: `{"code","name","arrival","departure","day"}`). Errors when
/// `ltsData` is absent; a missing route yields an empty `stops` array.
pub fn parse_live_status(html: &str) -> Result<Value, AppError> {
    let nd = extract_next_data(html)?;
    let Some(lts) = deep_get(&nd, "props.pageProps.ltsData") else {
        return Err(AppError::internal("Railyatri: no ltsData in page"));
    };
    let stops: Vec<Value> = deep_get(&nd, "props.pageProps.timeTableData")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|entry| entry.get("route"))
        .and_then(Value::as_array)
        .map(|route| route.iter().map(live_stop_to_value).collect())
        .unwrap_or_default();
    Ok(json!({
        "train_number": lts_field(lts.get("train_number")),
        "train_name": lts_field(lts.get("train_name")),
        "train_start_date": lts_field(lts.get("train_start_date")),
        "at_src": lts_field(lts.get("at_src")),
        "at_dstn": lts_field(lts.get("at_dstn")),
        "at_src_dstn": lts_field(lts.get("at_src_dstn")),
        "next_station_code": lts_field(lts.get("next_station_code")),
        "next_station_name": lts_field(lts.get("next_station_name")),
        "title": lts_field(lts.get("title")),
        "new_message": lts_field(lts.get("new_message")),
        "spent_time": lts_field(lts.get("spent_time")),
        "source_stn_name": lts_field(lts.get("source_stn_name")),
        "dest_stn_name": lts_field(lts.get("dest_stn_name")),
        "platform_number": lts_field(lts.get("platform_number")),
        "stops": stops,
    }))
}

/// Parse the JSON body of the Railyatri `GET {base}/get-status/{pnr}` endpoint.
/// Returns the parsed object unchanged (e.g. `{"status":false}` for an invalid
/// PNR); errors when the body is not JSON (or is not a JSON object).
pub fn parse_pnr_getstatus(body: &str) -> Result<Value, AppError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| AppError::internal("Railyatri: get-status response was not JSON"))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(AppError::internal(
            "Railyatri: get-status response was not a JSON object",
        ))
    }
}

/// Pick the primary timetable stops from a `trainTimeTable` object. Prefers the
/// first `routeGroup`'s `routesummary` when it is a complete timetable; falls
/// back to the union of all route groups (deduped by `station_code`, first
/// occurrence wins) and then to `trainScheduleDetail[0].route`.
fn timetable_stops(ttt: &Value) -> Vec<Value> {
    let mut candidates: Vec<Vec<Value>> = Vec::new();

    if let Some(groups) = ttt.get("routeGroup").and_then(Value::as_array) {
        if let Some(rs) = groups
            .first()
            .and_then(|g| g.get("routesummary"))
            .and_then(Value::as_array)
        {
            candidates.push(rs.clone());
        }
        let mut seen: Vec<String> = Vec::new();
        let mut merged: Vec<Value> = Vec::new();
        for group in groups {
            if let Some(rs) = group.get("routesummary").and_then(Value::as_array) {
                for stop in rs {
                    let code = stop
                        .get("station_code")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if !seen.contains(&code) {
                        seen.push(code);
                        merged.push(stop.clone());
                    }
                }
            }
        }
        if !merged.is_empty() {
            candidates.push(merged);
        }
    }

    if let Some(schedule) = ttt
        .get("trainScheduleDetail")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|entry| entry.get("route"))
        .and_then(Value::as_array)
    {
        candidates.push(schedule.clone());
    }

    candidates
        .iter()
        .find(|c| c.len() >= 10)
        .or_else(|| candidates.iter().find(|c| !c.is_empty()))
        .cloned()
        .unwrap_or_default()
}

/// Normalize a timetable stop into the schedule shape.
fn stop_to_value(stop: &Value) -> Value {
    let (code, name) = stop_pair(stop);
    let sta_min = stop.get("sta_min").and_then(get_i64);
    let std_min = stop.get("std_min").and_then(get_i64);
    let day = stop.get("day").and_then(get_i64).unwrap_or(1);
    let stop_flag = stop.get("stop").and_then(|v| v.as_bool()).unwrap_or(true);
    json!({
        "code": code,
        "name": name,
        "arrival": minutes_to_hhmm(sta_min),
        "departure": minutes_to_hhmm(std_min),
        "day": day,
        "stop": stop_flag,
    })
}

/// Normalize a timetable stop into the live status shape (no `stop` flag).
fn live_stop_to_value(stop: &Value) -> Value {
    let (code, name) = stop_pair(stop);
    let sta_min = stop.get("sta_min").and_then(get_i64);
    let std_min = stop.get("std_min").and_then(get_i64);
    let day = stop.get("day").and_then(get_i64).unwrap_or(1);
    json!({
        "code": code,
        "name": name,
        "arrival": minutes_to_hhmm(sta_min),
        "departure": minutes_to_hhmm(std_min),
        "day": day,
    })
}

/// Stringify an `ltsData` primitive (numbers/bools become strings, strings pass
/// through); null for missing or complex values.
fn lts_field(v: Option<&Value>) -> Value {
    match v {
        Some(Value::String(s)) => Value::String(s.clone()),
        Some(Value::Number(n)) => Value::String(n.to_string()),
        Some(Value::Bool(b)) => Value::String(b.to_string()),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_next_data_from_script() {
        let html = format!(
            "<html><script id=\"__NEXT_DATA__\" type=\"application/json\">{}</script></html>",
            json!({"props": {"pageProps": {"x": 1}}})
        );
        let v = extract_next_data(&html).unwrap();
        assert_eq!(deep_get(&v, "props.pageProps.x"), Some(&json!(1)));
    }

    #[test]
    fn extract_next_data_missing_script_is_error() {
        assert!(extract_next_data("<html></html>").is_err());
    }

    #[test]
    fn minutes_format_and_overflow() {
        assert_eq!(minutes_to_hhmm(Some(0)), "00:00");
        assert_eq!(minutes_to_hhmm(Some(855)), "14:15");
        assert_eq!(minutes_to_hhmm(Some(1440 + 5)), "00:05");
        assert_eq!(minutes_to_hhmm(None), "");
        assert_eq!(minutes_to_hhmm(Some(-30)), "23:30");
    }

    #[test]
    fn run_days_array_and_object_forms() {
        assert_eq!(
            normalize_run_days(&json!(["MON", "Tue", "FRI"])),
            vec!["MON", "TUE", "FRI"]
        );
        assert_eq!(
            normalize_run_days(&json!({"MON": true, "TUE": false, "WED": true})),
            vec!["MON", "WED"]
        );
        assert_eq!(normalize_run_days(&json!("nope")), Vec::<String>::new());
    }

    #[test]
    fn deep_get_missing_returns_none() {
        let v = json!({"a": {"b": 1}});
        assert_eq!(deep_get(&v, "a.b"), Some(&json!(1)));
        assert_eq!(deep_get(&v, "a.c"), None);
        assert_eq!(deep_get(&v, "x.y"), None);
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("testdata/{name}"))
            .unwrap_or_else(|e| panic!("cannot read testdata/{name}: {e}"))
    }

    fn hhmm(v: &Value, field: &str) -> String {
        v.get(field)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn parse_schedule_against_real_fixture() {
        let out = parse_schedule(&fixture("ry_schedule_12951.html")).unwrap();
        assert_eq!(out["train_number"], "12951");
        assert!(out["train_name"].as_str().is_some_and(|s| !s.is_empty()));
        let run_days = out["run_days"].as_array().unwrap();
        assert!(run_days.iter().any(|d| d == "MON"));
        assert_eq!(
            out["route_description"],
            "MUMBAI CENTRAL - NEW DELHI Express"
        );
        let stops = out["stops"].as_array().unwrap();
        assert!(!stops.is_empty());
        assert_eq!(stops[0]["code"], "MMCT");
        assert!(stops.len() > 100);
        for stop in stops {
            let arrival = hhmm(stop, "arrival");
            let departure = hhmm(stop, "departure");
            assert!(
                arrival.is_empty() || arrival.len() == 5,
                "bad arrival {arrival:?}"
            );
            assert!(
                departure.is_empty() || departure.len() == 5,
                "bad departure {departure:?}"
            );
            assert!(stop["day"].as_i64().unwrap_or(1) >= 1);
        }
    }

    #[test]
    fn parse_live_status_against_real_fixture() {
        let out = parse_live_status(&fixture("ry_live_12951.html")).unwrap();
        assert_eq!(out["train_number"], "12951");
        assert!(out["next_station_code"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(out["next_station_name"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(out["train_start_date"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(out["source_stn_name"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(out["dest_stn_name"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(out["at_src"].as_str().is_some());
        assert!(out["platform_number"].as_str().is_some());
        let stops = out["stops"].as_array().unwrap();
        assert!(stops.len() > 100);
        for stop in stops {
            let arrival = hhmm(stop, "arrival");
            let departure = hhmm(stop, "departure");
            assert!(arrival.is_empty() || arrival.len() == 5);
            assert!(departure.is_empty() || departure.len() == 5);
        }
    }

    #[test]
    fn parse_live_status_missing_lts_data_is_error() {
        let html = "<html><script id=\"__NEXT_DATA__\" type=\"application/json\">{\"props\":{\"pageProps\":{}}}</script></html>";
        assert!(parse_live_status(html).is_err());
    }

    #[test]
    fn parse_pnr_getstatus_valid_json_object() {
        let out = parse_pnr_getstatus(r#"{"status":false}"#).unwrap();
        assert!(out.is_object());
        assert_eq!(out["status"], false);
    }

    #[test]
    fn parse_pnr_getstatus_non_json_is_error() {
        assert!(parse_pnr_getstatus("not json").is_err());
    }
}
