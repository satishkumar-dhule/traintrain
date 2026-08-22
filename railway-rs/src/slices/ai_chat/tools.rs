//! Unified local tool registry shared by two consumers: the assistant's
//! agentic chat loop and the stdio MCP server (`railway-mcp`). Tools run real
//! rail services in-process and return **projected** semantic views — not raw
//! DTO dumps — so every round costs a fraction of the context window.
//!
//! Context discipline lives here too: a per-request [`Budget`] meters total
//! characters fed back to the model, each result is clamped, and every call
//! is bounded by [`TOOL_TIMEOUT`] (timeouts degrade to error payloads the
//! model can reason about, never hangs).

use serde_json::{json, Value};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use crate::core::error::AppError;
use crate::state::AppState;

/// Hard wall-clock bound for one tool execution.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(20);
/// Per-result character cap before truncation markers appear.
pub const RESULT_MAX_CHARS: usize = 4_000;
/// Default whole-request budget across all tool results.
pub const DEFAULT_BUDGET_CHARS: i64 = 24_000;

const MAX_BETWEEN_TRAINS: usize = 12;
const MAX_UPCOMING_STOPS: usize = 4;
const SEARCH_DEFAULT_LIMIT: usize = 6;
const SEARCH_MAX_LIMIT: usize = 8;
const MAX_AVAILABILITY_TRAINS: usize = 8;
const MAX_AVAILABILITY_CLASSES: usize = 6;
const BOARD_MAX_TRAINS: usize = 8;
const BOARD_DEFAULT_HOURS: u32 = 2;
/// NTES live-station windows grow fast; keep the model on a short leash.
const BOARD_MAX_HOURS: u32 = 4;

/// Static descriptor of one callable tool.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON-schema `parameters` object (OpenAI function-calling shape).
    pub parameters: Value,
}

/// The full registry, in priority order.
pub fn registry() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "trains_between",
            description: "Search live trains running between two stations today. Accepts station names ('Hyderabad') or codes ('SC'). Returns schedule times and run-days.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "src": {"type": "string"},
                    "dst": {"type": "string"}
                },
                "required": ["src", "dst"]
            }),
        },
        ToolDef {
            name: "live_status",
            description: "Live running position of a train today: where it is now, delay in minutes, platform and the next few scheduled stops.",
            parameters: json!({
                "type": "object",
                "properties": {"train": {"type": "string"}},
                "required": ["train"]
            }),
        },
        ToolDef {
            name: "average_delay",
            description: "Historical average arrival/departure delay per station for a train.",
            parameters: json!({
                "type": "object",
                "properties": {"train": {"type": "string"}},
                "required": ["train"]
            }),
        },
        ToolDef {
            name: "seat_availability",
            description: "Class-wise booking status (AVAILABLE/WL/RAC/REGRET) with fares for trains between two stations on a date. Use when the user asks whether tickets are available, about waitlist chances, or fares.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "src": {"type": "string"},
                    "dst": {"type": "string"},
                    "date": {"type": "string", "description": "Journey date YYYY-MM-DD (also DD-MM-YYYY or YYYYMMDD); omit for today"}
                },
                "required": ["src", "dst"]
            }),
        },
        ToolDef {
            name: "station_board",
            description: "Trains arriving at / departing from one station within the next few hours: scheduled vs expected time, platform and late flag. Use for 'what is running through SBC right now' style questions.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "station": {"type": "string"},
                    "hours": {"type": "integer", "minimum": 1, "maximum": 4, "description": "Lookahead window in hours; default 2"}
                },
                "required": ["station"]
            }),
        },
        ToolDef {
            name: "search_rail",
            description: "Fuzzy-search the offline rail corpus for stations or trains (BM25 over ~20k documents). Use FIRST to resolve vague place/train names into exact codes/numbers before calling other tools.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 8}
                },
                "required": ["query"]
            }),
        },
    ]
}

/// OpenAI-style `tools` array derived from [`registry`].
pub fn schemas() -> Vec<Value> {
    registry()
        .into_iter()
        .map(|d| {
            json!({
                "type": "function",
                "function": {
                    "name": d.name,
                    "description": d.description,
                    "parameters": d.parameters
                }
            })
        })
        .collect()
}

/// Per-request character meter shared across every tool round.
pub struct Budget {
    remaining: AtomicI64,
}

impl Budget {
    pub fn new(total_chars: i64) -> Self {
        Self {
            remaining: AtomicI64::new(total_chars.max(0)),
        }
    }

    /// Reserve up to `want` characters; returns the granted amount.
    pub fn take(&self, want: usize) -> usize {
        loop {
            let current = self.remaining.load(Ordering::SeqCst);
            let grant = (current.max(0) as usize).min(want);
            if self
                .remaining
                .compare_exchange(
                    current,
                    current - grant as i64,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .is_ok()
            {
                return grant;
            }
        }
    }

    pub fn remaining(&self) -> i64 {
        self.remaining.load(Ordering::SeqCst)
    }
}

/// Execute one named tool end-to-end: validation, live service call,
/// projection, timeout and budget-aware clamping. Errors are returned for
/// *caller* mistakes (unknown tool / bad args / upstream down); the chat loop
/// converts them into payloads the model can reason about.
pub async fn call_tool(
    state: &AppState,
    budget: &Budget,
    name: &str,
    arguments: &str,
) -> Result<String, AppError> {
    if !registry().iter().any(|d| d.name == name) {
        return Err(AppError::bad_request(format!("unknown tool: {name}")));
    }
    let args: Value = serde_json::from_str(arguments.trim()).unwrap_or_else(|_| json!({}));

    let fut = dispatch(state, name, &args);
    let value = tokio::time::timeout(TOOL_TIMEOUT, fut)
        .await
        .map_err(|_| AppError::source_unavailable(name, "tool timed out"))??;

    let serialized = serde_json::to_string(&value).unwrap_or_default();
    let grant = budget.take(serialized.chars().count());
    if grant == 0 {
        return Ok(json!({"context_budget_exhausted": true}).to_string());
    }
    Ok(clamp_to(grant, serialized))
}

async fn dispatch(state: &AppState, name: &str, args: &Value) -> Result<Value, AppError> {
    match name {
        "trains_between" => {
            let src = require_station(state, args, "src").await?;
            let dst = require_station(state, args, "dst").await?;
            if src == dst {
                return Err(AppError::bad_request(
                    "origin and destination are the same station",
                ));
            }
            let dto = crate::slices::trains_between::service::Service::get_trains_between(
                state, &src, &dst,
            )
            .await?;
            let mut view = project_trains_between(&dto);
            view["src_code"] = json!(src);
            view["dst_code"] = json!(dst);
            Ok(view)
        }
        "live_status" => {
            let train = require_train(args)?;
            // Empty date = today IST, resolved by the inner service.
            let dto =
                crate::slices::live_status::service::Service::get_live_status(state, &train, "")
                    .await?;
            Ok(project_live_status(&dto))
        }
        "average_delay" => {
            let train = require_train(args)?;
            let dto =
                crate::slices::average_delay::service::Service::get_average_delay(state, &train)
                    .await?;
            Ok(project_average_delay(&dto))
        }
        "seat_availability" => {
            let src = require_station(state, args, "src").await?;
            let dst = require_station(state, args, "dst").await?;
            if src == dst {
                return Err(AppError::bad_request(
                    "origin and destination are the same station",
                ));
            }
            let date = resolve_journey_date(args)?;
            let dto = crate::slices::availability::service::Service::get_availability(
                state,
                &src,
                &dst,
                &date,
                crate::slices::availability::SourcePref::Auto,
            )
            .await?;
            Ok(project_seat_availability(&dto))
        }
        "station_board" => {
            let station = require_station(state, args, "station").await?;
            let hours = args
                .get("hours")
                .and_then(Value::as_u64)
                .map(|h| h.clamp(1, BOARD_MAX_HOURS as u64) as u32)
                .unwrap_or(BOARD_DEFAULT_HOURS);
            let dto = crate::slices::live_station::service::Service::get_live_station(
                state, &station, hours,
            )
            .await?;
            Ok(project_station_board(&dto))
        }
        "search_rail" => {
            let query = require_str(args, "query")?;
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|l| l.min(SEARCH_MAX_LIMIT as u64) as usize)
                .unwrap_or(SEARCH_DEFAULT_LIMIT);
            let hits = state.retrieval.search(query.trim(), limit);
            Ok(json!({
                "query": query,
                "count": hits.len(),
                "results": hits.into_iter().map(|h| json!({
                    "kind": h.kind,
                    "code": h.code,
                    "title": h.title,
                    "detail": h.detail,
                })).collect::<Vec<_>>()
            }))
        }
        other => Err(AppError::bad_request(format!("unknown tool: {other}"))),
    }
}

// ---------- projections: raw DTO -> compact semantic view ----------

/// Keep identity, live position and only the *upcoming* story; drop the five
/// full historical stop timelines the DTO carries (~85% of its bytes).
fn project_live_status(dto: &crate::models::LiveStatusResponse) -> Value {
    let mut next_stops: Vec<Value> = Vec::new();
    let mut last_delay: Option<i64> = None;
    if let Some(stops) = &dto.stations {
        for s in stops {
            match s.status.as_str() {
                "departed" => last_delay = Some(s.delay_minutes),
                "expected" | "scheduled" => {
                    if next_stops.len() < MAX_UPCOMING_STOPS {
                        next_stops.push(json!({
                            "code": s.code,
                            "name": s.name,
                            "sch": s.scheduled_arrival,
                            "act": (if s.actual_arrival.is_empty() { None } else { Some(s.actual_arrival.clone()) }),
                            "delay_min": s.delay_minutes,
                            "platform": (if s.platform.is_empty() { None } else { Some(s.platform.clone()) }),
                        }));
                    }
                }
                _ => {}
            }
        }
    }

    json!({
        "train_number": dto.train_number,
        "train_name": dto.train_name,
        "position": dto.current_location_info,
        "platform": dto.platform_number,
        "data_source": dto.data_source,
        "last_seen_delay_minutes": last_delay,
        "next_stops": next_stops,
    })
}

/// Compact rows with human-readable run days, capped.
fn project_trains_between(dto: &crate::models::TrainsBetweenResponse) -> Value {
    let days = |mask: &[bool]| -> String {
        const NAMES: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        if mask.len() == 7 && mask.iter().all(|d| *d) {
            return "Daily".into();
        }
        mask.iter()
            .zip(NAMES)
            .filter(|(on, _)| **on)
            .map(|(_, n)| n)
            .collect::<Vec<_>>()
            .join(" ")
    };
    let trains: Vec<Value> = dto
        .trains
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .take(MAX_BETWEEN_TRAINS)
        .map(|t| {
            json!({
                "number": t.number,
                "name": t.name,
                "dep": t.departure_time,
                "arr": t.arrival_time,
                "runs": days(&t.runs_on),
            })
        })
        .collect();
    let total = dto.trains.as_ref().map(Vec::len).unwrap_or(0);
    json!({
        "from": dto.src,
        "to": dto.dst,
        "total_found": total,
        "data_source": dto.data_source,
        "note": (total > MAX_BETWEEN_TRAINS)
            .then(|| format!("showing first {MAX_BETWEEN_TRAINS} of {total}")),
        "trains": trains,
    })
}

/// Average-delay table trimmed to the noisiest stations (largest delays
/// first), which is what travellers actually ask about.
fn project_average_delay(dto: &crate::models::AverageDelayResponse) -> Value {
    let parse_min = |s: &str| -> i64 {
        // Formats seen upstream: "-3", "+12", "" (treat junk as unknown).
        s.trim().trim_start_matches('+').parse().unwrap_or(i64::MIN)
    };
    let mut rows: Vec<&crate::models::AverageDelayStation> =
        dto.stations.iter().flatten().collect();
    rows.sort_by_key(|r| -parse_min(&r.arrival_delay));
    let stations: Vec<Value> = rows
        .iter()
        .take(10)
        .map(|r| {
            json!({
                "code": r.code,
                "name": r.name,
                "arr_delay_min": (if parse_min(&r.arrival_delay) == i64::MIN { None } else { Some(parse_min(&r.arrival_delay)) }),
                "dep_delay_min": (if r.departure_delay.is_empty() { None } else { r.departure_delay.parse::<i64>().ok() }),
            })
        })
        .collect();
    json!({
        "train_no": dto.train_no,
        "train_name": dto.train_name,
        "days_of_run": dto.days_of_run,
        "data_source": dto.data_source,
        "stations_worst_first": stations,
    })
}

/// Seat-availability rows: trains carrying real class-wise status rank first
/// (stable), everything is capped, fares/predictions ride along only when the
/// source supplied them.
fn project_seat_availability(dto: &crate::models::AvailabilityResponse) -> Value {
    let trains = dto.trains.as_deref().unwrap_or(&[]);
    let mut ranked: Vec<&crate::models::AvailabilityTrain> = trains
        .iter()
        .filter(|t| !t.availability.is_empty())
        .collect();
    ranked.extend(trains.iter().filter(|t| t.availability.is_empty()));

    let rows: Vec<Value> = ranked
        .iter()
        .take(MAX_AVAILABILITY_TRAINS)
        .map(|t| {
            json!({
                "number": t.number,
                "name": t.name,
                "dep": t.departure_time,
                "arr": t.arrival_time,
                "duration": t.duration,
                "classes": t.availability.iter().take(MAX_AVAILABILITY_CLASSES).map(|c| {
                    let mut row = json!({
                        "class": c.class,
                        "status": c.status,
                        "tone": availability_tone(c.available, &c.status),
                    });
                    if let Some(fare) = c.fare {
                        row["fare"] = json!(fare);
                    }
                    if let Some(prediction) = c.prediction {
                        row["prediction"] = json!(prediction);
                    }
                    row
                }).collect::<Vec<_>>(),
            })
        })
        .collect();

    json!({
        "from": dto.src,
        "to": dto.dst,
        "date": dto.date,
        "data_source": dto.data_source,
        "notice": dto.notice,
        "trains": rows,
    })
}

/// Coarse signal for UI chips and model reasoning: green when bookable now,
/// red when hopeless, amber for waitlist-ish limbo.
fn availability_tone(available: Option<bool>, status: &str) -> &'static str {
    match available {
        Some(true) => "ok",
        Some(false) => "bad",
        None => {
            let s = status.trim().to_uppercase();
            if s.starts_with("RAC") || s.contains("WL") {
                "warn"
            } else if s.starts_with("REGRET") || s.starts_with("NOT") {
                "bad"
            } else if s.contains("AVAILABLE") {
                "ok"
            } else {
                "warn"
            }
        }
    }
}

/// Live-station board trimmed to identity + arrival story per train.
fn project_station_board(dto: &crate::models::LiveStationResponse) -> Value {
    let rows: Vec<Value> = dto
        .trains
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .take(BOARD_MAX_TRAINS)
        .map(|t| {
            json!({
                "number": t.number,
                "name": t.name,
                "sch": t.sta,
                "eta": t.eta,
                "platform": t.platform,
                "late": t.delay_arr,
            })
        })
        .collect();
    json!({
        "station_code": dto.station,
        "hours": dto.hours,
        "data_source": dto.data_source,
        "trains": rows,
    })
}

// ---------- arg helpers ----------

/// Journey-date argument with the availability slice's semantics: absent,
/// blank or "today" means today IST; anything else must parse as one of the
/// accepted formats and is normalized to ISO (`YYYY-MM-DD`).
fn resolve_journey_date(args: &Value) -> Result<String, AppError> {
    match args.get("date").and_then(Value::as_str) {
        None => Ok(today_ist()),
        Some(raw) => {
            let raw = raw.trim();
            if raw.is_empty() || raw.eq_ignore_ascii_case("today") {
                return Ok(today_ist());
            }
            for fmt in ["%Y-%m-%d", "%Y%m%d", "%d-%m-%Y", "%d/%m/%Y"] {
                if let Ok(d) = chrono::NaiveDate::parse_from_str(raw, fmt) {
                    return Ok(d.to_string());
                }
            }
            Err(AppError::bad_request(format!(
                "Invalid date: {raw}. Use YYYY-MM-DD, DD-MM-YYYY or YYYYMMDD."
            )))
        }
    }
}

fn today_ist() -> String {
    let offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).expect("IST offset is valid");
    chrono::Utc::now()
        .with_timezone(&offset)
        .date_naive()
        .to_string()
}

fn require_str(args: &Value, key: &str) -> Result<String, AppError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request(format!("tool argument '{key}' is required")))
}

fn require_train(args: &Value) -> Result<String, AppError> {
    let t = require_str(args, "train")?;
    if t.len() == 5 && t.bytes().all(|b| b.is_ascii_digit()) && t != "00000" {
        Ok(t)
    } else {
        Err(AppError::bad_request(format!(
            "'{t}' is not a valid 5-digit train number"
        )))
    }
}

/// Resolve a free-text station reference to a dataset code: known codes pass
/// through; anything else goes through BM25 retrieval (kind=station first).
async fn require_station(state: &AppState, args: &Value, key: &str) -> Result<String, AppError> {
    let input = require_str(args, key)?;
    let upper = input.to_ascii_uppercase();
    if crate::slices::station_codes::is_valid_code(&upper)
        && crate::slices::station_codes::code_known(state, &upper)
    {
        return Ok(upper);
    }
    for hit in state.retrieval.search(input.trim(), 3) {
        if hit.kind == "station" {
            return Ok(hit.code);
        }
    }
    Err(AppError::bad_request(format!("unknown station: {input}")))
}

fn clamp_to(max_chars: usize, s: String) -> String {
    if s.chars().count() <= max_chars {
        s
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}…[truncated]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn live_status_projection_drops_history_keeps_signal() {
        fn stop(
            code: &str,
            name: &str,
            sch: &str,
            act: &str,
            platform: &str,
            delay: i64,
            status: &str,
        ) -> crate::models::LiveStop {
            crate::models::LiveStop {
                name: name.into(),
                code: code.into(),
                scheduled_arrival: sch.into(),
                actual_arrival: act.into(),
                platform: platform.into(),
                delay_minutes: delay,
                status: status.into(),
            }
        }
        fn instance(date: &str) -> crate::models::TrainInstance {
            crate::models::TrainInstance {
                start_date: date.into(),
                position: "pos".into(),
                platform_number: String::new(),
                stops: None,
            }
        }
        let dto_typed = crate::models::LiveStatusResponse {
            train_number: Some("12951".into()),
            train_name: Some("RAJDHANI".into()),
            current_location_info: Some("Running late".into()),
            platform_number: Some("3".into()),
            train_start_date: None,
            data_source: Some("NTES".into()),
            stations: Some(vec![
                stop("AAA", "A", "10:00", "10:05", "", 5, "departed"),
                stop("BBB", "B", "11:00", "", "2", 0, "expected"),
                stop("CCC", "C", "12:00", "", "", 0, "scheduled"),
            ]),
            instances: Some(vec![
                instance("01-Jan"),
                instance("31-Dec"),
                instance("30-Dec"),
                instance("29-Dec"),
            ]),
        };
        let out = project_live_status(&dto_typed);
        assert!(out.get("instances").is_none(), "history must be dropped");
        assert_eq!(out["last_seen_delay_minutes"], 5);
        assert_eq!(out["next_stops"].as_array().unwrap().len(), 2);
        assert_eq!(out["next_stops"][0]["code"], "BBB");
        assert_eq!(out["platform"], "3");
    }

    #[test]
    fn between_projection_renders_days_and_caps_rows() {
        let mut trains = Vec::new();
        for i in 0..15 {
            trains.push(json!({
                "number": format!("{i:05}"),
                "name": format!("T{i}"),
                "departure_time":"10:00",
                "arrival_time":"12:00",
                "runs_on": [true,false,true,true,true,true,true]
            }));
        }
        let dto: Value = json!({
            "src":"SC - SECUNDERABAD","dst":"PUNE - PUNE JN",
            "trains":trains,"data_source":"NTES"
        });
        let dto_typed: crate::models::TrainsBetweenResponse = serde_json::from_value(dto).unwrap();
        let out = project_trains_between(&dto_typed);
        let rows = out["trains"].as_array().unwrap();
        assert_eq!(rows.len(), MAX_BETWEEN_TRAINS);
        assert_eq!(rows[0]["runs"], "Mon Wed Thu Fri Sat Sun");
        assert!(out["note"].as_str().unwrap().contains("first 12"));
    }

    #[test]
    fn between_daily_mask_reads_as_daily() {
        let dto: Value = json!({
            "src":"A","dst":"B",
            "trains":[{"number":"11111","name":"DAILY EXP","departure_time":"10:00","arrival_time":"12:00","runs_on":[true,true,true,true,true,true,true]}],
            "data_source":"NTES"
        });
        let dto_typed: crate::models::TrainsBetweenResponse = serde_json::from_value(dto).unwrap();
        let out = project_trains_between(&dto_typed);
        assert_eq!(out["trains"][0]["runs"], "Daily");
    }

    #[test]
    fn average_delay_sorts_worst_first() {
        let dto: Value = json!({
            "train_no":"19020","train_name":"EXP","days_of_run":"Daily",
            "data_source":"NTES",
            "stations":[
                {"sr":"1","name":"CALM","code":"CLM","arrival_delay":"+2","departure_delay":"+2"},
                {"sr":"2","name":"CHAOS","code":"CHS","arrival_delay":"+38","departure_delay":"+41"},
                {"sr":"3","name":"MID","code":"MID","arrival_delay":"+11","departure_delay":"+9"}
            ]
        });
        let dto_typed: crate::models::AverageDelayResponse = serde_json::from_value(dto).unwrap();
        let out = project_average_delay(&dto_typed);
        let rows = out["stations_worst_first"].as_array().unwrap();
        assert_eq!(rows[0]["code"], "CHS");
        assert_eq!(rows[2]["code"], "CLM");
    }

    #[test]
    fn budget_meters_down_and_never_goes_negative() {
        let b = Budget::new(100);
        assert_eq!(b.take(60), 60);
        assert_eq!(b.take(90), 40);
        assert_eq!(b.take(1), 0);
        assert_eq!(b.remaining(), 0);
    }

    #[test]
    fn clamp_marks_cut_payloads() {
        assert_eq!(clamp_to(10, "short".into()), "short");
        let cut = clamp_to(5, "abcdefghij".into());
        assert!(cut.starts_with("abcde") && cut.ends_with("[truncated]"));
    }

    #[test]
    fn tone_covers_every_status_family() {
        assert_eq!(availability_tone(None, "AVAILABLE-145"), "ok");
        assert_eq!(availability_tone(None, "WL 34"), "warn");
        assert_eq!(availability_tone(None, "RAC12"), "warn");
        assert_eq!(availability_tone(None, "REGRET"), "bad");
        assert_eq!(availability_tone(None, "NOT AVAILABLE"), "bad");
        assert_eq!(
            availability_tone(Some(false), "AVAILABLE 0022"),
            "bad",
            "explicit false beats the status text"
        );
        assert_eq!(availability_tone(Some(true), "GNWL82/WL59"), "ok");
    }

    #[test]
    fn seat_projection_prefers_class_data_and_caps_rows() {
        let mut many_classes = Vec::new();
        for i in 0..8 {
            many_classes.push(json!({
                "class": format!("C{i}"),
                "status": if i == 0 { "AVAILABLE 0100" } else { "WL 10" },
                "available": i == 0,
                "fare": 500 + i,
            }));
        }
        let dto: crate::models::AvailabilityResponse = serde_json::from_value(json!({
            "src": "SC", "dst": "PUNE", "date": "2026-08-22",
            "data_source": "Paytm", "notice": "n",
            "trains": [
                {"number": "00001", "name": "NO CLASSES EXP", "from_code": "SC",
                 "from_name": "SECUNDERABAD", "to_code": "PUNE", "to_name": "PUNE JN",
                 "departure_time": "06:00",
                 "arrival_time": "12:00", "duration": "06:00", "distance": "",
                 "classes": ["SL"], "train_type": "", "runs_on": [true,true,true,true,true,true,true]},
                {"number": "11111", "name": "RICH EXP", "from_code": "SC",
                 "from_name": "SECUNDERABAD", "to_code": "PUNE", "to_name": "PUNE JN",
                 "departure_time": "08:00",
                 "arrival_time": "14:00", "duration": "06:00", "distance": "",
                 "classes": ["SL"], "train_type": "", "runs_on": [true,true,true,true,true,true,true],
                 "availability": many_classes},
                {"number": "22222", "name": "FARELESS EXP", "from_code": "SC",
                 "from_name": "SECUNDERABAD", "to_code": "PUNE", "to_name": "PUNE JN",
                 "departure_time": "09:00",
                 "arrival_time": "15:00", "duration": "06:00", "distance": "",
                 "classes": ["3A"], "train_type": "", "runs_on": [true,true,true,true,true,true,true],
                 "availability": [{"class": "3A", "status": "RAC 5"}]}
            ]
        }))
        .unwrap();
        let out = project_seat_availability(&dto);
        assert_eq!(out["from"], "SC");
        assert_eq!(out["date"], "2026-08-22");

        let rows = out["trains"].as_array().unwrap();
        assert_eq!(rows.len(), 3, "all trains kept when under the cap");
        assert_eq!(
            rows[0]["number"], "11111",
            "trains with class-wise status rank first"
        );
        assert_eq!(rows[2]["number"], "00001", "bare train ranks last");

        // Class list is capped and only Some-fields materialize as keys.
        let classes = rows[0]["classes"].as_array().unwrap();
        assert_eq!(classes.len(), MAX_AVAILABILITY_CLASSES);
        assert_eq!(classes[0]["tone"], "ok");
        assert_eq!(classes[1]["fare"], json!(501));
        let bare_class = &rows[1]["classes"][0];
        assert_eq!(bare_class["tone"], "warn");
        assert!(bare_class.get("fare").is_none());
        assert!(bare_class.get("prediction").is_none());
    }

    #[test]
    fn journey_date_accepts_known_formats_defaults_today() {
        let today = today_ist();
        assert_eq!(resolve_journey_date(&json!({})).unwrap(), today);
        assert_eq!(resolve_journey_date(&json!({"date": ""})).unwrap(), today);
        assert_eq!(
            resolve_journey_date(&json!({"date": "today"})).unwrap(),
            today
        );
        assert_eq!(
            resolve_journey_date(&json!({"date": "20/10/2026"})).unwrap(),
            "2026-10-20"
        );
        assert_eq!(
            resolve_journey_date(&json!({"date": "20261020"})).unwrap(),
            "2026-10-20"
        );
        assert!(resolve_journey_date(&json!({"date": "not-a-date"})).is_err());
    }
}
