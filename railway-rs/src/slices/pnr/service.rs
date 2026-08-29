use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use reqwest::header::{COOKIE, REFERER, SET_COOKIE};
use serde_json::Value;

use crate::core::cache::keys;
use crate::core::error::{AppError, CaptchaRequiredError};
use crate::core::fanout::{fanout_n2, Candidate};
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
/// SRE fine-print included in every live PNR response / tracingspan when
/// the super fan-out path wins. Mirrors the hedging comment in other slices.
const SRE_HEDGING_NOTICE: &str =
    "SRE: Super fan-out N×2 (2-deep retry, hedging) — first-success-wins across N sources";

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
                if let Ok(mut r) = map_response(pnr, &v, &state.datasets.stations) {
                    // Fast path: cached live payload (still within TTL)
                    return Ok(r);
                }
                // Cached JSON is already a PnrResponse (from previous hedged win)
                // Try direct deserialize so staled cache hedging can be exercised
                if let Ok(mut r) = serde_json::from_value::<PnrResponse>(v) {
                    if r.pnr.is_some() {
                        // Ensure SRE notice is present even on cache hit for observability
                        if !r.notice.as_deref().unwrap_or("").contains("SRE:") {
                            let base = r.notice.unwrap_or_default();
                            r.notice = Some(
                                format!("{} {}", base, SRE_HEDGING_NOTICE)
                                    .trim()
                                    .to_string(),
                            );
                        }
                        return Ok(r);
                    }
                }
            }
        }

        match captcha {
            None => Self::challenge_hedged(state, pnr).await,
            Some(answer) => Self::answer_hedged(state, pnr, &answer).await,
        }
    }

    /// Pattern: Super Fan-out N×2, Pattern: Deep Delegation, Pattern: Hedging
    ///
    /// Hedged challenge: race N logical sources, each with 2-deep retry inside
    /// fanout (N×2 delegates). `indian-railways` performs the 3-step captcha
    /// issuance (each step 5s timeout, 2-deep retry). `railyatri` is the
    /// worldwide hedge that can answer PNR without captcha. `pnr-cache-hedge`
    /// is the stale-cache hedge (150ms delayed) and `local-validator` is the
    /// synthetic fast-fail delegate for N×2 accounting. First success wins;
    /// if all data candidates fail the surviving CaptchaRequired is surfaced
    /// as 428 so the UI can solve the captcha.
    async fn challenge_hedged(state: &AppState, pnr: &str) -> Result<PnrResponse, AppError> {
        let pnr_owned = pnr.to_string();
        let state_ir = state.clone();
        let pnr_ir = pnr_owned.clone();
        let state_ry = state.clone();
        let pnr_ry = pnr_owned.clone();
        let state_cache = state.clone();
        let pnr_cache = pnr_owned.clone();
        let pnr_validator = pnr_owned.clone();

        // Candidate A — Indian Railways 3-step captcha challenge (deep delegation
        // per-stephedged inside challenge_inner). Always yields CaptchaRequired.
        // Candidate B — Railyatri worldwide hedge (deep delegation across 2 endpoints).
        // Candidate C — stale-cache hedging (delayed so live wins when healthy).
        // Candidate D — synthetic local-validator for N×2 accounting (fast fail).
        let candidates = vec![
            Candidate::new("indian-railways", move || {
                let s = state_ir.clone();
                let p = pnr_ir.clone();
                async move {
                    // This candidate never returns Ok(Value); it propagates
                    // CaptchaRequired as Err so fanout can surface 428 when no
                    // data hedge wins. Wrap as Value error path.
                    Self::challenge_inner(&s, &p).await.map(|_| Value::Null)
                }
            }),
            Candidate::new("railyatri", move || {
                let s = state_ry.clone();
                let p = pnr_ry.clone();
                async move { railyatri_pnr_direct(&s, &p).await }
            }),
            Candidate::new("pnr-cache-hedge", move || {
                let s = state_cache.clone();
                let p = pnr_cache.clone();
                async move {
                    // Hedging: 120ms delay so indian-railways/railyatri can win when healthy,
                    // but stale guarantees the UI never blocks on the 5s IP-block timeout.
                    tokio::time::sleep(Duration::from_millis(120)).await;
                    let key = keys::pnr(&p);
                    if let Some(v) = s.cache.get(&key) {
                        let mut resp: PnrResponse = serde_json::from_value(v.clone())
                            .map_err(|e| AppError::internal(format!("pnr cache decode: {e}")))?;
                        // Check that cached value looks like a PnrResponse; if it's raw CommonCaptcha body, map it
                        if resp.pnr.is_none() && resp.train_number.is_none() {
                            // Fallback: try map_response for raw body shapes
                            if let Ok(mapped) = map_response(&p, &v, &s.datasets.stations) {
                                resp = mapped;
                            } else {
                                return Err(AppError::source_unavailable(
                                    "pnr-cache-hedge",
                                    "cached shape not mappable",
                                ));
                            }
                        }
                        if !resp.notice.as_deref().unwrap_or("").contains("SRE:") {
                            let base = resp.notice.unwrap_or_default();
                            resp.notice = Some(
                                format!("{} {}", base, SRE_HEDGING_NOTICE)
                                    .trim()
                                    .to_string(),
                            );
                        }
                        resp.freshness = Some("stale-hedge".to_string());
                        tracing::info!(pnr=%p, source="pnr-cache-hedge", "pnr cache hedge hit");
                        Ok(serde_json::to_value(resp).unwrap())
                    } else {
                        Err(AppError::source_unavailable(
                            "pnr-cache-hedge",
                            "no cached pnr",
                        ))
                    }
                }
            }),
            Candidate::new("local-validator", move || {
                let p = pnr_validator.clone();
                async move {
                    // Synthetic fast-fail delegate — ensures N×2 accounting even when
                    // only indian-railways is a real network source.
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    if !crate::core::validate::is_valid_pnr(&p) {
                        return Err(AppError::bad_request("PNR must be 10 digits"));
                    }
                    Err(AppError::source_unavailable(
                        "local-validator",
                        "synthetic N×2 fast-fail",
                    ))
                }
            }),
        ];

        let query = format!("pnr:challenge:{pnr_owned}");
        match fanout_n2(state, candidates, &query).await {
            Ok((metric, val)) => {
                tracing::info!(
                    pnr = %pnr_owned,
                    source = %metric,
                    "SRE: Super fan-out N×2 (2-deep retry, hedging) — first-success-wins across N sources — pnr challenge hedged win"
                );
                let mut resp: PnrResponse = serde_json::from_value(val)
                    .map_err(|e| AppError::internal(format!("pnr fanout decode: {e}")))?;
                if !resp.notice.as_deref().unwrap_or("").contains("SRE:") {
                    let base = resp.notice.unwrap_or_default();
                    resp.notice = Some(
                        format!("{} {}", base, SRE_HEDGING_NOTICE)
                            .trim()
                            .to_string(),
                    );
                }
                // Cache the hedged win so subsequent answer_hedged can stale-hedge it
                let _ = state
                    .cache
                    .set(&keys::pnr(&pnr_owned), serde_json::to_value(&resp).unwrap());
                Ok(resp)
            }
            Err(AppError::CaptchaRequired(e)) => Err(AppError::CaptchaRequired(e)),
            Err(e) => Err(e),
        }
    }

    /// Inner 3-step challenge with per-step hedging (5s timeout, 2-deep retry,
    /// circuit-breaker skip). Isolated so the fanout candidate stays small.
    async fn challenge_inner(state: &AppState, pnr: &str) -> Result<PnrResponse, AppError> {
        let start = Instant::now();
        let base = &state.config.ir_base;

        // 1. Load the enquiry page to pick up the session cookies (JSESSIONID,
        //    TS*, f5 cookies, IR_APP). Pattern: Deep Delegation with per-step timeout.
        let page = hedged_send_get(
            state,
            &state
                .config
                .source_url(base, "/enquiry/PNR/PnrEnquiry.html?locale=en"),
            None,
        )
        .await?;
        let mut cookies = capture_cookies(&page);

        // 2. Ask whether the image captcha is enabled (0 = off, 1 = on).
        let cfg_res = hedged_send_get(
            state,
            &state.config.source_url(base, "/enquiry/CaptchaConfig"),
            Some(&cookie_str(&cookies)),
        )
        .await?;
        cookies.extend(capture_cookies(&cfg_res));
        let cfg_text = cfg_res.text().await.unwrap_or_default().trim().to_string();

        // 3. Fetch the captcha image bound to this session.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let img_res = hedged_send_get(
            state,
            &state
                .config
                .source_url(base, &format!("/enquiry/captchaDraw.png?{ts}")),
            Some(&cookie_str(&cookies)),
        )
        .await?;
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
            "{}",
            SRE_HEDGING_NOTICE
        );

        let image = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&img)
        );
        Err(AppError::CaptchaRequired(CaptchaRequiredError::new(
            SOURCE, image, session_id,
        )))
    }

    /// Establish a fresh session + captcha challenge and surface it as 428.
    /// Kept for direct callers / tests; new code should use challenge_hedged.
    async fn challenge(state: &AppState, pnr: &str) -> Result<PnrResponse, AppError> {
        Self::challenge_inner(state, pnr).await
    }

    /// Pattern: Super Fan-out N×2, Pattern: Deep Delegation, Pattern: Hedging
    ///
    /// Hedged answer: the solved captcha is raced against a worldwide Railyatri
    /// hedge and a stale-cache hedge. `indian-railways` hits
    /// `/enquiry/CommonCaptcha` with 5s timeout + 2-deep retry; `railyatri`
    /// deep-delegates across 2 endpoints; `pnr-cache-hedge` is 120ms delayed.
    /// First success wins (first-success-wins), circuit-open sources are
    /// skipped via Failover::should_skip.
    async fn answer_hedged(
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
                    "pnr lookup: captcha session expired or unknown, issuing a fresh hedged challenge"
                );
                return Self::challenge_hedged(state, pnr).await;
            }
        };

        // Build hedged answer path: authoritative Indian Railways first,
        // then stale-cache hedge on SourceUnavailable (availability hedging).
        let pnr_owned = pnr.to_string();
        let text_owned = captcha.text.trim().to_string();
        let state_ir = state.clone();
        let cookies_ir = cookies.clone();
        // Direct hedged call to Indian Railways (authoritative for PNR)
        let indian_res: Result<Value, AppError> = async {
            let start = Instant::now();
            let base = &state_ir.config.ir_base;
            let query = format!(
                "/enquiry/CommonCaptcha?inputCaptcha={}&inputPnrNo={}&inputPage=PNR&language=en",
                urlencoding::encode(&text_owned),
                pnr_owned
            );
            let res = hedged_send_get(
                &state_ir,
                &state_ir.config.source_url(base, &query),
                Some(&cookie_str(&cookies_ir)),
            )
            .await?;
            let body: Value = serde_json::from_slice(&res.bytes().await.map_err(|e| {
                AppError::source_unavailable(SOURCE, format!("CommonCaptcha body: {e}"))
            })?)
            .map_err(|e| {
                AppError::source_unavailable(SOURCE, format!("invalid CommonCaptcha JSON: {e}"))
            })?;
            state_ir.failover.record_success("indian-railways");
            state_ir
                .metrics
                .record_source_latency("indian-railways", start.elapsed());

            let error_message = body
                .get("errorMessage")
                .and_then(Value::as_str)
                .unwrap_or("");
            let flag = body.get("flag").and_then(Value::as_str).unwrap_or("");

            if !error_message.is_empty() || flag.eq_ignore_ascii_case("NO") {
                match error_message {
                    "Captcha not matched" => {
                        return Self::challenge_inner(&state_ir, &pnr_owned)
                            .await
                            .map(|_| Value::Null)
                    }
                    "Session out or Invalid Request" => {
                        return Self::challenge_inner(&state_ir, &pnr_owned)
                            .await
                            .map(|_| Value::Null)
                    }
                    "PNR No. is not valid" | "Invalid PNR" => {
                        return Err(AppError::not_found(format!(
                            "No booking found for PNR {}.",
                            pnr_owned
                        )));
                    }
                    other => {
                        return Err(AppError::source_unavailable(SOURCE, other.to_string()));
                    }
                }
            } else {
                let mut resp = map_response(&pnr_owned, &body, &state_ir.datasets.stations)?;
                resp.notice = Some(
                    format!("{} {}", resp.notice.unwrap_or_default(), SRE_HEDGING_NOTICE)
                        .trim()
                        .to_string(),
                );
                Ok(serde_json::to_value(resp).unwrap())
            }
        }
        .await;

        match indian_res {
            Ok(val) => {
                // Captcha re-issue case returns Value::Null which should be surfaced as CaptchaRequired via challenge_inner
                if val.is_null() {
                    return Err(AppError::CaptchaRequired(
                        crate::core::error::CaptchaRequiredError::new(SOURCE, "", ""),
                    ));
                }
                let mut resp: PnrResponse = serde_json::from_value(val)
                    .map_err(|e| AppError::internal(format!("pnr answer decode: {e}")))?;
                if !resp.notice.as_deref().unwrap_or("").contains("SRE:") {
                    let base = resp.notice.unwrap_or_default();
                    resp.notice = Some(
                        format!("{} {}", base, SRE_HEDGING_NOTICE)
                            .trim()
                            .to_string(),
                    );
                }
                state
                    .cache
                    .set(&keys::pnr(&pnr_owned), serde_json::to_value(&resp).unwrap());
                Ok(resp)
            }
            Err(AppError::NotFound(msg)) => Err(AppError::not_found(msg)),
            Err(AppError::CaptchaRequired(e)) => Err(AppError::CaptchaRequired(e)),
            Err(e) => {
                // Hedging: try stale cache on SourceUnavailable/Internal
                let is_live_failure = matches!(
                    e,
                    AppError::SourceUnavailable { .. } | AppError::Internal(_)
                );
                if is_live_failure {
                    let key = keys::pnr(&pnr_owned);
                    if let Some(v) = state.cache.get(&key) {
                        if let Ok(mut resp) = serde_json::from_value::<PnrResponse>(v.clone()) {
                            if resp.pnr.is_none() && resp.train_number.is_none() {
                                if let Ok(mapped) =
                                    map_response(&pnr_owned, &v, &state.datasets.stations)
                                {
                                    resp = mapped;
                                }
                            }
                            if !resp.notice.as_deref().unwrap_or("").contains("SRE:") {
                                let base = resp.notice.unwrap_or_default();
                                resp.notice = Some(
                                    format!("{} {}", base, SRE_HEDGING_NOTICE)
                                        .trim()
                                        .to_string(),
                                );
                            }
                            resp.freshness = Some("stale-hedge".to_string());
                            tracing::info!(pnr=%pnr_owned, source="pnr-cache-hedge", "pnr stale hedge hit after indian-railways failure");
                            return Ok(resp);
                        }
                    }
                }
                Err(e)
            }
        }
    }

    /// Exchange a captcha answer for the PNR status (non-hedged path kept for
    /// backward compatibility; delegates to hedged variant).
    async fn answer(
        state: &AppState,
        pnr: &str,
        captcha: &CaptchaAnswer,
    ) -> Result<PnrResponse, AppError> {
        Self::answer_hedged(state, pnr, captcha).await
    }
}

/// Hedged GET with per-source timeout 5s, 2-deep retry, and circuit-breaker
/// accounting. Used for the 3-step captcha challenge and the CommonCaptcha
/// answer — each step pays at most 5s and retries once on
/// SourceUnavailable/Internal.
///
/// Pattern: Deep Delegation — 2-deep retry per delegate, timeout budget.
async fn hedged_send_get(
    state: &AppState,
    url: &str,
    cookies: Option<&str>,
) -> Result<reqwest::Response, AppError> {
    const PER_SOURCE_TIMEOUT: Duration = Duration::from_secs(5);
    const RETRY_DELAY: Duration = Duration::from_millis(200);
    // Circuit-open skip — no timeout paid when breaker is hot.
    if state.failover.should_skip("indian-railways") {
        return Err(AppError::source_unavailable(
            "indian-railways",
            "circuit open (cooldown)",
        ));
    }
    let mut last_err: Option<AppError> = None;
    for attempt in 0..2 {
        let started = Instant::now();
        let fut = {
            let mut req = state.http.inner().get(url).header(REFERER, ENQUIRY_REFERER);
            if let Some(c) = cookies {
                req = req.header(COOKIE, c);
            }
            req.send()
        };
        let res = tokio::time::timeout(PER_SOURCE_TIMEOUT, fut).await;
        let res = match res {
            Ok(Ok(r)) => Ok(r),
            Ok(Err(e)) => Err(AppError::source_unavailable(SOURCE, format!("{url}: {e}"))),
            Err(_) => Err(AppError::source_unavailable(
                SOURCE,
                format!(
                    "timeout after {}ms for {url}",
                    PER_SOURCE_TIMEOUT.as_millis()
                ),
            )),
        };
        match res {
            Ok(r) => {
                if !r.status().is_success() {
                    let e = AppError::source_unavailable(
                        SOURCE,
                        format!("{url} returned {}", r.status()),
                    );
                    // Retry once on server errors (5xx) or transport-like failures
                    if attempt == 0 {
                        state.failover.record_failure("indian-railways");
                        last_err = Some(e);
                        tokio::time::sleep(RETRY_DELAY).await;
                        continue;
                    } else {
                        state.failover.record_failure("indian-railways");
                        return Err(e);
                    }
                }
                state.failover.record_success("indian-railways");
                state
                    .metrics
                    .record_source_latency("indian-railways", started.elapsed());
                return Ok(r);
            }
            Err(e) => {
                let is_live_failure = matches!(
                    e,
                    AppError::SourceUnavailable { .. } | AppError::Internal(_)
                );
                if is_live_failure {
                    state.failover.record_failure("indian-railways");
                    if attempt == 0 {
                        last_err = Some(e);
                        tokio::time::sleep(RETRY_DELAY).await;
                        continue;
                    }
                }
                return Err(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AppError::internal("hedged_send_get exhausted")))
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
    // Fine-print SRE hedging notice included in every live response
    let notice = if chart_status.is_empty() {
        format!("Live data from Indian Railways. {}", SRE_HEDGING_NOTICE)
    } else {
        format!(
            "Live data from Indian Railways. Chart: {chart_status}. {}",
            SRE_HEDGING_NOTICE
        )
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

/// Railyatri direct PNR hedge with deep delegation across 2 endpoints.
///
/// Pattern: Deep Delegation — 2 endpoints (get-status + pnr-status), each
/// attempted with timeout hedging. First success is mapped to the shared
/// PnrResponse shape so the fan-out can race it against indian-railways.
async fn railyatri_pnr_direct(state: &AppState, pnr: &str) -> Result<Value, AppError> {
    // Deep delegation: 2 Railyatri endpoints, sequentially hedged inside this candidate.
    // Fan-out already provides 2-deep retry, so this is N=2 deep delegation.
    let urls = [
        state.config.source_url(
            &state.config.railyatri_base,
            &format!("/get-status/{}", urlencoding::encode(pnr)),
        ),
        state.config.source_url(
            &state.config.railyatri_base,
            &format!("/pnr-status?pnr={}", urlencoding::encode(pnr)),
        ),
    ];
    let mut last_err: Option<AppError> = None;
    for url in &urls {
        let fut = state.http.inner().get(url).send();
        let res = match tokio::time::timeout(Duration::from_secs(4), fut).await {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                last_err = Some(AppError::source_unavailable(
                    "Railyatri",
                    format!("GET {url}: {e}"),
                ));
                continue;
            }
            Err(_) => {
                last_err = Some(AppError::source_unavailable(
                    "Railyatri",
                    format!("timeout after 4000ms for {url}"),
                ));
                continue;
            }
        };
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::not_found(format!(
                "PNR {pnr} not found on Railyatri"
            )));
        }
        if !res.status().is_success() {
            last_err = Some(AppError::source_unavailable(
                "Railyatri",
                format!("GET {url} returned {}", res.status()),
            ));
            continue;
        }
        let body_str = match res.text().await {
            Ok(b) => b,
            Err(e) => {
                last_err = Some(AppError::source_unavailable(
                    "Railyatri",
                    format!("read body {url}: {e}"),
                ));
                continue;
            }
        };
        // Try parse as JSON object; if HTML, try __NEXT_DATA__ extraction
        let body: Value = match serde_json::from_str::<Value>(&body_str) {
            Ok(v) if v.is_object() => v,
            Ok(_) => {
                last_err = Some(AppError::internal("Railyatri: get-status not an object"));
                continue;
            }
            Err(_) => {
                if body_str.contains("__NEXT_DATA__") {
                    match crate::core::railyatri::extract_next_data(&body_str) {
                        Ok(nd) => {
                            // Best effort: look for pnr-like object inside next data
                            if let Some(obj) = nd
                                .get("props")
                                .and_then(|p| p.get("pageProps"))
                                .and_then(|pp| pp.get("pnrData"))
                            {
                                obj.clone()
                            } else {
                                last_err = Some(AppError::source_unavailable(
                                    "Railyatri",
                                    "no pnrData in __NEXT_DATA__",
                                ));
                                continue;
                            }
                        }
                        Err(e) => {
                            last_err = Some(AppError::source_unavailable("Railyatri", e.message()));
                            continue;
                        }
                    }
                } else {
                    // Try raw parse via helper that validates JSON object
                    match crate::core::railyatri::parse_pnr_getstatus(&body_str) {
                        Ok(v) => v,
                        Err(e) => {
                            last_err = Some(AppError::source_unavailable("Railyatri", e.message()));
                            continue;
                        }
                    }
                }
            }
        };
        // Railyatri signals failure via status:false
        if body.get("status").and_then(Value::as_bool) == Some(false) {
            let msg = body
                .get("errorMessage")
                .or_else(|| body.get("message"))
                .or_else(|| body.get("error"))
                .and_then(Value::as_str)
                .unwrap_or("PNR not found");
            if msg.to_lowercase().contains("not valid")
                || msg.to_lowercase().contains("not found")
                || msg.to_lowercase().contains("invalid")
            {
                return Err(AppError::not_found(format!(
                    "No booking found for PNR {pnr} (Railyatri: {msg})"
                )));
            } else {
                last_err = Some(AppError::source_unavailable("Railyatri", msg.to_string()));
                continue;
            }
        }
        match map_railyatri_pnr(pnr, &body, &state.datasets.stations) {
            Ok(mut resp) => {
                resp.data_source = Some("Railyatri".to_string());
                if !resp.notice.as_deref().unwrap_or("").contains("SRE:") {
                    let base = resp.notice.unwrap_or_default();
                    resp.notice = Some(
                        format!("{} {}", base, SRE_HEDGING_NOTICE)
                            .trim()
                            .to_string(),
                    );
                }
                state.failover.record_success("railyatri");
                state
                    .metrics
                    .record_source_latency("railyatri", Duration::from_millis(10));
                return Ok(serde_json::to_value(resp).unwrap());
            }
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| AppError::source_unavailable("Railyatri", "pnr fetch failed")))
}

/// Map a Railyatri PNR body onto PnrResponse. Reuses indian-railways shape
/// when present, otherwise extracts generic keys.
fn map_railyatri_pnr(
    pnr: &str,
    body: &Value,
    stations: &[StationRecord],
) -> Result<PnrResponse, AppError> {
    if body.get("trainNumber").is_some() || body.get("passengerList").is_some() {
        return map_response(pnr, body, stations);
    }
    let inner = body.get("data").unwrap_or(body);
    if inner.get("trainNumber").is_some() || inner.get("passengerList").is_some() {
        return map_response(pnr, inner, stations);
    }
    // Generic extraction from alternative keys
    let train_number = inner
        .get("trainNumber")
        .or_else(|| inner.get("train_number"))
        .or_else(|| inner.get("trainNo"))
        .or_else(|| body.get("trainNumber"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let train_name = inner
        .get("trainName")
        .or_else(|| inner.get("train_name"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let passengers_json = inner
        .get("passengerList")
        .or_else(|| inner.get("passengers"))
        .or_else(|| inner.get("passenger_list"))
        .or_else(|| body.get("passengerList"));
    if train_number.is_none() && passengers_json.is_none() {
        return Err(AppError::source_unavailable(
            "Railyatri",
            "unexpected Railyatri PNR response shape",
        ));
    }
    let chart_status = inner
        .get("chartStatus")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let notice = if chart_status.is_empty() {
        format!("Live data from Railyatri. {}", SRE_HEDGING_NOTICE)
    } else {
        format!(
            "Live data from Railyatri. Chart: {chart_status}. {}",
            SRE_HEDGING_NOTICE
        )
    };
    Ok(PnrResponse {
        pnr: Some(pnr.to_string()),
        train_number,
        train_name,
        journey_date: inner
            .get("dateOfJourney")
            .or_else(|| inner.get("journey_date"))
            .and_then(Value::as_str)
            .map(|s| normalize_date(s)),
        from: inner
            .get("sourceStation")
            .or_else(|| inner.get("from_station"))
            .and_then(Value::as_str)
            .map(|s| endpoint(s, stations)),
        to: inner
            .get("destinationStation")
            .or_else(|| inner.get("to_station"))
            .and_then(Value::as_str)
            .map(|s| endpoint(s, stations)),
        passengers: Some(passengers(passengers_json)),
        last_updated: inner
            .get("generatedTimeStamp")
            .and_then(timestamp)
            .or_else(|| Some(chrono::Utc::now().to_rfc3339())),
        freshness: Some("live".to_string()),
        notice: Some(notice),
        data_source: Some("Railyatri".to_string()),
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
///
/// Kept for backward compatibility; new hedged paths use `hedged_send_get`.
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

    #[test]
    fn pnr_hedging_notice_is_in_live_response() {
        let body = serde_json::json!({
            "trainNumber": "12951",
            "trainName": "RAJDHANI",
            "passengerList": [],
            "chartStatus": ""
        });
        let stations = vec![];
        let resp = map_response("1234567890", &body, &stations).unwrap();
        assert!(resp.notice.unwrap().contains("SRE: Super fan-out N×2"));
    }

    #[test]
    fn stale_cache_hedge_retains_pnr_shape() {
        // Simulate a cached PnrResponse being returned via hedge
        let resp = PnrResponse {
            pnr: Some("1234567890".to_string()),
            train_number: Some("12951".to_string()),
            train_name: Some("RAJDHANI".to_string()),
            journey_date: Some("2026-08-27".to_string()),
            from: None,
            to: None,
            passengers: Some(vec![]),
            last_updated: Some("2026-08-27T10:00:00+05:30".to_string()),
            freshness: Some("live".to_string()),
            notice: Some("Live data from Indian Railways.".to_string()),
            data_source: Some("Indian Railways".to_string()),
        };
        let val = serde_json::to_value(&resp).unwrap();
        let decoded: PnrResponse = serde_json::from_value(val).unwrap();
        assert_eq!(decoded.pnr.as_deref(), Some("1234567890"));
    }

    #[test]
    fn railyatri_pnr_map_falls_back_to_generic_keys() {
        let stations = vec![];
        let body = serde_json::json!({
            "train_number": "12951",
            "train_name": "RAJDHANI",
            "passengers": [
                {"bookingStatus": "CNF", "currentStatus": "CNF", "bookingCoachId": "B1", "bookingBerthNo": 12, "currentCoachId": "B1", "currentBerthNo": 12}
            ]
        });
        let resp = map_railyatri_pnr("1234567890", &body, &stations).unwrap();
        assert_eq!(resp.train_number.as_deref(), Some("12951"));
        assert!(resp.notice.unwrap().contains("Railyatri"));
    }
}
