use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use reqwest::header::{COOKIE, REFERER, SET_COOKIE};
use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::{AppError, CaptchaRequiredError};
use crate::core::http::HttpClient;
use crate::data::StationRecord;
use crate::models::{PnrEndpoint, PnrPassenger, PnrResponse};
use crate::state::AppState;
use base64::Engine;

/// Government source for PNR status (https://www.indianrail.gov.in).
///
/// The official enquiry is captcha-gated: a session is established on the
/// PnrEnquiry page, a `captchaDraw.png` image is bound to that session, and
/// the user's answer is exchanged against `/enquiry/CommonCaptcha`. This
/// service surfaces that challenge to the client as HTTP 428; the client
/// answers and retries with `captcha_session` / `captcha_text`.
const SOURCE: &str = "Indian Railways";
const ENQUIRY_REFERER: &str = "https://www.indianrail.gov.in/enquiry/PNR/PnrEnquiry.html?locale=en";

static SESSION_SEQ: AtomicU64 = AtomicU64::new(0);

/// User-submitted captcha answer tied to the session that raised the 428.
pub struct CaptchaAnswer {
    pub session_id: String,
    pub text: String,
    pub source: String,
}

/// Server-side state bound to a captcha challenge (stored in the shared
/// cache under `pnr_sess:<session_id>` so it expires automatically).
#[derive(serde::Serialize, serde::Deserialize)]
struct CaptchaSession {
    pnr: String,
    cookies: Vec<(String, String)>,
}

pub struct Service;

impl Service {
    /// Resolve live PNR status.
    ///
    /// `captcha` is present when the client is answering a previous 428
    /// challenge; `text == "REFRESH"` requests a fresh challenge image.
    pub async fn get_status(
        state: &AppState,
        pnr: &str,
        captcha: Option<CaptchaAnswer>,
    ) -> Result<PnrResponse, AppError> {
        if captcha.is_none() {
            let key = keys::pnr(pnr);
            if let Some(v) = state.cache.get(&key) {
                if let Ok(r) = map_response(pnr, &v, &state.datasets.stations) {
                    return Ok(r);
                }
            }
        }

        if state.failover.should_skip("indian-railways") {
            return Err(AppError::source_unavailable(
                SOURCE,
                "circuit open — indian-railways temporarily unavailable (cooldown)",
            ));
        }

        match captcha {
            None => Self::challenge(state, pnr).await,
            Some(answer) => Self::answer(state, pnr, &answer).await,
        }
    }

    /// Establish a fresh session + captcha challenge and surface it as 428.
    async fn challenge(state: &AppState, pnr: &str) -> Result<PnrResponse, AppError> {
        let start = Instant::now();
        let base = &state.config.ir_base;

        // 1. Load the enquiry page to pick up the session cookies (JSESSIONID,
        //    TS*, f5 cookies, IR_APP).
        let page = send_get(
            &state.http,
            &state
                .config
                .source_url(base, "/enquiry/PNR/PnrEnquiry.html?locale=en"),
            None,
        )
        .await
        .map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state.failover.record_failure("indian-railways");
            }
            e
        })?;
        let mut cookies = capture_cookies(&page);

        // 2. Ask whether the image captcha is enabled (0 = off, 1 = on).
        let cfg_res = send_get(
            &state.http,
            &state.config.source_url(base, "/enquiry/CaptchaConfig"),
            Some(&cookie_str(&cookies)),
        )
        .await
        .map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state.failover.record_failure("indian-railways");
            }
            e
        })?;
        cookies.extend(capture_cookies(&cfg_res));
        let cfg_text = cfg_res.text().await.unwrap_or_default().trim().to_string();

        // 3. Fetch the captcha image bound to this session.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let img_res = send_get(
            &state.http,
            &state
                .config
                .source_url(base, &format!("/enquiry/captchaDraw.png?{ts}")),
            Some(&cookie_str(&cookies)),
        )
        .await
        .map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state.failover.record_failure("indian-railways");
            }
            e
        })?;
        cookies.extend(capture_cookies(&img_res));
        let img = img_res.bytes().await.map_err(|e| {
            AppError::source_unavailable(SOURCE, format!("captcha image body: {e}"))
        })?;
        state.failover.record_success("indian-railways");

        let session_id = new_session_id();
        let session = CaptchaSession {
            pnr: pnr.to_string(),
            cookies,
        };
        state.cache.set(
            &format!("pnr_sess:{session_id}"),
            serde_json::to_value(&session)?,
        );

        tracing::info!(
            %pnr,
            source = SOURCE,
            captcha_config = %cfg_text,
            latency_ms = start.elapsed().as_millis(),
            "pnr lookup: captcha challenge issued"
        );

        let image = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&img)
        );
        Err(AppError::CaptchaRequired(CaptchaRequiredError::new(
            SOURCE, image, session_id,
        )))
    }

    /// Exchange a captcha answer for the PNR status.
    async fn answer(
        state: &AppState,
        pnr: &str,
        captcha: &CaptchaAnswer,
    ) -> Result<PnrResponse, AppError> {
        if captcha.text.is_empty() {
            return Err(AppError::bad_request(
                "captcha_text is required to answer the challenge.",
            ));
        }

        let sess_key = format!("pnr_sess:{}", captcha.session_id);
        let session: Option<CaptchaSession> = state
            .cache
            .get(&sess_key)
            .and_then(|v| serde_json::from_value(v).ok());
        state.cache.remove(&sess_key);

        let cookies = match session {
            Some(s) if s.pnr == pnr => s.cookies,
            Some(_) => {
                return Err(AppError::bad_request(
                    "Captcha session was created for a different PNR. Start a fresh enquiry.",
                ));
            }
            None => {
                tracing::info!(
                    session = %captcha.session_id,
                    "pnr lookup: captcha session expired or unknown, issuing a fresh challenge"
                );
                return Self::challenge(state, pnr).await;
            }
        };

        let start = Instant::now();
        let base = &state.config.ir_base;
        let query = format!(
            "/enquiry/CommonCaptcha?inputCaptcha={}&inputPnrNo={}&inputPage=PNR&language=en",
            urlencoding::encode(captcha.text.trim()),
            pnr
        );
        let res = send_get(
            &state.http,
            &state.config.source_url(base, &query),
            Some(&cookie_str(&cookies)),
        )
        .await
        .map_err(|e| {
            if matches!(
                e,
                AppError::SourceUnavailable { .. } | AppError::Internal(_)
            ) {
                state.failover.record_failure("indian-railways");
            }
            e
        })?;
        let body: Value = serde_json::from_slice(&res.bytes().await.map_err(|e| {
            AppError::source_unavailable(SOURCE, format!("CommonCaptcha body: {e}"))
        })?)
        .map_err(|e| {
            AppError::source_unavailable(SOURCE, format!("invalid CommonCaptcha JSON: {e}"))
        })?;
        state.failover.record_success("indian-railways");

        let error_message = body
            .get("errorMessage")
            .and_then(Value::as_str)
            .unwrap_or("");
        let flag = body.get("flag").and_then(Value::as_str).unwrap_or("");

        if !error_message.is_empty() || flag.eq_ignore_ascii_case("NO") {
            match error_message {
                "Captcha not matched" => {
                    tracing::info!(
                        %pnr,
                        source = SOURCE,
                        latency_ms = start.elapsed().as_millis(),
                        "pnr lookup: captcha not matched, issuing a fresh challenge"
                    );
                    Self::challenge(state, pnr).await
                }
                "Session out or Invalid Request" => {
                    tracing::info!(
                        %pnr,
                        source = SOURCE,
                        latency_ms = start.elapsed().as_millis(),
                        "pnr lookup: upstream session expired, issuing a fresh challenge"
                    );
                    Self::challenge(state, pnr).await
                }
                "PNR No. is not valid" | "Invalid PNR" => {
                    tracing::info!(
                        %pnr,
                        source = SOURCE,
                        latency_ms = start.elapsed().as_millis(),
                        "pnr lookup: no booking found"
                    );
                    Err(AppError::not_found(format!(
                        "No booking found for PNR {pnr}."
                    )))
                }
                other => {
                    tracing::warn!(
                        %pnr,
                        source = SOURCE,
                        latency_ms = start.elapsed().as_millis(),
                        %other,
                        "pnr lookup: upstream rejected the request"
                    );
                    Err(AppError::source_unavailable(SOURCE, other.to_string()))
                }
            }
        } else {
            let resp = map_response(pnr, &body, &state.datasets.stations)?;
            tracing::info!(
                %pnr,
                source = SOURCE,
                latency_ms = start.elapsed().as_millis(),
                train = %resp.train_number.as_deref().unwrap_or("-"),
                "pnr lookup resolved"
            );
            state
                .cache
                .set(&keys::pnr(pnr), serde_json::to_value(&resp)?);
            Ok(resp)
        }
    }
}

/// Map a successful `/enquiry/CommonCaptcha` payload onto `PnrResponse`.
///
/// Field names mirror the official `showPnr()`/`drawRow()` JavaScript on
/// https://www.indianrail.gov.in/enquiry/PNR/PnrEnquiry.html.
fn map_response(
    pnr: &str,
    body: &Value,
    stations: &[StationRecord],
) -> Result<PnrResponse, AppError> {
    if body.get("trainNumber").is_none() && body.get("passengerList").is_none() {
        return Err(AppError::source_unavailable(
            SOURCE,
            "unexpected CommonCaptcha response shape",
        ));
    }

    let chart_status = body
        .get("chartStatus")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let notice = if chart_status.is_empty() {
        "Live data from Indian Railways.".to_string()
    } else {
        format!("Live data from Indian Railways. Chart: {chart_status}.")
    };

    Ok(PnrResponse {
        pnr: Some(pnr.to_string()),
        train_number: body
            .get("trainNumber")
            .and_then(Value::as_str)
            .map(str::to_string),
        train_name: body
            .get("trainName")
            .and_then(Value::as_str)
            .map(str::to_string),
        journey_date: body
            .get("dateOfJourney")
            .and_then(Value::as_str)
            .map(normalize_date),
        from: body
            .get("sourceStation")
            .and_then(Value::as_str)
            .map(|s| endpoint(s, stations)),
        to: body
            .get("destinationStation")
            .and_then(Value::as_str)
            .map(|s| endpoint(s, stations)),
        passengers: Some(passengers(body.get("passengerList"))),
        last_updated: body
            .get("generatedTimeStamp")
            .and_then(timestamp)
            .or_else(|| Some(chrono::Utc::now().to_rfc3339())),
        freshness: Some("live".to_string()),
        notice: Some(notice),
        data_source: Some(SOURCE.to_string()),
    })
}

/// Resolve `"AK - AKOLA JN"` into code + name; a bare code (`"AK"`) is looked
/// up in the local station dataset for its real name.
fn endpoint(v: &str, stations: &[StationRecord]) -> PnrEndpoint {
    let v = v.trim();
    let (code, name) = match v.split_once(" - ") {
        Some((code, name)) => (code.trim().to_string(), name.trim().to_string()),
        None => {
            if let Some(s) = stations.iter().find(|s| s.code.eq_ignore_ascii_case(v)) {
                (s.code.clone(), s.name.clone())
            } else {
                (String::new(), v.to_string())
            }
        }
    };
    PnrEndpoint {
        code,
        name,
        time: String::new(),
        day: 0,
    }
}

/// Replicate the official status composition:
/// `<status>[/<coach>][/<berth>][/<berthCode>]`.
fn compose(
    status: &str,
    coach: &str,
    berth: i64,
    berth_code: Option<&str>,
    show_coach: bool,
) -> String {
    let mut out = status.to_string();
    if show_coach && !coach.is_empty() {
        out.push('/');
        out.push_str(coach);
    }
    let berth_worth_showing =
        (status != "CNF" && status != "CAN") || (status == "CNF" && berth != 0);
    if berth_worth_showing && berth != 0 {
        out.push('/');
        out.push_str(&berth.to_string());
    }
    if let Some(code) = berth_code {
        if !code.is_empty() && code != "-1" {
            out.push('/');
            out.push_str(code);
        }
    }
    out
}

fn passengers(v: Option<&Value>) -> Vec<PnrPassenger> {
    v.and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|p| {
                    let booking_status =
                        p.get("bookingStatus").and_then(Value::as_str).unwrap_or("");
                    let booking_coach = p
                        .get("bookingCoachId")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let booking_berth =
                        p.get("bookingBerthNo").and_then(Value::as_i64).unwrap_or(0);
                    let booking_code = p.get("bookingBerthCode").and_then(Value::as_str);
                    let quota = p
                        .get("passengerQuota")
                        .and_then(Value::as_str)
                        .unwrap_or("");

                    let mut booking_status = compose(
                        booking_status,
                        booking_coach,
                        booking_berth,
                        booking_code,
                        true,
                    );
                    if !quota.is_empty() {
                        booking_status.push('/');
                        booking_status.push_str(quota);
                    }

                    let current_status =
                        p.get("currentStatus").and_then(Value::as_str).unwrap_or("");
                    let current_coach = p
                        .get("currentCoachId")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let current_berth =
                        p.get("currentBerthNo").and_then(Value::as_i64).unwrap_or(0);
                    let current_status =
                        compose(current_status, current_coach, current_berth, None, true);

                    let coach = if !current_coach.is_empty() {
                        current_coach.to_string()
                    } else {
                        booking_coach.to_string()
                    };
                    let berth = if current_berth != 0 {
                        current_berth.to_string()
                    } else if booking_berth != 0 {
                        booking_berth.to_string()
                    } else {
                        String::new()
                    };

                    PnrPassenger {
                        booking_status,
                        coach,
                        berth,
                        current_status,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `generatedTimeStamp` -> RFC3339 in IST.
fn timestamp(v: &Value) -> Option<String> {
    let get = |k: &str| v.get(k).and_then(Value::as_i64);
    let (y, mo, d, h, mi, s) = (
        get("year")?,
        get("month")?,
        get("day")?,
        get("hour")?,
        get("minute")?,
        get("second")?,
    );
    Some(format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}+05:30"))
}

/// Normalize the official `dateOfJourney` (e.g. `"Aug 12, 2026 5:40:00 PM"`,
/// `"2026-08-12"`, `"12-08-2026"`) into `YYYY-MM-DD`.
fn normalize_date(s: &str) -> String {
    let s = s.trim();
    for fmt in [
        "%Y-%m-%d",
        "%d-%m-%Y",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%b %d, %Y %I:%M:%S %p",
        "%b %d, %Y %I:%M %p",
        "%b %d, %Y",
    ] {
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return d.date().to_string();
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return d.to_string();
        }
    }
    s.to_string()
}

/// GET with the enquiry referer and (optionally) session cookies.
async fn send_get(
    http: &HttpClient,
    url: &str,
    cookies: Option<&str>,
) -> Result<reqwest::Response, AppError> {
    let mut req = http.inner().get(url).header(REFERER, ENQUIRY_REFERER);
    if let Some(c) = cookies {
        req = req.header(COOKIE, c);
    }
    let res = req
        .send()
        .await
        .map_err(|e| AppError::source_unavailable(SOURCE, format!("{url}: {e}")))?;
    if !res.status().is_success() {
        return Err(AppError::source_unavailable(
            SOURCE,
            format!("{url} returned {}", res.status()),
        ));
    }
    Ok(res)
}

/// Collect `Set-Cookie` header values into `(name, value)` pairs.
fn capture_cookies(res: &reqwest::Response) -> Vec<(String, String)> {
    res.headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|s| s.split(';').next())
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                None
            } else {
                Some((name.to_string(), value.trim().to_string()))
            }
        })
        .collect()
}

fn cookie_str(cookies: &[(String, String)]) -> String {
    cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn new_session_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let seq = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}{seq:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_official_date_formats() {
        assert_eq!(normalize_date("Aug 12, 2026 5:40:00 PM"), "2026-08-12");
        assert_eq!(normalize_date("2026-08-12"), "2026-08-12");
        assert_eq!(normalize_date("12-08-2026"), "2026-08-12");
        assert_eq!(normalize_date("12/08/2026"), "2026-08-12");
    }

    #[test]
    fn endpoint_splits_code_and_name() {
        let stations = vec![StationRecord {
            code: "AK".into(),
            name: "AKOLA JN".into(),
            state: String::new(),
            zone: String::new(),
            ..StationRecord::default()
        }];
        let e = endpoint("MMCT - MUMBAI CENTRAL", &stations);
        assert_eq!(e.code, "MMCT");
        assert_eq!(e.name, "MUMBAI CENTRAL");
        let e = endpoint("AK", &stations);
        assert_eq!(e.code, "AK");
        assert_eq!(e.name, "AKOLA JN");
        let e = endpoint("SOMEWHERE ELSE", &stations);
        assert_eq!(e.code, "");
        assert_eq!(e.name, "SOMEWHERE ELSE");
    }

    #[test]
    fn composes_official_status_strings() {
        let p = |booking_status: &str,
                 booking_coach: &str,
                 booking_berth: i64,
                 current_status: &str,
                 current_coach: &str,
                 current_berth: i64| {
            PnrPassenger {
                booking_status: compose(
                    booking_status,
                    booking_coach,
                    booking_berth,
                    Some("SL"),
                    true,
                ),
                coach: if current_coach.is_empty() {
                    booking_coach.to_string()
                } else {
                    current_coach.to_string()
                },
                berth: if current_berth != 0 {
                    current_berth.to_string()
                } else if booking_berth != 0 {
                    booking_berth.to_string()
                } else {
                    String::new()
                },
                current_status: compose(current_status, current_coach, current_berth, None, true),
            }
        };
        let p = p("CNF", "B2", 47, "CNF", "B2", 47);
        assert_eq!(p.booking_status, "CNF/B2/47/SL");
        assert_eq!(p.current_status, "CNF/B2/47");
        assert_eq!(p.coach, "B2");
        assert_eq!(p.berth, "47");
    }
}
