//! NTES public website (`enquiry.indianrail.gov.in/mntes`) form client.
//!
//! The mobile JSON API (`/crisns/AppServAnd`) is blocked from this deployment
//! by the Akamai bot manager (it answers an empty `200 OK`), so live boards are
//! queried through the public web forms, which are served to browsers without a
//! challenge:
//!
//! - `GET  /mntes/` -> Akamai `TS012f81d3`/`TS0161a678`, `JSESSIONID`,
//!   `SERVERID` cookies.
//! - `GET  /mntes/GetCSRFToken?t=<epoch-ms>` -> the CSRF hidden-input token.
//! - `POST /mntes/q?opt=<Query>&subOpt=<sub>` -> the actual query as a form.
//!
//! Responses are HTML tables, which are parsed into the same JSON shapes the
//! mobile API used to produce (`trainList`, `trainBtwStationList`, `list`), so
//! the services, cache keys and frontend contract stay unchanged.
//!
//! Rejected or challenged responses are recovered in two stages before giving
//! up: first only the CSRF token is re-fetched against the same session (a
//! stale/rejected token is answered with an empty `200 OK` while the session
//! cookies are still valid - verified live on 2026-08-16), then the whole
//! session (cookies + CSRF) is re-harvested. Callers must still propagate
//! `AppError::SourceUnavailable` honestly - never fabricate train data.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use reqwest::header::{COOKIE, ORIGIN, REFERER, SET_COOKIE, USER_AGENT};
use serde_json::{json, Value};

use super::super::error::AppError;
use super::super::http::HttpClient;

/// Desktop Chrome UA the NTES web app expects; non-browser UAs are challenged.
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

/// Characters of a body to keep when reporting a decode failure.
const SNIPPET_CHARS: usize = 120;

fn re(pattern: &str) -> &'static Regex {
    static REGEXES: std::sync::OnceLock<std::collections::HashMap<&'static str, Regex>> =
        std::sync::OnceLock::new();
    REGEXES
        .get_or_init(|| {
            let mut m = std::collections::HashMap::new();
            for (name, pat) in [
                ("tag", r"<[^>]+>"),
                ("csrf", r#"name=['"]([^'"]+)['"]\s+value=['"]([^'"]+)['"]"#),
                ("num", r"<b>\s*(\d{4,5})\s*</b>"),
                ("bold", r"<b>\s*([^<]+?)\s*</b>"),
                ("eta", r#"<font color="(?:green|red)">([^<]+)</font>"#),
                (
                    "delay",
                    r#"<span[^>]*class="[^"]*w3-(?:green|red)[^"]*"[^>]*>\s*([^<]+?)\s*</span>"#,
                ),
                ("sta", r#"<font size="1">\s*&nbsp;([^<]+)</font>"#),
                ("platform", r"<b>\s*(\d{1,2})\s*</b>"),
                (
                    "tbs_head",
                    r"<span><b>(\d{4,5})</b>\s*&nbsp;&nbsp;\s*([^<]+)</span>",
                ),
                ("tbs_runs", r"<span>\s*([^<]+?)\s*</span>"),
                (
                    "tbs_time",
                    r"<b>(\d{2}:\d{2})</b>\s*<br>\s*[^<]+?<br>\s*(?:<b>)?([A-Z0-9]{2,6})",
                ),
                // Spot-train popup (opt=TrainRunning, subOpt=FindRunningInstancePop).
                ("ts_header", r"<h3>\s*(\d{4,5})\s+([^<]+?)\s*</h3>"),
                (
                    "ts_pane",
                    r#"<div class="tab-pane ?(\w*)" id="(train[\w-]+)""#,
                ),
                ("ts_date", r"train(\d+)-(\w+)-(\d{4})"),
                ("ts_h6", r"<h6[^>]*>([\s\S]*?)</h6>"),
                (
                    "ts_currpos",
                    r#"id="currPos[^"]*">"#,
                ),
                (
                    "ts_block",
                    r#"class=" w3-card-2(?: w3-sand)?"#,
                ),
                (
                    "ts_name",
                    r#"<span><font size="1"><b>([^<]+)</b><br>"#,
                ),
                (
                    "ts_code",
                    r#"<b>([A-Z][A-Z0-9]{1,5})\s*(?:<span class="w3-round w3-orange"[^>]*>\s*PF\s*([\w*]+)\s*</span>\s*)?</b>"#,
                ),
                (
                    "ts_actual",
                    r#"color="(green|red)"\s*>[\s\S]*?<b>([^<]+)</b>[\s\S]*?w3-round w3-(green|red)"[^>]*>([^<]+)</span>"#,
                ),
                (
                    "ts_sch",
                    r#"<b><font size="1"[^>]*>([^<]+)</font></b>"#,
                ),
                (
                    "ts_left_col",
                    r#"style="float:left;width:100px;text-align:right;">"#,
                ),
                ("ts_track", r#"<div class="w3-bar-block"#),
                (
                    "ts_dep_col",
                    r#"style="float:right;text-align:right;">"#,
                ),
                ("ts_dep_end", r"</div>"),
                ("ts_time", r"(\d{1,2}:\d{2})"),
                (
                    "ts_badge_hr",
                    r"(\d+)\s*(?:hr|hrs|hour|hours|h)",
                ),
                (
                    "ts_badge_min",
                    r"(\d+)\s*(?:min|mins|minute|minutes)",
                ),
                ("ts_badge_colon", r"(\d{1,2}):(\d{2})"),
                // Station timetable (opt=TrainsAtStation, subOpt=tas).
                (
                    "stt_summary",
                    r"<b>\s*(\d+)\s+Trains scheduled at ([A-Z0-9]{2,6}) - ([^<]+?)\s*</b>",
                ),
                (
                    "stt_head",
                    r"<span\s*>\s*<b>(\d{4,5})</b>\s*&nbsp;&nbsp;\s*([^<]+?)</span>",
                ),
                (
                    "stt_route",
                    r"<br><span\s*>\s*([^<]+?)\s*</span>",
                ),
                (
                    "stt_time",
                    r"(?:Arr\.|Dep\.)[^<]*<b>([^<]+?)</b>",
                ),
                (
                    "stt_days",
                    r#"<div style="text-align: center; width: 50%;">([^<]+)</div>"#,
                ),
                // Average delay (opt=AverageDelay, subOpt=show).
                (
                    "ad_header",
                    r"<td[^>]*w3-blue[^>]*>[^<]*<span\s*>\s*(\d{4,5})\s+([^<]+?)\s*</span>",
                ),
                (
                    "ad_days",
                    r"Days of Run:\s*&nbsp;</span>\s*([^<]+?)\s*<",
                ),
                (
                    "ad_type",
                    r"Type:\s*&nbsp;</span>\s*<span>([^<]+?)</span>",
                ),
                (
                    "ad_delay",
                    r#"<font[^>]*color:\s*(?:green|red)[^>]*>\s*([^<]+?)\s*</font>"#,
                ),
                // Heritage trains (opt=HeritageTrainsBetweenStation, subOpt=tbsh).
                (
                    "ht_summary",
                    r#"<font class="bluehead"><b>(\d+)\s+([^<]+)</b></font>"#,
                ),
                (
                    "ht_head",
                    r#"<span style="padding-left: 10px;"><b>(\d{4,5})</b>\s*&nbsp;&nbsp;\s*([^<]+)</span>"#,
                ),
                (
                    "ht_runs",
                    r#"<span style="padding-left: 10px;">([^<]+)</span>"#,
                ),
                (
                    "ht_stn",
                    r"<b>(\d{2}:\d{2})</b>\s*<br>\s*([^<]+?)\s*<br>\s*<b>([A-Z0-9]{2,6})</b>",
                ),
                (
                    "ht_dur",
                    r"--(\d{1,2}:\d{2})\s*Hrs\.--",
                ),
                // Parcel special trains (opt=TrainRunning, subOpt=splTrnDtl).
                (
                    "parcel_no",
                    r#"onTrainInputByFindP\('(\d{4,5})'"#,
                ),
                (
                    "parcel_name",
                    r"</button>\s*&nbsp;<b>\s*([^<]+?)\s*</b>",
                ),
                (
                    "parcel_route",
                    r"<br/>\s*([^<]+?)\s*(?:<br|<div)",
                ),
                (
                    "parcel_validity",
                    r"Validity : <b>([^<]+)</b> To <b>([^<]+)</b>",
                ),
                (
                    "parcel_days",
                    r"Days of Run : <b>([^<]+)</b>",
                ),
                (
                    "parcel_leg",
                    r"<b>([A-Z0-9]{2,6}) - (\d{1,2}:\d{2})</b>",
                ),
                (
                    "parcel_travel",
                    r"Travel Time:\s*&nbsp;<b>(\d{1,2}:\d{2})\s*Hrs\.</b>",
                ),
                // Per-train exception calendar (opt=TrainRunning, subOpt=excpInfo).
                (
                    "exc_head",
                    r"<h4>\s*(\d{4,5})\s+-\s+([^<]+?)</h4>",
                ),
                (
                    "exc_route",
                    r"</h4>\s*([^<]+?)\s*<br",
                ),
                (
                    "exc_days",
                    r"Days of Run : <b>([^<]+)</b>",
                ),
                (
                    "exc_month",
                    r#"<font size="5pt">([A-Za-z]{3})-(\d{4})</font>"#,
                ),
                (
                    "exc_daynum",
                    r"<b>(?:\s*<font[^>]*>)?\s*(?:&nbsp;)?(\d{1,2})\s*(?:</font>)?\s*</b>",
                ),
                (
                    "exc_tag",
                    r"w3-tag[^>]*>\s*\[([^\]]+)\]",
                ),
                (
                    "exc_bg",
                    r"background:\s*([a-z]+)",
                ),
                (
                    "exc_fg",
                    r#"color="([^"]+)""#,
                ),
                // Journey Station Basis (opt=TrainRunning, subOpt=FindStationList).
                (
                    "fsl_train",
                    r#"name="trainNo"[^>]*value="([^"]*)""#,
                ),
                (
                    "fsl_option",
                    r#"<option title="([^"]*)" value="([^"]*)"[^>]*>\s*([^<]+?)\s*</option>"#,
                ),
                (
                    "fsl_option_block",
                    r#"<select[^>]*name="jStation"[^>]*>([\s\S]*?)</select>"#,
                ),
                // Train on Map (endpoint TrnMap) JavaScript variable blocks.
                (
                    "trn_route",
                    r#"var myStns\s*=\s*(\[[^;]*?\])\s*;"#,
                ),
                (
                    "trn_track",
                    r#"var myStnsF\s*=\s*(\[[^;]*?\])\s*;"#,
                ),
                (
                    "trn_names",
                    r#"var myStnNames\s*=\s*(\[[^;]*?\])\s*;"#,
                ),
                (
                    "trn_train",
                    r#"var train\s*=\s*(\[[^;]*?\]);"#,
                ),
                (
                    "trn_runinfo",
                    r#"var runInfo\s*=\s*(\[[^;]*?\]);"#,
                ),
                (
                    "trn_cstn",
                    r#"var cStn\s*=\s*(\[[^;]*?\]);"#,
                ),
                (
                    "trn_jstn",
                    r#"var jStn\s*=\s*(\[[^;]*?\]);"#,
                ),
            ] {
                m.insert(name, Regex::new(pat).expect("regex compiles"));
            }
            m
        })
        .get(pattern)
        .expect("known regex")
}

fn match_1(pattern: &str, text: &str) -> Option<String> {
    re(pattern)
        .captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
}

/// Web-form client for the public NTES query endpoint.
#[derive(Clone)]
pub struct NtesWebClient {
    http: HttpClient,
    base: String,
    /// `{base}/mntes` - the web app root that owns the CSRF and `/q` forms.
    web: String,
    /// Akamai + session cookies, harvested once from `GET /mntes/` and shared
    /// across clones via `Arc<Mutex<..>>` (same pattern as `IrctcClient`).
    cookies: Arc<Mutex<Vec<(String, String)>>>,
    /// Latest `(name, value)` CSRF token; re-fetched after a session reset or
    /// a CSRF-only refresh.
    csrf: Arc<Mutex<Option<(String, String)>>>,
}

impl NtesWebClient {
    /// Build a client rooted at `base_url` (e.g. the configured `ntes_base`,
    /// `https://enquiry.indianrail.gov.in`). The web app is reached under
    /// `{base}/mntes`. A trailing slash on the base is stripped.
    pub fn new(http: &HttpClient, base_url: &str) -> Self {
        let base = base_url.trim_end_matches('/').to_string();
        let web = format!("{base}/mntes");
        Self {
            http: http.clone(),
            web,
            base,
            cookies: Arc::new(Mutex::new(Vec::new())),
            csrf: Arc::new(Mutex::new(None)),
        }
    }

    /// Trains expected at `station_code` within `hours` hours. `station_name`
    /// is the official name from the local dataset (`NEW DELHI`); the form
    /// needs it verbatim as `jStnName`.
    ///
    /// Emits the mobile-shape JSON `{"trainList":[{"trainNo","trainName",
    /// "scheduledTime","expectedTime","delayArr","platformNo"}]}`.
    pub async fn live_station(
        &self,
        station_code: &str,
        station_name: &str,
        hours: u32,
    ) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "q",
                "LiveStation",
                "show",
                Some("See Train Status"),
                &[],
                &[
                    ("jStation", station_code.to_string()),
                    ("jStnName", station_name.to_string()),
                    ("jFromStationInput", station_code.to_string()),
                    ("jToStationInput", String::new()),
                    ("nHr", hours.to_string()),
                ],
            )
            .await?;
        parse_live_station(&html)
            .map(|trains| json!({ "trainList": trains }))
            .ok_or_else(|| {
                AppError::source_unavailable(
                    "ntes",
                    "no train rows found in the NTES live-station page",
                )
            })
    }

    /// Trains running between two stations. Station names come from the local
    /// dataset; the form wants `CODE - NAME` pairs.
    ///
    /// Emits the mobile-shape JSON `{"trainBtwStationList":[{"trainNo",
    /// "trainName","depTime","arrTime","runOnMon".."runOnSun"}]}`.
    pub async fn trains_between(
        &self,
        from_code: &str,
        from_name: &str,
        to_code: &str,
        to_name: &str,
    ) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "q",
                "TrainsBetweenStation",
                "tbs",
                Some("See Train Status"),
                &[],
                &[
                    ("jFromStationInput", format!("{from_code} - {from_name}")),
                    ("jToStationInput", format!("{to_code} - {to_name}")),
                    ("swap", String::new()),
                ],
            )
            .await?;
        parse_trains_between(&html)
            .map(|trains| json!({ "trainBtwStationList": trains }))
            .ok_or_else(|| {
                AppError::source_unavailable(
                    "ntes",
                    "no train rows found in the NTES trains-between page",
                )
            })
    }

    /// Trains scheduled at `station_code` on `date` (`None` = any day). The
    /// form needs the `CODE - NAME` pair; `date` is a `DD-MMM-YYYY` string or
    /// `No Specific Date`.
    ///
    /// Emits `{"station","stationName","date","total","list":[{"trainNo",
    /// "trainName","route","trainType","classes","arrival","departure",
    /// "days"}]}`.
    pub async fn station_timetable(
        &self,
        station_code: &str,
        station_name: &str,
        date: Option<&str>,
    ) -> Result<Value, AppError> {
        let date = date.unwrap_or("No Specific Date");
        let html = self
            .post_form(
                "q",
                "TrainsAtStation",
                "tas",
                Some("Trains scheduled at"),
                &[],
                &[
                    (
                        "jFromStationInput",
                        format!("{station_code} - {station_name}"),
                    ),
                    ("trainStartDate", date.to_string()),
                ],
            )
            .await?;
        parse_station_timetable(&html, date).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no station-timetable rows found in the NTES response",
            )
        })
    }

    /// Average arrival/departure delays for `train`.
    ///
    /// Emits `{"trainNo","trainName","daysOfRun","trainType","list":[{"sr",
    /// "name","code","arrivalDelay","departureDelay"}]}`.
    pub async fn average_delay(&self, train: &str) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "q",
                "AverageDelay",
                "show",
                Some("Avg. Arr. Delay"),
                &[],
                &[("trainNo", train.to_string())],
            )
            .await?;
        parse_average_delay(&html).ok_or_else(|| {
            AppError::source_unavailable("ntes", "no average-delay rows found in the NTES response")
        })
    }

    /// Heritage trains for the NTES selection list index `selection`.
    ///
    /// Emits `{"selection","total","list":[{"trainNo","trainName","runs",
    /// "trainType","srcTime","srcStation","srcCode","duration","dstTime",
    /// "dstStation","dstCode"}]}`.
    pub async fn heritage_trains(&self, selection: u8) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "q",
                "HeritageTrainsBetweenStation",
                "tbsh",
                Some("Heritage Trains"),
                &[],
                &[("heritageStn", selection.to_string())],
            )
            .await?;
        parse_heritage(&html).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no heritage-train rows found in the NTES response",
            )
        })
    }

    /// All parcel special trains currently running.
    ///
    /// Emits `{"list":[{"trainNo","trainName","route","validityFrom",
    /// "validityTo","daysOfRun","srcCode","srcTime","dstCode","dstTime",
    /// "travelTime"}]}`.
    pub async fn parcel_special_trains(&self) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "q",
                "TrainRunning",
                "splTrnDtl",
                Some("Parcel Special Trains"),
                &[],
                &[("trainNo", String::new())],
            )
            .await?;
        parse_parcel(&html).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no parcel-special-train rows found in the NTES response",
            )
        })
    }

    /// Per-train exception calendar (`opt=TrainRunning`, `subOpt=excpInfo`).
    ///
    /// The old batch `ExcpTrains` form (`excpType` + `excpDateType=T`) is
    /// disabled server-side ("Requested service in un-available at the
    /// moment"), so the only working NTES route is this per-train calendar.
    /// `train` is a plain number (e.g. `04138`); leading zeros are kept.
    ///
    /// Emits `{"train":{"number","name","source","destination","daysOfRun"},
    /// "exceptions":[{"date","kind","note"}],"noData"}`.
    pub async fn train_exceptions(&self, train: &str) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "q",
                "TrainRunning",
                "excpInfo",
                Some("Exceptional Trains Details"),
                &[],
                &[("trainNo", train.to_string())],
            )
            .await?;
        parse_train_exceptions(&html, train).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no exceptional-train calendar found in the NTES response",
            )
        })
    }

    /// Spot-a-train running instances for `train` (e.g. `12055`). The NTES
    /// "Spot Your Train" popup lists one tab per reported run (upcoming,
    /// current and past); the active tab is the relevant run, so it drives
    /// `train_start_date` and the live position.
    ///
    /// Emits the mobile `ShowFullRunJson`-shape JSON with the same field
    /// spellings the live-status service normalizes (`trainNo`, `trainName`,
    /// `source_stn_name`, `dest_stn_name`, `next_station_code/name`,
    /// `platform_number`, `at_src`, `at_dstn`, `train_start_date`,
    /// `instances[].{start_date,position}`, `stops[].{name,code,arrival,
    /// actual_arrival,platform,delay_minutes}`).
    pub async fn train_status(&self, train: &str) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "tr",
                "TrainRunning",
                "FindRunningInstancePop",
                Some("id=\"train"),
                &[("trainNo", train), ("refDate", "")],
                &[],
            )
            .await?;
        parse_train_status(&html).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no running-instance data in the NTES train-status page",
            )
        })
    }

    /// The journey stations NTES offers for `train` (the "Journey Station
    /// Basis" second mode of Spot Your Train). Each station's select value is
    /// the `CODE#<dayChange>#<seq>` triple the service composes back into the
    /// `jStation` form field.
    ///
    /// Emits `{"trainNo","list":[{"code","name","seq","dayChange",
    /// "arrivalDays","departureDays"}]}`.
    pub async fn journey_stations(&self, train: &str) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "q",
                "TrainRunning",
                "FindStationList",
                Some("name=\"jStation\""),
                &[],
                &[("trainNo", train.to_string())],
            )
            .await?;
        parse_journey_stations(&html).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no journey-station list found in the NTES response",
            )
        })
    }

    /// The journey-station-basis running status for `train` as seen from
    /// `j_station` - the full `CODE#<dayChange>#<seq>` select value, e.g.
    /// `NDLS#false#1`. Returns the SAME normalized shape as `train_status`.
    ///
    /// Emits the mobile `ShowFullRunJson`-shape JSON with the same field
    /// spellings `train_status` produces.
    pub async fn journey_station_basis(
        &self,
        train: &str,
        j_station: &str,
    ) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "tr",
                "TrainRunning",
                "ShowRunCStn",
                Some("id=\"train"),
                &[("trainNo", train)],
                &[
                    ("trainNo", train.to_string()),
                    ("jStation", j_station.to_string()),
                ],
            )
            .await?;
        parse_train_status(&html).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no journey-station-basis run found in the NTES response",
            )
        })
    }

    /// The full route map for `train` on `date` (`DD-MMM-YYYY`), from the NTES
    /// "Train on Map" JavaScript blocks.
    ///
    /// Emits `{"trainNo","trainName","source","destination","sourceCode",
    /// "destCode","startDate","route":[{"code","name","arrival","departure",
    /// "day","distance","daysOfRun"}],"track":[code,...]}`.
    pub async fn train_route_map(&self, train: &str, date: &str) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "TrnMap",
                "map",
                "route",
                Some("var myStns"),
                &[("trainNo", train), ("trainStartDate", date)],
                &[("trainNo", train.to_string())],
            )
            .await?;
        parse_train_route_map(&html).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no train-route-map data found in the NTES response",
            )
        })
    }

    /// The "Train on Map" live spot view for `train` on `date`, with
    /// `j_station` (`CODE#<dayChange>#<seq>`) as the journey station and
    /// `arr_dep_flag` the NTES `A`/`D` arrival/departure flag.
    ///
    /// Emits `{"trainNo","trainName","source","destination","sourceCode",
    /// "destCode","startDate","currentStation":{"code"},"journeyStation":
    /// {"code","name","label","expectedArrival","actualArrival","delayStatus",
    /// "platform"},"status":[{"code","expectedArrival","actualArrival",
    /// "expectedDeparture","actualDeparture","arrivalDelay",
    /// "departureDelay"}]}`.
    pub async fn train_spot_map(
        &self,
        train: &str,
        j_station: &str,
        date: &str,
        arr_dep_flag: &str,
    ) -> Result<Value, AppError> {
        let html = self
            .post_form(
                "TrnMap",
                "map",
                "spot",
                Some("var cStn"),
                &[
                    ("trainNo", train),
                    ("jStation", j_station),
                    ("jDate", date),
                    ("arrDepFlag", arr_dep_flag),
                    ("from", "N"),
                ],
                &[("trainNo", train.to_string())],
            )
            .await?;
        parse_train_spot_map(&html).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                "no train-spot-map data found in the NTES response",
            )
        })
    }

    // -- session / transport -------------------------------------------------

    /// One query round-trip with staged recovery. A transport failure, an
    /// empty body, or (when `rows_marker` is set) a result page without the
    /// expected content means the request was rejected or challenged. Recovery
    /// replaces the cheapest thing first (NTES answers a rejected/stale CSRF
    /// token with an empty `200 OK` while the session cookies are still valid):
    ///
    /// 1. drop only the cached CSRF token so the retry re-fetches it against
    ///    the same session;
    /// 2. re-harvest cookies + CSRF as a whole fresh session.
    ///
    /// Only after both stages does it give up honestly.
    async fn post_form(
        &self,
        endpoint: &str,
        opt: &str,
        sub_opt: &str,
        rows_marker: Option<&str>,
        query: &[(&str, &str)],
        fields: &[(&str, String)],
    ) -> Result<String, AppError> {
        for attempt in 0..3 {
            match self
                .post_form_once(endpoint, opt, sub_opt, query, fields)
                .await
            {
                Ok(body) => {
                    let challenged =
                        body.trim().is_empty() || rows_marker.is_some_and(|m| !body.contains(m));
                    if !challenged {
                        return Ok(body);
                    }
                    self.recover(attempt);
                }
                Err(e) => {
                    if attempt < 2 {
                        self.recover(attempt);
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Err(AppError::source_unavailable(
            "ntes",
            "NTES web form returned no usable data after a CSRF + session refresh",
        ))
    }

    /// Stage the recovery for the next attempt: a stale token is dropped first
    /// (the session cookies stay, so the retry only re-fetches the CSRF token);
    /// only a second failure resets the whole session.
    fn recover(&self, attempt: usize) {
        if attempt == 0 {
            *self.csrf.lock().unwrap() = None;
        } else {
            self.reset_session();
        }
    }

    async fn post_form_once(
        &self,
        endpoint: &str,
        opt: &str,
        sub_opt: &str,
        query: &[(&str, &str)],
        fields: &[(&str, String)],
    ) -> Result<String, AppError> {
        let (name, value) = self.csrf_token().await?;
        let mut body: Vec<(String, String)> = vec![("lan".to_string(), "en".to_string())];
        if endpoint == "q" {
            body.extend(
                [
                    ("appLang".to_string(), "en".to_string()),
                    ("find".to_string(), "Get Trains".to_string()),
                    ("clear".to_string(), "Clear".to_string()),
                ]
                .into_iter(),
            );
        }
        body.extend(fields.iter().map(|(k, v)| (k.to_string(), v.clone())));
        body.push((name, value));

        let mut url = format!("{}/{endpoint}?opt={opt}&subOpt={sub_opt}", self.web);
        for (k, v) in query {
            url.push_str(&format!("&{k}={v}"));
        }
        let cookie = cookie_str(&self.cookies.lock().unwrap());
        let mut req = self
            .http
            .inner()
            .post(&url)
            .form(&body)
            .header(USER_AGENT, BROWSER_UA)
            .header(REFERER, format!("{}/", self.web))
            .header(ORIGIN, self.base.clone())
            .header("X-Requested-With", "XMLHttpRequest");
        if !cookie.is_empty() {
            req = req.header(COOKIE, cookie);
        }
        let res = req
            .send()
            .await
            .map_err(|e| AppError::source_unavailable("ntes", format!("request failed: {e}")))?;
        let status = res.status();
        let bytes = res
            .bytes()
            .await
            .map_err(|e| AppError::source_unavailable("ntes", format!("read response: {e}")))?;
        if !status.is_success() {
            return Err(AppError::source_unavailable(
                "ntes",
                format!("POST {url} returned {status}"),
            ));
        }
        if bytes.is_empty() {
            return Err(AppError::source_unavailable(
                "ntes",
                format!("POST {url} returned an empty response (status {status})"),
            ));
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Current `(name, value)` CSRF token, fetching it once per session.
    async fn csrf_token(&self) -> Result<(String, String), AppError> {
        if let Some(token) = self.csrf.lock().unwrap().clone() {
            return Ok(token);
        }
        self.ensure_session().await;
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| AppError::internal(format!("ntes: system clock before unix epoch: {e}")))?
            .as_millis();
        let url = format!("{}/GetCSRFToken?t={millis}", self.web);
        let cookie = cookie_str(&self.cookies.lock().unwrap());
        let mut req = self
            .http
            .inner()
            .get(&url)
            .header(USER_AGENT, BROWSER_UA)
            .header(REFERER, format!("{}/", self.web))
            .header("X-Requested-With", "XMLHttpRequest");
        if !cookie.is_empty() {
            req = req.header(COOKIE, cookie);
        }
        let res = req
            .send()
            .await
            .map_err(|e| AppError::source_unavailable("ntes", format!("request failed: {e}")))?;
        let status = res.status();
        let bytes = res.bytes().await.map_err(|e| {
            AppError::source_unavailable("ntes", format!("read CSRF token response: {e}"))
        })?;
        if !status.is_success() {
            return Err(AppError::source_unavailable(
                "ntes",
                format!("GET {url} returned {status}"),
            ));
        }
        let text = String::from_utf8_lossy(&bytes);
        if text.trim().is_empty() {
            return Err(AppError::source_unavailable("ntes", "empty CSRF response"));
        }
        let (name, value) = extract_csrf(&text).ok_or_else(|| {
            AppError::source_unavailable(
                "ntes",
                format!("no CSRF token in response: {}", body_snippet(&text)),
            )
        })?;
        *self.csrf.lock().unwrap() = Some((name.clone(), value.clone()));
        Ok((name, value))
    }

    /// Lazy one-time session bootstrap: harvest the Akamai + session cookies
    /// from the web app root. Best-effort like the IRCTC client - if the root
    /// is blocked the jar stays empty and the query fails with its real error.
    async fn ensure_session(&self) {
        if !self.cookies.lock().unwrap().is_empty() {
            return;
        }
        let url = format!("{}/", self.web);
        let req = self
            .http
            .inner()
            .get(&url)
            .header(USER_AGENT, BROWSER_UA)
            .header(REFERER, self.base.clone());
        if let Ok(res) = req.send().await {
            self.merge_cookies(&res);
        }
    }

    /// Drop cookies + CSRF so the next call re-harvests a fresh session.
    fn reset_session(&self) {
        self.cookies.lock().unwrap().clear();
        *self.csrf.lock().unwrap() = None;
    }

    /// Append `Set-Cookie` headers, replacing any same-named cookie so the
    /// newest value (the live Akamai token) wins.
    fn merge_cookies(&self, res: &reqwest::Response) {
        let mut cookies = self.cookies.lock().unwrap();
        for (name, value) in res
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|s| s.split(';').next())
            .filter_map(|pair| pair.split_once('='))
            .map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))
        {
            merge_cookie(&mut cookies, name, value);
        }
    }
}

// -- HTML parsing -------------------------------------------------------------

/// Live-station table rows -> mobile-shape JSON array (or `None` when the page
/// carried no train rows, i.e. it was the challenged nav shell).
fn parse_live_station(html: &str) -> Option<Vec<Value>> {
    let rows: Vec<Value> = html
        .split("<tr")
        .filter(|seg| seg.contains("See Train Status"))
        .filter_map(parse_live_row)
        .collect();
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}

/// One live-station row: `<td>serial</td><td>train info</td><td>arrival</td>
/// <td>departure</td><td>platform</td>`.
fn parse_live_row(seg: &str) -> Option<Value> {
    let tds: Vec<&str> = seg.split("<td").skip(1).collect();
    if tds.len() < 5 {
        return None;
    }
    let info = tds[1];
    let arrival = tds[2];
    let departure = tds[3];
    let platform_cell = tds[4];

    let number = match_1("num", info)?;
    let name = re("bold")
        .captures_iter(info)
        .nth(1)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default();
    // Arrival-focused fields, falling back to the departure column for trains
    // that originate here (their arrival cell only shows "Source").
    let eta = match_1("eta", arrival).or_else(|| match_1("eta", departure));
    let sta = match_1("sta", arrival).or_else(|| match_1("sta", departure));
    let delay_text = match_1("delay", arrival).or_else(|| match_1("delay", departure));
    let delayed = match delay_text {
        Some(t) => !t.eq_ignore_ascii_case("On Time"),
        None => eta.as_deref() != sta.as_deref(),
    };
    let platform = match_1("platform", platform_cell).unwrap_or_default();

    Some(json!({
        "trainNo": number,
        "trainName": name,
        "scheduledTime": sta.unwrap_or_default(),
        "expectedTime": eta.unwrap_or_default(),
        "delayArr": delayed,
        "platformNo": platform,
    }))
}

/// Trains-between rows -> mobile-shape JSON array (or `None` when the page
/// carried no train rows).
fn parse_trains_between(html: &str) -> Option<Vec<Value>> {
    let rows: Vec<Value> = html
        .split("<tr")
        .filter(|seg| seg.contains("See Train Status"))
        .filter_map(parse_tbs_row)
        .collect();
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}

fn parse_tbs_row(seg: &str) -> Option<Value> {
    let head = re("tbs_head").captures(seg)?;
    let number = head.get(1)?.as_str().to_string();
    let name = head.get(2)?.as_str().trim().to_string();
    let days = parse_runs(seg);
    let mut times: Vec<String> = re("tbs_time")
        .captures_iter(seg)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .collect();
    if times.is_empty() {
        return None;
    }
    let dep_time = times.remove(0);
    let arr_time = times.pop().unwrap_or_default();

    let mut entry = json!({
        "trainNo": number,
        "trainName": name,
        "depTime": dep_time,
        "arrTime": arr_time,
    });
    if let Value::Object(map) = &mut entry {
        for (day, on) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .into_iter()
            .zip(days.iter())
        {
            map.insert(format!("runOn{day}"), json!(on));
        }
    }
    Some(entry)
}

/// Run-days text (second plain `<span>`), e.g. `Daily | Superfast` or
/// `Mon Wed Fri | Superfast` / `Mon,Sat | Superfast`, into Monday-first
/// booleans. NTES mixes space- and comma-separated day lists.
fn parse_runs(seg: &str) -> [bool; 7] {
    let run_text = match_1("tbs_runs", seg).unwrap_or_default();
    let left = run_text.split('|').next().unwrap_or_default().trim();
    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    if left.eq_ignore_ascii_case("daily") {
        return [true; 7];
    }
    let tokens: Vec<&str> = left
        .split(|c: char| c == ',' || c.is_whitespace())
        .collect();
    let mut days = [false; 7];
    for (i, day) in day_names.iter().enumerate() {
        days[i] = tokens.iter().any(|t| t.eq_ignore_ascii_case(day));
    }
    days
}

/// Calendar month-name lookup for `MMM-YYYY` headers (e.g. `Aug-2026`).
const EXC_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Per-train exception-calendar page -> `{"train":{...}, "exceptions":[...],
/// "noData"}` (or `None` when the page is neither a result nor a no-data page,
/// e.g. a challenge/error page).
///
/// * Result page: `<h4>04138 - BJU GWL SPL</h4>`, a `source - destination.<br/>`
///   line, `Days of Run : <b>Wed,Sun</b>` and one `<td class="w3-tooltip">`
///   cell per calendar day under the `<font size="5pt">Aug-2026</font>` header.
///   Exception days carry a `w3-tag` tooltip span (e.g. `[Train is Cancelled]`)
///   and/or a coloured circle (`<font color="white" style="background: red">`).
/// * No-data page: `<div class="w3-panel w3-round w3-red"><h4>No Exceptional
///   Details found for train 12951 !!!</h4></div>` with no calendar; only the
///   requested `train` number is known.
fn parse_train_exceptions(html: &str, train: &str) -> Option<Value> {
    let no_data = html.contains("No Exceptional Details found");
    if no_data {
        return Some(json!({
            "train": json!({
                "number": train,
                "name": "",
                "source": "",
                "destination": "",
                "daysOfRun": [],
            }),
            "exceptions": [],
            "noData": true,
        }));
    }

    let caps = re("exc_head").captures(html)?;
    let number = caps.get(1)?.as_str().trim().to_string();
    let name = caps.get(2)?.as_str().trim().to_string();

    let (source, destination) = re("exc_route")
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| {
            let line = m.as_str().trim().trim_end_matches('.').to_string();
            let mut parts = line.splitn(2, " - ");
            let src = parts.next().unwrap_or("").trim().to_string();
            let dst = parts.next().unwrap_or("").trim().to_string();
            (src, dst)
        })
        .unwrap_or((String::new(), String::new()));

    let days_of_run: Vec<String> = re("exc_days")
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| {
            m.as_str()
                .split(',')
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // The calendar month/year is the anchor for the day cells; without it the
    // page does not carry the calendar we are built to parse.
    let (month, year) = re("exc_month")
        .captures(html)
        .and_then(|c| {
            let month_name = c.get(1)?.as_str();
            let month = EXC_MONTHS
                .iter()
                .position(|m| m.eq_ignore_ascii_case(month_name))?
                + 1;
            let year: u32 = c.get(2)?.as_str().parse().ok()?;
            Some((month, year))
        })
        .unwrap_or((0, 0));
    if month == 0 {
        return None;
    }

    let exceptions: Vec<Value> = html
        .split(r#"<td class="w3-tooltip""#)
        .skip(1)
        .filter_map(|cell| {
            let day = re("exc_daynum")
                .captures(cell)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())?;
            let day: u32 = day.parse().ok()?;
            let (kind, note) = exception_kind(cell)?;
            Some(json!({
                "date": format!("{year:04}-{month:02}-{day:02}"),
                "kind": kind,
                "note": note,
            }))
        })
        .collect();

    Some(json!({
        "train": json!({
            "number": number,
            "name": name,
            "source": source,
            "destination": destination,
            "daysOfRun": days_of_run,
        }),
        "exceptions": exceptions,
        "noData": false,
    }))
}

/// Classify one calendar day cell as an exception kind, or `None` for a normal
/// (green), not-running (grey) or blank day. Kind is decided from the hover
/// tooltip span first (`[Train is Cancelled]` etc.), then from the circle's
/// (text, background) colour pair, matching the legend table on the page.
fn exception_kind(cell: &str) -> Option<(&'static str, &'static str)> {
    if let Some(caps) = re("exc_tag").captures(cell) {
        let note = caps.get(1)?.as_str().trim();
        return match note {
            "Train is Cancelled" => Some(("cancelled", "Train is Cancelled")),
            "Train is Scheduled to Run on Diverted Route" => {
                Some(("diverted", "Train is Scheduled to Run on Diverted Route"))
            }
            "Train is Rescheduled from Source" => {
                Some(("rescheduled", "Train is Rescheduled from Source"))
            }
            "Train is Scheduled to Start from New Source" => {
                Some(("new_source", "Train is Scheduled to Start from New Source"))
            }
            "Train is Scheduled to Terminate on New Destination" => Some((
                "new_destination",
                "Train is Scheduled to Terminate on New Destination",
            )),
            _ => return None,
        };
    }

    let bg = re("exc_bg")
        .captures(cell)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    let fg = re("exc_fg")
        .captures(cell)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    match (fg.as_str(), bg.as_str()) {
        ("white", "red") => Some(("cancelled", "Train is Cancelled")),
        ("white", "blue") => Some(("rescheduled", "Train is Rescheduled from Source")),
        ("white", "orange") => Some(("diverted", "Train is Scheduled to Run on Diverted Route")),
        ("green", "yellow") => Some(("new_source", "Train is Scheduled to Start from New Source")),
        ("yellow", "red") => Some((
            "new_destination",
            "Train is Scheduled to Terminate on New Destination",
        )),
        _ => None,
    }
}

/// Station-timetable page -> `{station, stationName, date, total, list}` (or
/// `None` when the page carried no train rows). `date` mirrors the requested
/// form value (`DD-MMM-YYYY` or `No Specific Date`).
fn parse_station_timetable(html: &str, date: &str) -> Option<Value> {
    let summary = re("stt_summary").captures(html)?;
    let total: usize = summary.get(1)?.as_str().trim().parse().ok()?;
    let station = summary.get(2)?.as_str().to_string();
    let station_name = summary.get(3)?.as_str().trim().to_string();
    let list: Vec<Value> = html
        .split("<tr")
        .filter(|seg| seg.contains("showTrainServiceSchedule"))
        .filter_map(parse_station_timetable_row)
        .collect();
    if list.is_empty() {
        return None;
    }
    Some(json!({
        "station": station,
        "stationName": station_name,
        "date": date,
        "total": total,
        "list": list,
    }))
}

fn parse_station_timetable_row(seg: &str) -> Option<Value> {
    let head = re("stt_head").captures(seg)?;
    let number = head.get(1)?.as_str().to_string();
    let name = head.get(2)?.as_str().trim().to_string();
    let spans: Vec<String> = re("stt_route")
        .captures_iter(seg)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .collect();
    let route = spans.first().cloned().unwrap_or_default();
    let (train_type, classes) = match spans.get(1).map(|s| s.as_str()) {
        Some(s) => match s.split_once('|') {
            Some((t, c)) => (t.trim().to_string(), c.trim().to_string()),
            None => (s.to_string(), String::new()),
        },
        None => (String::new(), String::new()),
    };
    let mut times: Vec<String> = re("stt_time")
        .captures_iter(seg)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .collect();
    if times.len() < 2 {
        return None;
    }
    let arrival = times.remove(0);
    let departure = times.pop().unwrap_or_default();
    let days = match_1("stt_days", seg).unwrap_or_default();

    Some(json!({
        "trainNo": number,
        "trainName": name,
        "route": route,
        "trainType": train_type,
        "classes": classes,
        "arrival": arrival,
        "departure": departure,
        "days": days,
    }))
}

/// Average-delay page -> `{trainNo, trainName, daysOfRun, trainType, list}`
/// (or `None` when no station rows were found).
fn parse_average_delay(html: &str) -> Option<Value> {
    let header = re("ad_header").captures(html)?;
    let train_no = header.get(1)?.as_str().to_string();
    let train_name = header.get(2)?.as_str().trim().to_string();
    let days_of_run = match_1("ad_days", html).unwrap_or_default();
    let train_type = match_1("ad_type", html).unwrap_or_default();
    let rows: Vec<Value> = html
        .split("<tr")
        .filter(|seg| seg.contains("font-size:small large") && !seg.contains("font-weight: bold"))
        .filter_map(parse_average_delay_row)
        .collect();
    if rows.is_empty() {
        return None;
    }
    Some(json!({
        "trainNo": train_no,
        "trainName": train_name,
        "daysOfRun": days_of_run,
        "trainType": train_type,
        "list": rows,
    }))
}

fn parse_average_delay_row(seg: &str) -> Option<Value> {
    let cells: Vec<&str> = seg.split("<td").skip(1).collect();
    if cells.len() < 5 {
        return None;
    }
    let sr = td_text(cells[0]);
    if sr.is_empty() || !sr.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(json!({
        "sr": sr,
        "name": td_text(cells[1]),
        "code": td_text(cells[2]),
        "arrivalDelay": match_1("ad_delay", cells[3]).unwrap_or_default(),
        "departureDelay": match_1("ad_delay", cells[4]).unwrap_or_default(),
    }))
}

/// Heritage-trains page -> `{selection, total, list}` (or `None` when no train
/// rows were found). `selection` is the summary caption text, e.g.
/// `All Heritage Trains`.
fn parse_heritage(html: &str) -> Option<Value> {
    let summary = re("ht_summary").captures(html)?;
    let total: usize = summary.get(1)?.as_str().trim().parse().ok()?;
    let selection = summary.get(2)?.as_str().trim().to_string();
    // Rows nest a `<table>` (with its own `<tr>`), so `split("<tr")` cannot
    // delimit them; instead each row is the slice between two `ht_head` spans.
    let heads: Vec<(usize, usize)> = re("ht_head")
        .find_iter(html)
        .map(|m| (m.start(), m.end()))
        .collect();
    let list: Vec<Value> = heads
        .iter()
        .enumerate()
        .filter_map(|(i, (start, _))| {
            let end = heads.get(i + 1).map(|(s, _)| *s).unwrap_or(html.len());
            parse_heritage_row(&html[*start..end])
        })
        .collect();
    if list.is_empty() {
        return None;
    }
    Some(json!({
        "selection": selection,
        "total": total,
        "list": list,
    }))
}

fn parse_heritage_row(seg: &str) -> Option<Value> {
    let head = re("ht_head").captures(seg)?;
    let number = head.get(1)?.as_str().to_string();
    let name = head.get(2)?.as_str().trim().to_string();
    let (runs, train_type) = match_1("ht_runs", seg)
        .and_then(|s| {
            s.split_once('|')
                .map(|(r, t)| (r.trim().to_owned(), t.trim().to_owned()))
        })
        .unwrap_or_default();
    let stops: Vec<(String, String, String)> = re("ht_stn")
        .captures_iter(seg)
        .map(|c| {
            (
                c.get(1)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
                c.get(2)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
                c.get(3)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
            )
        })
        .collect();
    if stops.is_empty() {
        return None;
    }
    let (src_time, src_station, src_code) = stops[0].clone();
    let (dst_time, dst_station, dst_code) = stops.get(1).cloned().unwrap_or_default();

    Some(json!({
        "trainNo": number,
        "trainName": name,
        "runs": runs,
        "trainType": train_type,
        "srcTime": src_time,
        "srcStation": src_station,
        "srcCode": src_code,
        "duration": match_1("ht_dur", seg).unwrap_or_default(),
        "dstTime": dst_time,
        "dstStation": dst_station,
        "dstCode": dst_code,
    }))
}

/// Parcel-special-trains page -> `{list}` (or `None` when no train rows were
/// found).
fn parse_parcel(html: &str) -> Option<Value> {
    let list: Vec<Value> = html
        .split("<tr")
        .filter(|seg| seg.contains("onTrainInputByFindP"))
        .filter_map(parse_parcel_row)
        .collect();
    if list.is_empty() {
        None
    } else {
        Some(json!({ "list": list }))
    }
}

fn parse_parcel_row(seg: &str) -> Option<Value> {
    let number = match_1("parcel_no", seg)?;
    let validity = re("parcel_validity").captures(seg)?;
    let legs: Vec<(String, String)> = re("parcel_leg")
        .captures_iter(seg)
        .map(|c| {
            (
                c.get(1)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
                c.get(2)
                    .map(|m| m.as_str().trim().to_string())
                    .unwrap_or_default(),
            )
        })
        .collect();
    let (src_code, src_time) = legs.first().cloned().unwrap_or_default();
    let (dst_code, dst_time) = legs.get(1).cloned().unwrap_or_default();

    Some(json!({
        "trainNo": number,
        "trainName": match_1("parcel_name", seg).unwrap_or_default(),
        "route": match_1("parcel_route", seg).unwrap_or_default(),
        "validityFrom": validity.get(1)?.as_str().trim().to_string(),
        "validityTo": validity.get(2)?.as_str().trim().to_string(),
        "daysOfRun": match_1("parcel_days", seg).unwrap_or_default(),
        "srcCode": src_code,
        "srcTime": src_time,
        "dstCode": dst_code,
        "dstTime": dst_time,
        "travelTime": match_1("parcel_travel", seg).unwrap_or_default(),
    }))
}

// -- journey-station / train-on-map parsing -----------------------------------

/// Parse a JavaScript string-array literal (`["NDLS","GZB",...]`) into
/// strings: strip the brackets, split on commas, trim whitespace and the
/// surrounding double quotes. Empty entries are preserved - the track array
/// (`myStnsF`) uses them as real placeholders.
fn js_array(literal: &str) -> Vec<String> {
    let inner = literal.trim();
    let inner = inner
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(inner);
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim().to_string())
        .collect()
}

/// The `runInfo` array literal has the same shape as the code lists.
fn js_runinfo(literal: &str) -> Vec<String> {
    js_array(literal)
}

/// FindStationList page -> `{trainNo, list}` of journey stations (or `None`
/// when the page carried no `jStation` options, i.e. it was the challenged
/// shell). Each option carries the `arrDays#depDays` title and the
/// `CODE#dayChange#seq` value the service round-trips into `ShowRunCStn`.
fn parse_journey_stations(html: &str) -> Option<Value> {
    let train_no = match_1("fsl_train", html)?;
    let block = re("fsl_option_block").captures(html)?.get(1)?.as_str();
    let list: Vec<Value> = re("fsl_option")
        .captures_iter(block)
        .map(|c| {
            let (arrival_days, departure_days) = c
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or("")
                .split_once('#')
                .unwrap_or(("", ""));
            let parts: Vec<&str> = c
                .get(2)
                .map(|m| m.as_str())
                .unwrap_or("")
                .split('#')
                .collect();
            let text = c.get(3).map(|m| m.as_str()).unwrap_or("");
            json!({
                "code": parts.first().copied().unwrap_or(""),
                "name": text
                    .split_once(" - ")
                    .map(|(n, _)| n.trim())
                    .unwrap_or_else(|| text.trim()),
                "seq": parts
                    .get(2)
                    .and_then(|s| s.trim().parse::<usize>().ok())
                    .unwrap_or(0),
                "dayChange": parts
                    .get(1)
                    .and_then(|s| s.trim().parse::<bool>().ok())
                    .unwrap_or(false),
                "arrivalDays": arrival_days.trim(),
                "departureDays": departure_days.trim(),
            })
        })
        .collect();
    if list.is_empty() {
        return None;
    }
    Some(json!({ "trainNo": train_no, "list": list }))
}

/// TrnMap route page (JavaScript variable blocks) -> `{trainNo, trainName,
/// source, destination, sourceCode, destCode, startDate, route, track}` (or
/// `None` when the page carried no route codes). Each `runInfo` entry is the
/// `arr#dep#day#distance#daysOfRun#daysOfRunActual` tuple of the route station
/// at the same index.
fn parse_train_route_map(html: &str) -> Option<Value> {
    let route = match_1("trn_route", html)?;
    let route = js_array(&route);
    if route.is_empty() {
        return None;
    }
    let track = match_1("trn_track", html)
        .map(|s| js_array(&s))
        .unwrap_or_default();
    let names = match_1("trn_names", html)
        .map(|s| js_array(&s))
        .unwrap_or_default();
    let train = match_1("trn_train", html)
        .map(|s| js_array(&s))
        .unwrap_or_default();
    let run_info = match_1("trn_runinfo", html)
        .map(|s| js_runinfo(&s))
        .unwrap_or_default();

    let route_entries: Vec<Value> = run_info
        .iter()
        .enumerate()
        .map(|(i, info)| {
            let parts: Vec<&str> = info.split('#').collect();
            json!({
                "code": route.get(i).cloned().unwrap_or_default(),
                "name": names.get(i).cloned().unwrap_or_default(),
                "arrival": parts.first().copied().unwrap_or(""),
                "departure": parts.get(1).copied().unwrap_or(""),
                "day": parts.get(2).copied().unwrap_or(""),
                "distance": parts.get(3).copied().unwrap_or(""),
                "daysOfRun": parts.get(4).copied().unwrap_or(""),
            })
        })
        .collect();

    Some(json!({
        "trainNo": train.first().cloned().unwrap_or_default(),
        "trainName": train.get(1).cloned().unwrap_or_default(),
        "source": train.get(2).cloned().unwrap_or_default(),
        "destination": train.get(3).cloned().unwrap_or_default(),
        "sourceCode": train.get(4).cloned().unwrap_or_default(),
        "destCode": train.get(5).cloned().unwrap_or_default(),
        "startDate": train.get(6).cloned().unwrap_or_default(),
        "route": route_entries,
        "track": track,
    }))
}

/// TrnMap spot page (JavaScript variable blocks) -> `{trainNo, trainName,
/// source, destination, sourceCode, destCode, startDate, currentStation,
/// journeyStation, status}` (or `None` when the page carried no current
/// station or route codes). Unlike the route page, each `runInfo` entry is the
/// `arrDatetime|arrDelay#depDatetime|depDelay` pair where `Source` /
/// `Destination` mark the terminals.
fn parse_train_spot_map(html: &str) -> Option<Value> {
    let route = match_1("trn_route", html)?;
    let route = js_array(&route);
    let cstn = match_1("trn_cstn", html)?;
    let cstn = js_array(&cstn);
    if route.is_empty() || cstn.is_empty() {
        return None;
    }
    let train = match_1("trn_train", html)
        .map(|s| js_array(&s))
        .unwrap_or_default();
    let jstn = match_1("trn_jstn", html)
        .map(|s| js_array(&s))
        .unwrap_or_default();
    let run_info = match_1("trn_runinfo", html)
        .map(|s| js_runinfo(&s))
        .unwrap_or_default();

    let status: Vec<Value> = run_info
        .iter()
        .enumerate()
        .map(|(i, info)| {
            let (arr_part, dep_part) = match info.split_once('#') {
                Some((a, d)) => (a, d),
                None => (info.as_str(), ""),
            };
            let (arr_token, arr_delay) = spot_delay(arr_part);
            let (dep_token, dep_delay) = spot_delay(dep_part);
            json!({
                "code": route.get(i).cloned().unwrap_or_default(),
                "expectedArrival": spot_time(&arr_token),
                "actualArrival": "",
                "expectedDeparture": spot_time(&dep_token),
                "actualDeparture": "",
                "arrivalDelay": arr_delay,
                "departureDelay": dep_delay,
            })
        })
        .collect();

    let journey_station = json!({
        "code": jstn.first().cloned().unwrap_or_default(),
        "name": jstn.get(1).cloned().unwrap_or_default(),
        "label": strip_tags(jstn.get(4).map(String::as_str).unwrap_or("")),
        "expectedArrival": jstn.get(5).cloned().unwrap_or_default(),
        "actualArrival": jstn.get(6).cloned().unwrap_or_default(),
        "delayStatus": jstn.get(7).cloned().unwrap_or_default(),
        "platform": jstn.get(8).cloned().unwrap_or_default(),
    });

    Some(json!({
        "trainNo": train.first().cloned().unwrap_or_default(),
        "trainName": train.get(1).cloned().unwrap_or_default(),
        "source": train.get(2).cloned().unwrap_or_default(),
        "destination": train.get(3).cloned().unwrap_or_default(),
        "sourceCode": train.get(4).cloned().unwrap_or_default(),
        "destCode": train.get(5).cloned().unwrap_or_default(),
        "startDate": train.get(6).cloned().unwrap_or_default(),
        "currentStation": json!({ "code": cstn.first().cloned().unwrap_or_default() }),
        "journeyStation": journey_station,
        "status": status,
    }))
}

/// `17-Aug-2026 15:20|On Time` -> (`17-Aug-2026 15:20`, `On Time`).
fn spot_delay(part: &str) -> (String, String) {
    match part.split_once('|') {
        Some((t, d)) => (t.trim().to_string(), d.trim().to_string()),
        None => (part.trim().to_string(), String::new()),
    }
}

/// Terminal markers (`Source` / `Destination`) carry no real time - blank them.
fn spot_time(token: &str) -> String {
    if token.is_empty()
        || token.eq_ignore_ascii_case("Source")
        || token.eq_ignore_ascii_case("Destination")
    {
        String::new()
    } else {
        token.to_string()
    }
}

// -- spot-train parsing ------------------------------------------------------

/// One run-instance tab from the spot-train popup.
struct TrainStatusPane {
    /// `DD-MMM-YYYY` (title-case month) of this run's start.
    date: String,
    active: bool,
    /// Position banner text, e.g. `Departed from GHAZIABAD(GZB) at 15:58 14-Aug
    /// (Delay: 00:03)`.
    position: String,
    html: String,
}

#[derive(Debug)]
struct TrainStop {
    name: String,
    code: String,
    platform: String,
    scheduled_arrival: String,
    actual_arrival: String,
    delay_minutes: i64,
}

/// Spot-train popup -> the shared normalized train-status JSON (or `None`
/// when the response carried no run instances, i.e. an unknown train number).
///
/// NTES renders every run instance (start-date tab) with its own position
/// banner and station timeline, so each instance is parsed with its full
/// per-stop status and surfaced in `instances[].stops`. The active run still
/// fills the top-level fields (next station, position) exactly as before; a
/// later date selection swaps in the matching instance's own stops and
/// position fields (see `live_status::service::select_run_for_date`).
fn parse_train_status(html: &str) -> Option<Value> {
    let header = re("ts_header").captures(html)?;
    let train_number = header.get(1)?.as_str().trim().to_string();
    let train_name = strip_tags(header.get(2)?.as_str());

    let panes = train_status_panes(html);
    let active = panes.iter().find(|p| p.active).unwrap_or(&panes[0]);
    let stops: Vec<TrainStop> = train_stops(&active.html);
    if stops.is_empty() {
        return None;
    }

    let (at_src, at_dstn, current_idx) = current_position(&active.position, &stops);
    // A not-yet-started run is "at" its origin: point the next-station widget
    // at the source so the service marks exactly the source as expected.
    let next_idx = if at_src {
        0
    } else {
        (current_idx + 1).min(stops.len() - 1)
    };

    let instances: Vec<Value> = panes
        .iter()
        .map(|p| {
            let stops = train_stops(&p.html);
            let (at_src, at_dstn, current_idx) = current_position(&p.position, &stops);
            let next_idx = if at_src {
                0
            } else {
                (current_idx + 1).min(stops.len().saturating_sub(1))
            };
            let next = stops.get(next_idx);
            json!({
                "start_date": p.date,
                "position": p.position,
                "at_src": at_src.to_string(),
                "at_dstn": at_dstn.to_string(),
                "next_station_code": next.map(|s| s.code.as_str()).unwrap_or(""),
                "next_station_name": next.map(|s| s.name.as_str()).unwrap_or(""),
                "platform_number": next.map(|s| s.platform.as_str()).unwrap_or(""),
                "stops": stops_json(&stops),
            })
        })
        .collect();

    Some(json!({
        "train_number": train_number,
        "train_name": train_name,
        "source_stn_name": stops[0].name,
        "dest_stn_name": stops[stops.len() - 1].name,
        "next_station_code": stops[next_idx].code,
        "next_station_name": stops[next_idx].name,
        "platform_number": stops[next_idx].platform,
        "at_src": at_src.to_string(),
        "at_dstn": at_dstn.to_string(),
        "train_start_date": active.date,
        "data_source": "NTES",
        "instances": instances,
        "stops": stops_json(&stops),
    }))
}

/// One station row of a run timeline, as the shared normalized JSON.
fn stops_json(stops: &[TrainStop]) -> Vec<Value> {
    stops
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "code": s.code,
                "arrival": s.scheduled_arrival,
                "actual_arrival": s.actual_arrival,
                "platform": s.platform,
                "delay_minutes": s.delay_minutes,
            })
        })
        .collect()
}

fn train_status_panes(html: &str) -> Vec<TrainStatusPane> {
    let starts: Vec<(usize, String, bool)> = re("ts_pane")
        .captures_iter(html)
        .map(|c| {
            let start = c.get(0).map(|m| m.start()).unwrap_or(0);
            let id = c.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
            let active = c.get(1).map(|m| m.as_str() == "active").unwrap_or(false);
            (start, id, active)
        })
        .collect();
    let mut panes = Vec::with_capacity(starts.len());
    for (i, (start, id, active)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map(|(s, _, _)| *s).unwrap_or(html.len());
        let seg = &html[*start..end];
        panes.push(TrainStatusPane {
            date: pane_date(id),
            active: *active,
            position: pane_position(seg),
            html: seg.to_string(),
        });
    }
    panes
}

/// `train14-aug-2026` -> `14-Aug-2026`.
fn pane_date(id: &str) -> String {
    match re("ts_date").captures(id) {
        Some(c) => {
            let month = c
                .get(2)
                .map(|m| {
                    let s = m.as_str();
                    let mut chars = s.chars();
                    match chars.next() {
                        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                        None => s.to_string(),
                    }
                })
                .unwrap_or_default();
            format!(
                "{}-{}-{}",
                c.get(1).map(|m| m.as_str()).unwrap_or(""),
                month,
                c.get(3).map(|m| m.as_str()).unwrap_or("")
            )
        }
        None => id.to_string(),
    }
}

/// Position banner of one run: the page's status `<h6>`, falling back to the
/// text of the `w3-sand` current-position banner.
fn pane_position(seg: &str) -> String {
    if let Some(c) = re("ts_h6").captures(seg) {
        let text = strip_tags(c.get(1).map(|m| m.as_str()).unwrap_or(""));
        if !text.is_empty() {
            return text;
        }
    }
    if let Some(start) = re("ts_currpos").find(seg) {
        let rest = &seg[start.end()..];
        let end = re("ts_block")
            .find(rest)
            .map(|m| m.start())
            .unwrap_or(rest.len());
        let text = strip_tags(&rest[..end]);
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// Station-by-station live timeline of one run, skipping the position banner
/// (`currPos` / `w3-sand`) cards. Blocks run from the end of one `w3-card-2`
/// delimiter to the start of the next, so a nested "upcoming station" card
/// inside a station block is never absorbed into it.
fn train_stops(seg: &str) -> Vec<TrainStop> {
    let mut stops = Vec::new();
    let spans: Vec<(usize, usize)> = re("ts_block")
        .find_iter(seg)
        .map(|m| (m.start(), m.end()))
        .collect();
    for (i, (_, end)) in spans.iter().enumerate() {
        let start = *end;
        let end = spans.get(i + 1).map(|(s, _)| *s).unwrap_or(seg.len());
        let block = &seg[start..end];
        if block.contains("currPos") || block.contains("w3-sand") {
            continue;
        }
        if let Some(stop) = parse_train_stop(block) {
            stops.push(stop);
        }
    }
    stops
}

fn parse_train_stop(block: &str) -> Option<TrainStop> {
    let name_m = re("ts_name").captures(block)?;
    let name = strip_tags(name_m.get(1)?.as_str());
    if name.is_empty() {
        return None;
    }
    let after = &block[name_m.get(0)?.end()..];
    let code_m = re("ts_code").captures(after)?;
    let code = code_m.get(1)?.as_str().to_string();
    let platform = code_m
        .get(2)
        .map(|m| m.as_str().replace('*', ""))
        .unwrap_or_default();

    let left = column_text(block, "ts_left_col", "ts_track");
    let dep = column_text(block, "ts_dep_col", "ts_dep_end");
    // The source row shows `SRC` in the arrival column; fall back to the
    // departure column (the origin departure), like the live-station rows do.
    let col = if left
        .and_then(|l| match_1("ts_sch", l))
        .is_some_and(|t| t.chars().any(|c| c.is_ascii_digit()))
    {
        left
    } else {
        dep
    };

    let scheduled = col
        .and_then(|c| match_1("ts_sch", c))
        .filter(|t| t.chars().any(|c| c.is_ascii_digit()))
        .map(|t| hhmm(&t))
        .unwrap_or_default();
    let (actual, delay) = match col.and_then(|c| re("ts_actual").captures(c)) {
        Some(c) if is_real_time(c.get(2).map(|m| m.as_str()).unwrap_or("")) => (
            hhmm(c.get(2).map(|m| m.as_str()).unwrap_or("")),
            badge_minutes(c.get(4).map(|m| m.as_str()).unwrap_or("")),
        ),
        _ => (String::new(), 0),
    };

    Some(TrainStop {
        name,
        code,
        platform,
        scheduled_arrival: scheduled,
        actual_arrival: actual,
        delay_minutes: delay,
    })
}

/// Slice of `block` between the two named column markers.
fn column_text<'a>(block: &'a str, start_pat: &str, end_pat: &str) -> Option<&'a str> {
    let start = re(start_pat).find(block)?.end();
    let rest = &block[start..];
    let end = re(end_pat)
        .find(rest)
        .map(|m| m.start())
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// First `HH:MM` in `text`, else empty.
fn hhmm(text: &str) -> String {
    re("ts_time")
        .captures(text)
        .map(|c| c.get(1).map(|m| m.as_str()).unwrap_or_default().to_string())
        .unwrap_or_default()
}

/// A reported actual is real only when it has no trailing `*` (NTES marks
/// estimated times with a star).
fn is_real_time(raw: &str) -> bool {
    let t = raw.trim();
    !t.is_empty() && !t.ends_with('*') && t.contains(':')
}

/// Badge text (`On Time`, `3 Min`, `41 Mins.`, `1 Hr 5 Min`) -> minutes late.
fn badge_minutes(badge: &str) -> i64 {
    let b = badge.trim().to_ascii_lowercase();
    if b.is_empty() || b.contains("on time") {
        return 0;
    }
    if let Some(c) = re("ts_badge_colon").captures(&b) {
        let h = c
            .get(1)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(0);
        let m = c
            .get(2)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(0);
        return h * 60 + m;
    }
    let hours = re("ts_badge_hr")
        .captures(&b)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or(0);
    let mins = re("ts_badge_min")
        .captures(&b)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or(0);
    hours * 60 + mins
}

/// Where the run is, from its position banner. Returns `(at_src, at_dstn,
/// current_stop_idx)`; the current index is the last departed stop.
fn current_position(position: &str, stops: &[TrainStop]) -> (bool, bool, usize) {
    if position.to_ascii_lowercase().contains("yet to start") {
        return (true, false, 0);
    }
    for (prefix, arrived) in [("Departed from", false), ("Arrived at", true)] {
        if let Some(code) = pos_code(position, prefix) {
            if let Some(i) = stops.iter().position(|s| s.code == code) {
                return (false, arrived && i == stops.len() - 1, i);
            }
        }
    }
    match stops.iter().rposition(|s| !s.actual_arrival.is_empty()) {
        Some(i) if i == stops.len() - 1 => (false, true, i),
        Some(i) => (false, false, i),
        None => (true, false, 0),
    }
}

/// Station code out of `Departed from GHAZIABAD(GZB) at ...` style text.
fn pos_code(position: &str, prefix: &str) -> Option<String> {
    let rest = position.get(position.find(prefix)? + prefix.len()..)?;
    let code = rest.split('(').nth(1)?.split(')').next()?.trim();
    if (2..=6).contains(&code.len()) && code.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(code.to_string())
    } else {
        None
    }
}

/// Text of one `<td>` cell given the segment after the `<td` marker, dropping
/// the opening tag's attributes (e.g. ` align="left"><font..>NAME</font>` ->
/// `NAME`).
fn td_text(seg: &str) -> String {
    let rest = seg.split_once('>').map(|(_, r)| r).unwrap_or(seg);
    strip_tags(rest)
}

/// Collapse an HTML cell to plain text (tags stripped, entities unescaped for
/// spaces, stray `<`/`>` removed, whitespace normalized).
fn strip_tags(seg: &str) -> String {
    re("tag")
        .replace_all(seg, " ")
        .into_owned()
        .replace("&nbsp;", " ")
        .chars()
        .filter(|c| !matches!(c, '<' | '>'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract `(name, value)` from a CSRF hidden-input, e.g.
/// `<input type='hidden' name='csrfToken' value='abc'>`.
fn extract_csrf(body: &str) -> Option<(String, String)> {
    let caps = re("csrf").captures(body)?;
    let name = caps.get(1)?.as_str().trim().to_string();
    let value = caps.get(2)?.as_str().trim().to_string();
    if name.is_empty() || value.is_empty() {
        None
    } else {
        Some((name, value))
    }
}

/// Insert/replace one cookie in the jar (newest value wins).
fn merge_cookie(jar: &mut Vec<(String, String)>, name: String, value: String) {
    if name.is_empty() {
        return;
    }
    if let Some(existing) = jar.iter_mut().find(|(k, _)| *k == name) {
        existing.1 = value;
    } else {
        jar.push((name, value));
    }
}

fn cookie_str(cookies: &[(String, String)]) -> String {
    cookies
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn body_snippet(body: &str) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    let snippet: String = chars.by_ref().take(SNIPPET_CHARS).collect();
    if chars.next().is_some() {
        format!("{snippet}...")
    } else {
        snippet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn http() -> HttpClient {
        HttpClient::new("railway-rs-test", Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn csrf_extraction_handles_hidden_input() {
        let body =
            "<input type='hidden' name='-zr1hgfgigick1786620354' value='-1ageb3318ns5329777005'>";
        assert_eq!(
            extract_csrf(body).unwrap(),
            (
                "-zr1hgfgigick1786620354".to_string(),
                "-1ageb3318ns5329777005".to_string()
            )
        );
        let body =
            "<input type=\"hidden\" name=\"csrfToken\" value=\"dce6e4e056319e36dac78a98842e5432\">";
        assert_eq!(
            extract_csrf(body).unwrap(),
            (
                "csrfToken".to_string(),
                "dce6e4e056319e36dac78a98842e5432".to_string()
            )
        );
        assert_eq!(extract_csrf("Request Rejected"), None);
    }

    #[test]
    fn live_station_row_parses_times_delay_and_platform() {
        let row = r#"<tr><td nowrap>1</td>
        <td align=left nowrap><b>04071</b>&nbsp;|<b> SOU NDLS SPL</b><br>
        <font size="2">(SOU-NDLS)&nbsp;TRAIN ON DEMAND</font><br>
        <span class="w3-round w3-blue w3-tiny" style="padding:1px 4px;font-size:8pt;cursor: pointer;" onclick="onTrainStatus('04071',document.getElementsByName('frmSTN')[0],'13-Aug-2026')">See Train Status >></span>
        &nbsp;
        <span class="w3-round w3-orange w3-tiny" style="padding:1px 4px;font-size:8pt;cursor: pointer;" onclick="showTrainServiceSchedule('04071','13-Aug-2026',document.getElementsByName('frmSTN')[0])">Train Schedule >></span>
        </td>
        <td nowrap width="130px">
            <font color="red">15:00</font><br>
            <span class="w3-round w3-red w3-tiny" style="padding: 1px 4px;">41 Mins.</span><br>
            <font size="1">&nbsp;14:19</font>
        </td>
        <td nowrap width="130px">
            <font color="red">15:01*</font><br>
            <span class="w3-round w3-red w3-tiny" style="padding: 1px 4px;">40 Mins.</span><br>
            <font size="1">&nbsp;14:21</font>
        </td>
        <td width="80px"><b>1</b><br><button>Coach Position</button></td></tr>"#;
        let trains = parse_live_station(row).unwrap();
        let t = &trains[0];
        assert_eq!(t["trainNo"], "04071");
        assert_eq!(t["trainName"], "SOU NDLS SPL");
        assert_eq!(t["scheduledTime"], "14:19");
        assert_eq!(t["expectedTime"], "15:00");
        assert_eq!(t["delayArr"], true);
        assert_eq!(t["platformNo"], "1");
    }

    #[test]
    fn live_station_on_time_and_source_trains() {
        let row = r#"<tr><td nowrap>1</td>
        <td align=left nowrap><b>12951</b>&nbsp;|<b> MUMBAI RAJDHANI</b><br>
        <span onclick="onTrainStatus('12951',document.getElementsByName('frmSTN')[0],'13-Aug-2026')">See Train Status >></span>
        </td>
        <td nowrap width="130px">
            <font color="green">09:15</font><br>
            <span class="w3-round w3-green w3-tiny">On Time</span><br>
            <font size="1">&nbsp;09:15</font>
        </td>
        <td nowrap width="130px"><font size="2">Source</font></td>
        <td width="80px"><b>1</b></td></tr>
        <tr><td nowrap>2</td>
        <td align=left nowrap><b>12301</b>&nbsp;|<b> RAJDHANI EXP</b><br>
        <span onclick="onTrainStatus('12301',document.getElementsByName('frmSTN')[0],'13-Aug-2026')">See Train Status >></span>
        </td>
        <td nowrap width="130px"><font size="2">Destination</font></td>
        <td nowrap width="130px">
            <font color="green">10:00</font><br>
            <font size="1">&nbsp;10:00</font>
        </td>
        <td width="80px"><b>2</b></td></tr>"#;
        let trains = parse_live_station(row).unwrap();
        assert_eq!(trains.len(), 2);
        assert_eq!(trains[0]["expectedTime"], "09:15");
        assert_eq!(trains[0]["delayArr"], false);
        // Source train: arrival cell has no times, falls back to departure.
        assert_eq!(trains[1]["expectedTime"], "10:00");
        assert_eq!(trains[1]["delayArr"], false);
        assert_eq!(trains[1]["platformNo"], "2");
    }

    #[test]
    fn trains_between_row_parses_times_and_runs() {
        let row = r#"<tr class="w3-round">
        <td colspan=3>
        <span><b>12904</b>&nbsp;&nbsp;GOLDEN TEMPLE M</span><br>
        <span>Daily | Superfast</span>
        <div style="float: right;padding:4px;border:0px;margin-top:-30px;text-align:right;"><img onclick="showTrainServiceSchedule('12904','14-Aug-2026',document.getElementsByName('frmTBS')[0]);" /><br>
        <span class="w3-round" style="padding-top:10px;padding-left:10px;"><span class="w3-round w3-blue" style="padding: 0px 3px;font-size:9pt;cursor: pointer;" onclick="onTrainStatus('12904',document.getElementsByName('frmTBS')[0],'')">See Train Status >></span></span></div>
        <div style="display: flex; justify-content: space-between; align-items: center; width: 100%; border-top: 1px solid #eee; text-align: center; padding: 2px 0;">
        <span style="text-align: left;width: 25%;"><b>04:00</b><br>Hazrat Nizamuddin Jn<br>NZM</span>
        <div style="text-align: center; width: 50%;">--19:55 Hrs.--<br>&nbsp;&nbsp;</div>
        <span style="text-align: right; width: 25%;"><b>23:55</b><br>Bandra Terminus<br><b>BDTS</b></span>
        </div>
        </td>
        </tr>
        <tr class="w3-round">
        <td colspan=3>
        <span><b>22654</b>&nbsp;&nbsp;NZM TVC SF EXP</span><br>
        <span>Mon,Sat | Superfast</span>
        <span class="w3-round w3-blue" style="padding: 0px 3px;font-size:9pt;cursor: pointer;" onclick="onTrainStatus('22654',document.getElementsByName('frmTBS')[0],'')">See Train Status >></span>
        <span style="text-align: left;width: 25%;"><b>17:00</b><br>Hazrat Nizamuddin Jn<br>NZM</span>
        <span style="text-align: right; width: 25%;"><b>11:40</b><br>Kochuveli<br><b>KCVL</b></span>
        </td>
        </tr>"#;
        let trains = parse_trains_between(row).unwrap();
        assert_eq!(trains.len(), 2);
        assert_eq!(trains[0]["trainNo"], "12904");
        assert_eq!(trains[0]["trainName"], "GOLDEN TEMPLE M");
        assert_eq!(trains[0]["depTime"], "04:00");
        assert_eq!(trains[0]["arrTime"], "23:55");
        for day in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
            assert_eq!(trains[0][format!("runOn{day}")], true, "{day} runs");
        }
        assert_eq!(trains[1]["depTime"], "17:00");
        assert_eq!(trains[1]["arrTime"], "11:40");
        assert_eq!(trains[1]["runOnMon"], true);
        assert_eq!(trains[1]["runOnTue"], false);
        assert_eq!(trains[1]["runOnWed"], false);
        assert_eq!(trains[1]["runOnFri"], false);
        assert_eq!(trains[1]["runOnSat"], true);
        assert_eq!(trains[1]["runOnSun"], false);
    }

    #[test]
    fn train_exceptions_parses_calendar() {
        let html = r##"<html><title>Exceptional Trains Details</title><body>
        <h4>04138 - BJU GWL SPL</h4>
        BARAUNI JN - GWALIOR JN.<br/>
        Days of Run : <b>Wed,Sun</b>
        <table><tr><th colspan="7"><font size="5pt">Aug-2026</font></th></tr>
        <tr>
        <td class="w3-tooltip" style="padding: 10px;">
        <font color="#bfbfbf" size="4pt"><b>&nbsp;</b></font>
        </td>
        <td class="w3-tooltip" style="padding: 10px;">
        <font color="#bfbfbf" size="4pt"><b>10</b></font>
        </td>
        <td class="w3-tooltip" style="padding: 10px;">
        <font color="green" size="4pt"><b>12</b></font>
        </td>
        <td class="w3-tooltip" style="padding: 10px;">
        <span style="position:absolute;left:0;bottom:40px" class="w3-text w3-tag w3-red w3-round-xlarge">[Train is Cancelled]</span>
        <b> <font color="white" size="4pt" style="background: red;border-radius: 50%;padding: 5px;">16</font></b>
        </td>
        <td class="w3-tooltip" style="padding: 10px;">
        <span style="position:absolute;left:0;bottom:40px" class="w3-text w3-tag w3-orange w3-round-xlarge">[Train is Scheduled to Run on Diverted Route]</span>
        <b> <font color="white" size="4pt" style="background: orange;border-radius: 50%;padding: 5px;">19</font></b>
        </td>
        </tr>
        </table></body></html>"##;
        let data = parse_train_exceptions(html, "04138").unwrap();
        assert_eq!(data["noData"], false);
        assert_eq!(data["train"]["number"], "04138");
        assert_eq!(data["train"]["name"], "BJU GWL SPL");
        assert_eq!(data["train"]["source"], "BARAUNI JN");
        assert_eq!(data["train"]["destination"], "GWALIOR JN");
        assert_eq!(data["train"]["daysOfRun"], json!(["Wed", "Sun"]));
        let exceptions = data["exceptions"].as_array().unwrap();
        assert_eq!(exceptions.len(), 2);
        assert_eq!(exceptions[0]["date"], "2026-08-16");
        assert_eq!(exceptions[0]["kind"], "cancelled");
        assert_eq!(exceptions[0]["note"], "Train is Cancelled");
        assert_eq!(exceptions[1]["date"], "2026-08-19");
        assert_eq!(exceptions[1]["kind"], "diverted");
    }

    #[test]
    fn train_exceptions_colour_pair_fallback() {
        let html = r##"<h4>12951 - TEST EXP</h4>
        SRC JN - DST JN<br/>
        Days of Run : <b>Mon</b>
        <table><tr><th><font size="5pt">Sep-2026</font></th></tr>
        <tr>
        <td class="w3-tooltip" style="padding: 10px;">
        <b> <font color="yellow" size="4pt" style="background: red;border-radius: 50%;padding: 5px;">3</font></b>
        </td>
        <td class="w3-tooltip" style="padding: 10px;">
        <b> <font color="white" size="4pt" style="background: blue;border-radius: 50%;padding: 5px;">4</font></b>
        </td>
        </tr></table>"##;
        let data = parse_train_exceptions(html, "12951").unwrap();
        let exceptions = data["exceptions"].as_array().unwrap();
        assert_eq!(exceptions.len(), 2);
        assert_eq!(exceptions[0]["date"], "2026-09-03");
        assert_eq!(exceptions[0]["kind"], "new_destination");
        assert_eq!(exceptions[1]["date"], "2026-09-04");
        assert_eq!(exceptions[1]["kind"], "rescheduled");
    }

    #[test]
    fn train_exceptions_nodata_page() {
        let html = r##"<html><title>Exceptional Trains Details</title><body>
        <div class="w3-panel w3-round w3-red"><h4>No Exceptional Details found for train 12951 !!!</h4></div>
        </body></html>"##;
        let data = parse_train_exceptions(html, "12951").unwrap();
        assert_eq!(data["noData"], true);
        assert_eq!(data["train"]["number"], "12951");
        assert!(data["exceptions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn shell_page_without_rows_parses_to_none() {
        assert_eq!(
            parse_live_station("<table><tr><th>No data</th></tr></table>"),
            None
        );
        assert_eq!(
            parse_trains_between("<table><tr><th>No data</th></tr></table>"),
            None
        );
        assert_eq!(
            parse_train_exceptions("<table><tr><th>No data</th></tr></table>", "12951"),
            None
        );
    }

    #[test]
    fn station_timetable_parses_summary_and_rows() {
        let html = r##"<table>
        <tr><th colspan="9" align="left"><font size="2" color="#006AD5" face="verdana"><b>326  Trains scheduled at NDLS - NEW DELHI</b></font></th></tr>
        <tr class=" w3-round">
          <td colspan=3 ...>
            <span ><b>22403</b>&nbsp;&nbsp;PDY NDLS SF EXP</span>
            <br><span >Pondicherry (PDY) - New Delhi (NDLS)</span>
            <br><span >Superfast | 1A,2A,3A,SL,GEN,PWD</span>
            <div style="float: right;padding:5px;border:0px;margin-top:-30px;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" onclick="showTrainServiceSchedule('22403','15-Aug-2026',document.getElementsByName('frmTAS')[0]);" style="cursor:pointer;background: #eee;"/></div>
            <div style="display: flex; justify-content: space-between; align-items: center; width: 100%; border-top: 1px solid #eee; text-align: center; padding: 5px 2px;">
              <span style="text-align: left;width: 25%;">Arr.	: <b>00:20</b></span>
              <div style="text-align: center; width: 50%;">- Fri -</div>
              <span style="text-align: right; width: 25%;">Dep.: <b>DSTN</b></span>
            </div>
          </td>
        </tr>
        </table>"##;
        let data = parse_station_timetable(html, "No Specific Date").unwrap();
        assert_eq!(data["station"], "NDLS");
        assert_eq!(data["stationName"], "NEW DELHI");
        assert_eq!(data["date"], "No Specific Date");
        assert_eq!(data["total"], 326);
        let list = data["list"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["trainNo"], "22403");
        assert_eq!(list[0]["trainName"], "PDY NDLS SF EXP");
        assert_eq!(list[0]["route"], "Pondicherry (PDY) - New Delhi (NDLS)");
        assert_eq!(list[0]["trainType"], "Superfast");
        assert_eq!(list[0]["classes"], "1A,2A,3A,SL,GEN,PWD");
        assert_eq!(list[0]["arrival"], "00:20");
        assert_eq!(list[0]["departure"], "DSTN");
        assert_eq!(list[0]["days"], "- Fri -");
    }

    #[test]
    fn station_timetable_parses_daily_run_and_source_departure() {
        let html = r##"<table>
        <tr><th colspan="9" align="left"><font size="2" color="#006AD5" face="verdana"><b>2  Trains scheduled at GZB - GHAZIABAD</b></font></th></tr>
        <tr class=" w3-round">
          <td colspan=3>
            <span ><b>12055</b>&nbsp;&nbsp;DDN JANSHTBDI</span>
            <br><span >New Delhi (NDLS) - Ghaziabad (GZB)</span>
            <br><span >JAN SHATABDI | CC,EC</span>
            <div style="float: right;padding:5px;border:0px;margin-top:-30px;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" onclick="showTrainServiceSchedule('12055','15-Aug-2026',document.getElementsByName('frmTAS')[0]);" style="cursor:pointer;background: #eee;"/></div>
            <div style="display: flex; justify-content: space-between; align-items: center; width: 100%; border-top: 1px solid #eee; text-align: center; padding: 5px 2px;">
              <span style="text-align: left;width: 25%;">Arr.	: <b>SRC</b></span>
              <div style="text-align: center; width: 50%;">- Daily -</div>
              <span style="text-align: right; width: 25%;">Dep.: <b>15:55</b></span>
            </div>
          </td>
        </tr>
        </table>"##;
        let data = parse_station_timetable(html, "15-Aug-2026").unwrap();
        assert_eq!(data["date"], "15-Aug-2026");
        assert_eq!(data["total"], 2);
        let train = &data["list"][0];
        assert_eq!(train["trainNo"], "12055");
        assert_eq!(train["arrival"], "SRC");
        assert_eq!(train["departure"], "15:55");
        assert_eq!(train["days"], "- Daily -");
        assert_eq!(train["trainType"], "JAN SHATABDI");
        assert_eq!(train["classes"], "CC,EC");
    }

    #[test]
    fn average_delay_parses_header_and_rows() {
        let html = r#"<table class="table table-bordered table-condensed table-striped" >
          <tbody>
            <tr><td class="w3-blue" align="left" style="border-bottom:1px solid #cccccc;border-right:none;"colspan="2"><span >12055 DDN JANSHTBDI</span></TD></tr>
            <tr>
              <td align="left" style="border-bottom:none;border-right:none;"><span class="bluehead">Days of Run: &nbsp;</span>Daily</TD>
              <td align="right" style="border-bottom:none;"><span class="bluehead">Type: &nbsp;</span><span>JAN SHATABDI</span></TD>
            </tr>
          </tbody>
        </table>
        <table class="table table-bordered table-condensed table-striped">
          <tbody>
            <tr valign="top" height="20">
              <td><font style="font-size:small large; font-weight: bold">Sr.</font></td>
              <td><font style="font-size:small large; font-weight: bold">Station</font></td>
              <td><font style="font-size:small large; font-weight: bold">Code</font></td>
              <td><font style="font-size:small large; font-weight: bold">Avg. Arr. Delay</font></td>
              <td><font style="font-size:small large; font-weight: bold">Avg. Dep. Delay</font></td>
            </tr>
            <tr><td><font style="font-size:small large;">1</font></td><td align="left"><font style="font-size:small large;">NEW DELHI</font></td><td><font style="font-size:small large;">NDLS</font></td><td></td><td><font style="font-size:small large;  color: green">On Time</font></td></tr>
            <tr><td><font style="font-size:small large;">2</font></td><td align="left"><font style="font-size:small large;">GHAZIABAD</font></td><td><font style="font-size:small large;">GZB</font></td><td><font style="font-size:small large;  color: red">00:14</font></td><td><font style="font-size:small large;  color: red">00:15</font></td></tr>
          </tbody>
        </table>"#;
        let data = parse_average_delay(html).unwrap();
        assert_eq!(data["trainNo"], "12055");
        assert_eq!(data["trainName"], "DDN JANSHTBDI");
        assert_eq!(data["daysOfRun"], "Daily");
        assert_eq!(data["trainType"], "JAN SHATABDI");
        let list = data["list"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["sr"], "1");
        assert_eq!(list[0]["name"], "NEW DELHI");
        assert_eq!(list[0]["code"], "NDLS");
        assert_eq!(
            list[0]["arrivalDelay"], "",
            "empty arrival cell -> empty delay"
        );
        assert_eq!(list[0]["departureDelay"], "On Time");
        assert_eq!(list[1]["sr"], "2");
        assert_eq!(list[1]["name"], "GHAZIABAD");
        assert_eq!(list[1]["code"], "GZB");
        assert_eq!(list[1]["arrivalDelay"], "00:14");
        assert_eq!(list[1]["departureDelay"], "00:15");
    }

    #[test]
    fn heritage_parses_summary_and_rows() {
        let html = r#"<table>
        <tr><td colspan="9" align="left" style="padding-left: 20px;"><font class="bluehead"><b>43 All Heritage Trains</b></font></td></tr>
        <tr class=" w3-round" style="margin-left: 5dp;margin-right: 5dp;">
          <td style="width: 30px;" align="center">1.</td>
          <td  colspan=3 style="border-radius: 25px 25px 25px 25px;margin-top: 10px;padding-bottom: 0px;border-bottom:5px solid #eee;"><span style="padding-left: 10px;"><b>52457</b>&nbsp;&nbsp;KLK SML EXP</span><br><span style="padding-left: 10px;">Daily | Passenger</span>
          <div style="float: right;padding:4px;border:0px;margin-top:-20px;margin-right:10px;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" style="background: #eee;cursor: pointer;" onclick="showTrainServiceSchedule('52457','15-Aug-2026',document.getElementsByName('frmTBSH')[0]);" /></div>
          <div style="width: 100%;height:1px;background-color:#E9ECEE;"></div>
          <table style="width: 100%;margin: 0px;padding-left:10px;padding-right:10px;">
            <tr style="padding: 0px;">
              <td width="35%" style="padding-left: 10px;"><b>03:30</b><br>KALKA<br><b>KLK</b></td>
              <td align="center" width="30%">--05:20 Hrs.--</td>
              <td align="right" width="35%" style="padding-right: 10px;"><b>08:50</b><br>SHIMLA<br><b>SML</b></td>
            </tr>
          </table>
          </td>
        </tr>
        </table>"#;
        let data = parse_heritage(html).unwrap();
        assert_eq!(data["selection"], "All Heritage Trains");
        assert_eq!(data["total"], 43);
        let list = data["list"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["trainNo"], "52457");
        assert_eq!(list[0]["trainName"], "KLK SML EXP");
        assert_eq!(list[0]["runs"], "Daily");
        assert_eq!(list[0]["trainType"], "Passenger");
        assert_eq!(list[0]["srcTime"], "03:30");
        assert_eq!(list[0]["srcStation"], "KALKA");
        assert_eq!(list[0]["srcCode"], "KLK");
        assert_eq!(list[0]["duration"], "05:20");
        assert_eq!(list[0]["dstTime"], "08:50");
        assert_eq!(list[0]["dstStation"], "SHIMLA");
        assert_eq!(list[0]["dstCode"], "SML");
    }

    #[test]
    fn parcel_special_parses_rows() {
        let html = r#"<table>
        <tr><th style="text-align: center;" colspan="6">All Parcel Special Trains</th></tr>
        <tr class="active">
        <td  align="center" valign="middle" style="width: 60px;" nowrap><b>1</b></td>
        <td style="text-align: left;indent:8px; margin-top:5px;"><button type="button" class="custom-btn" style="height: 30px;padding-left: 10px;padding-right: 10px;padding-bottom: 5px;padding-top: 5px;" onClick="javascript:onTrainInputByFindP('00111','15-Aug-2026')"><b>00111</b></button> &nbsp;<b> BIRD-SGTY RAPID CARGO </b>&nbsp;&nbsp; <span class="w3-round w3-blue w3-tiny w3-round" style="padding:2px 5px;font-size:8pt;cursor: pointer;" onclick="javascript:onTrainInputByFindP('00111','15-Aug-2026')">See Train Status >></span>
        <div style="float: right;padding:4px;border:1px solid #E9ECEE;"><img alt="See Schedule" height="20" width="20" src="images/calendar_black.png" onclick="showTrainServiceScheduleSpot('00111','15-Aug-2026',845513,document.frmTBS);" style="cursor:pointer;background: #eee;" />
        </div>
        <br/>BHIVANDI ROAD - SANKRAIL GOODS TERMINAL
        <br/> Validity : <b>25-Jul-2026</b> To <b>31-Dec-2099</b>
        <div style="width: 100%;height:1px;background-color:#E9ECEE;"></div>
        Days of Run : <b>Sat</b>
        <br/>
        <b>BIRD - 22:30</b>
        |<b>SGTY - 15:15</b>
        |Travel Time:&nbsp;<b>40:45 Hrs.</b>
        <br/></td>
        </tr>
        </table>"#;
        let data = parse_parcel(html).unwrap();
        let list = data["list"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["trainNo"], "00111");
        assert_eq!(list[0]["trainName"], "BIRD-SGTY RAPID CARGO");
        assert_eq!(list[0]["route"], "BHIVANDI ROAD - SANKRAIL GOODS TERMINAL");
        assert_eq!(list[0]["validityFrom"], "25-Jul-2026");
        assert_eq!(list[0]["validityTo"], "31-Dec-2099");
        assert_eq!(list[0]["daysOfRun"], "Sat");
        assert_eq!(list[0]["srcCode"], "BIRD");
        assert_eq!(list[0]["srcTime"], "22:30");
        assert_eq!(list[0]["dstCode"], "SGTY");
        assert_eq!(list[0]["dstTime"], "15:15");
        assert_eq!(list[0]["travelTime"], "40:45");
    }

    #[test]
    fn new_parsers_reject_shell_pages() {
        let shell = "<table><tr><th>No data</th></tr></table>";
        assert_eq!(parse_station_timetable(shell, "No Specific Date"), None);
        assert_eq!(parse_average_delay(shell), None);
        assert_eq!(parse_heritage(shell), None);
        assert_eq!(parse_parcel(shell), None);
    }

    // -- spot train ----------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn spot_block(
        code: &str,
        name: &str,
        arr_s: &str,
        arr_a: &str,
        badge: &str,
        dep_s: &str,
        dep_a: &str,
        pf: &str,
        km: &str,
        src: bool,
        dstn: bool,
    ) -> String {
        let color = if badge.eq_ignore_ascii_case("on time") {
            "green"
        } else {
            "red"
        };
        let src_m = if src {
            r#"<font size="2"><b>&nbsp;SRC&nbsp;&nbsp;</b></font>"#
        } else {
            ""
        };
        let dstn_m = if dstn {
            r#"<b>&nbsp;DSTN&nbsp;&nbsp;</b>"#
        } else {
            ""
        };
        format!(
            r#"<div class=" w3-card-2" style="width:100%;">
<div class="w3-container" style="float:left;width:100px;text-align:right;">
  <b><font size="1">{arr_s}</font></b><br>
  <font size="1" color="{color}" ><b>{arr_a}</b><br><span class="w3-round w3-{color}" style="padding: 1px 4px;">{badge}</span></font><br>
  {src_m}
</div>
<div class="w3-container" style="float:left;width:100px;text-align:center;">
  <div class="w3-bar-block" style="width:100%; background-image:url('track_gray.png');"><i class="fa fa-circle" style="color:teal;"></i></div>
</div>
<div class="w3-container" style="float:right;flex:1;padding-left:0px;padding-right:0px;display:flex;">
  <div class="w3-container" style="float:left;flex:1;">
    <span><font size="1"><b>{name}</b><br> <div class="w3-container" style="flex:1;padding:0px;display:inline-block;width:100%;text-align: center;"> <div style="float:left;padding: 0px;"><b>{code} <span class="w3-round w3-orange" style="padding: 1px 4px;">PF {pf}*</span></b></div>
    <div style="float:right;padding: 0px;"><b>{km}</b> KMs</div>
    <button class="btn" type="button">Coach Position</button>
    <div class="w3-modal"><div style="float:left;text-align:left;"><b>ENG</b> Locomotive</div><div style="float:left;text-align:left;"><b>D1</b> AC Chair Car</div></div>
    </div>
  </div>
</div>
<div class="w3-container" style="float:right;text-align:right;">
  <span><b><font size="1" >{dep_s}</font></b></span><br>
  <span><font size="1" color="{color}" ><b>{dep_a}</b><br><span class="w3-round w3-{color}" style="padding: 1px 4px;">{badge}</span></font></span>
  {dstn_m}
</div>
</div>"#,
        )
    }

    /// The 12055-style popup: an upcoming not-started run (15-Aug) followed by
    /// the active run (14-Aug) mid-way between GHAZIABAD and MEERUT CITY.
    fn running_popup() -> String {
        let upcoming = |d: &str| {
            [
                spot_block(
                    "NDLS",
                    "NEW DELHI",
                    "",
                    "",
                    "On Time",
                    &format!("15:20 {d}"),
                    &format!("15:20 {d}"),
                    "9",
                    "0",
                    true,
                    false,
                ),
                spot_block(
                    "GZB",
                    "GHAZIABAD",
                    &format!("15:53 {d}"),
                    "",
                    "On Time",
                    &format!("15:55 {d}"),
                    "",
                    "1",
                    "26",
                    false,
                    false,
                ),
                spot_block(
                    "MTC",
                    "MEERUT CITY",
                    &format!("16:32 {d}"),
                    "",
                    "On Time",
                    &format!("16:34 {d}"),
                    "",
                    "3",
                    "74",
                    false,
                    false,
                ),
                spot_block(
                    "MOZ",
                    "MUZAFFARNAGAR",
                    &format!("17:12 {d}"),
                    "",
                    "On Time",
                    &format!("17:14 {d}"),
                    "",
                    "1",
                    "129",
                    false,
                    false,
                ),
                spot_block(
                    "DBD",
                    "DEOBAND",
                    &format!("17:32 {d}"),
                    "",
                    "On Time",
                    &format!("17:34 {d}"),
                    "",
                    "2",
                    "158",
                    false,
                    false,
                ),
                spot_block(
                    "TPZ",
                    "TAPRI",
                    &format!("18:09 {d}"),
                    "",
                    "On Time",
                    &format!("18:11 {d}"),
                    "",
                    "1",
                    "206",
                    false,
                    false,
                ),
                spot_block(
                    "RK",
                    "ROORKEE",
                    &format!("18:43 {d}"),
                    "",
                    "On Time",
                    &format!("18:45 {d}"),
                    "",
                    "1",
                    "233",
                    false,
                    false,
                ),
                spot_block(
                    "HW",
                    "HARIDWAR JN",
                    &format!("19:27 {d}"),
                    "",
                    "On Time",
                    &format!("19:32 {d}"),
                    "",
                    "1",
                    "274",
                    false,
                    false,
                ),
                spot_block(
                    "DDN",
                    "DEHRADOON",
                    &format!("21:05 {d}"),
                    "",
                    "On Time",
                    "",
                    "",
                    "3",
                    "324",
                    false,
                    true,
                ),
            ]
            .join("\n")
        };
        let running = |d: &str| {
            [
                spot_block(
                    "NDLS",
                    "NEW DELHI",
                    "",
                    "",
                    "On Time",
                    &format!("15:20 {d}"),
                    &format!("15:20 {d}"),
                    "9",
                    "0",
                    true,
                    false,
                ),
                spot_block(
                    "GZB",
                    "GHAZIABAD",
                    &format!("15:53 {d}"),
                    &format!("15:56 {d}"),
                    "3 Min",
                    &format!("15:55 {d}"),
                    &format!("15:58 {d}"),
                    "1",
                    "26",
                    false,
                    false,
                ),
                spot_block(
                    "MTC",
                    "MEERUT CITY",
                    &format!("16:32 {d}"),
                    "",
                    "On Time",
                    &format!("16:34 {d}"),
                    "",
                    "3",
                    "74",
                    false,
                    false,
                ),
                spot_block(
                    "MOZ",
                    "MUZAFFARNAGAR",
                    &format!("17:12 {d}"),
                    "",
                    "On Time",
                    &format!("17:14 {d}"),
                    "",
                    "1",
                    "129",
                    false,
                    false,
                ),
                spot_block(
                    "DBD",
                    "DEOBAND",
                    &format!("17:32 {d}"),
                    "",
                    "On Time",
                    &format!("17:34 {d}"),
                    "",
                    "2",
                    "158",
                    false,
                    false,
                ),
                spot_block(
                    "TPZ",
                    "TAPRI",
                    &format!("18:09 {d}"),
                    "",
                    "On Time",
                    &format!("18:11 {d}"),
                    "",
                    "1",
                    "206",
                    false,
                    false,
                ),
                spot_block(
                    "RK",
                    "ROORKEE",
                    &format!("18:43 {d}"),
                    "",
                    "On Time",
                    &format!("18:45 {d}"),
                    "",
                    "1",
                    "233",
                    false,
                    false,
                ),
                spot_block(
                    "HW",
                    "HARIDWAR JN",
                    &format!("19:27 {d}"),
                    "",
                    "On Time",
                    &format!("19:32 {d}"),
                    "",
                    "1",
                    "274",
                    false,
                    false,
                ),
                spot_block(
                    "DDN",
                    "DEHRADOON",
                    &format!("21:05 {d}"),
                    "",
                    "On Time",
                    "",
                    "",
                    "3",
                    "324",
                    false,
                    true,
                ),
            ]
            .join("\n")
        };
        format!(
            "<html><h3>12055 DDN JANSHTBDI</h3>\
             <div class=\"tab-pane \" id=\"train15-aug-2026\">\
             <h5>NEW DELHI (NDLS) - DEHRADOON (DDN)</h5>\
             <div class=\"w3-container\" style=\"width:100%;\"><h6 class =\"text-secondary\"><b>Yet to start from its source</b></h6></div>\
             <div class=\" w3-card-2 w3-sand\" style=\"width:100%;\"><div style=\"width:100%;\"><font size=\"2\" color=\"green\"><b>Yet to start from its source</b></font></div></div>\
             {up15}\
             </div>\
             <div class=\"tab-pane active\" id=\"train14-aug-2026\">\
             <h5>NEW DELHI (NDLS) - DEHRADOON (DDN)</h5>\
             <div class=\"w3-container\" style=\"width:100%;\"><h6 class =\"text-primary\"><b>Departed from GHAZIABAD(GZB) at 15:58 14-Aug (Delay: 00:03)</b></h6></div>\
             <div class=\" w3-card-2 w3-sand\" style=\"width:100%;\"><div style=\"width:100%;\" id=\"currPos14-aug-2026\"><font size=\"2\" color=\"green\"><b>Departed from GHAZIABAD(GZB) at 15:58 14-Aug</b></font></div></div>\
             {run14}\
             </div></html>",
            up15 = upcoming("15-Aug"),
            run14 = running("14-Aug"),
        )
    }

    #[test]
    fn train_status_parses_running_run() {
        let norm = parse_train_status(&running_popup()).unwrap();
        assert_eq!(norm["train_number"], "12055");
        assert_eq!(norm["train_name"], "DDN JANSHTBDI");
        assert_eq!(norm["source_stn_name"], "NEW DELHI");
        assert_eq!(norm["dest_stn_name"], "DEHRADOON");
        assert_eq!(norm["train_start_date"], "14-Aug-2026");
        assert_eq!(norm["next_station_code"], "MTC");
        assert_eq!(norm["next_station_name"], "MEERUT CITY");
        assert_eq!(norm["platform_number"], "3");
        assert_eq!(norm["at_src"], "false");
        assert_eq!(norm["at_dstn"], "false");
        assert_eq!(norm["data_source"], "NTES");

        let instances = norm["instances"].as_array().unwrap();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0]["start_date"], "15-Aug-2026");
        assert_eq!(instances[0]["position"], "Yet to start from its source");
        assert_eq!(
            instances[0]["at_src"], "true",
            "upcoming run sits at its origin"
        );
        assert_eq!(
            instances[0]["next_station_code"], "NDLS",
            "upcoming run's next station is the source"
        );
        let up_stops = instances[0]["stops"].as_array().unwrap();
        assert_eq!(up_stops.len(), 9, "every instance carries its own timeline");
        assert_eq!(up_stops[1]["code"], "GZB");
        assert_eq!(
            up_stops[1]["actual_arrival"], "",
            "not started -> no actual"
        );
        assert_eq!(instances[1]["start_date"], "14-Aug-2026");
        assert!(instances[1]["position"]
            .as_str()
            .unwrap()
            .contains("Departed from GHAZIABAD(GZB)"));
        let run_stops = instances[1]["stops"].as_array().unwrap();
        assert_eq!(run_stops[1]["actual_arrival"], "15:56");
        assert_eq!(run_stops[1]["delay_minutes"], 3);

        let stops = norm["stops"].as_array().unwrap();
        assert_eq!(stops.len(), 9);
        // Source arrival falls back to the origin departure, like the
        // live-station rows do.
        assert_eq!(stops[0]["arrival"], "15:20");
        assert_eq!(stops[0]["actual_arrival"], "15:20");
        assert_eq!(stops[0]["delay_minutes"], 0);
        assert_eq!(stops[1]["code"], "GZB");
        assert_eq!(stops[1]["arrival"], "15:53");
        assert_eq!(stops[1]["actual_arrival"], "15:56");
        assert_eq!(stops[1]["delay_minutes"], 3);
        assert_eq!(stops[1]["platform"], "1");
        assert_eq!(
            stops[2]["actual_arrival"], "",
            "not yet reached -> no actual"
        );
        assert_eq!(stops[8]["code"], "DDN");
        assert_eq!(stops[8]["arrival"], "21:05");
    }

    #[test]
    fn train_status_marks_not_started_train_at_source() {
        let popup = format!(
            "<h3>22478 VANDE BHARAT EXP</h3>\
             <div class=\"tab-pane active\" id=\"train15-aug-2026\">\
             <h5>SHRI MATA VAISHNO DEVI KATRA (SVDK) - NEW DELHI (NDLS)</h5>\
             <h6 class =\"text-secondary\"><b>Yet to start from its source</b></h6>\
             <div class=\" w3-card-2 w3-sand\" style=\"width:100%;\"><div style=\"width:100%;\" id=\"currPos15-aug-2026\"><font size=\"2\"><b>Yet to start from its source</b></font></div></div>\
             {stops}\
             </div>",
            stops = [
                spot_block("SVDK", "SHRI MATA VAISHNO DEVI KATRA", "", "", "On Time", "05:45 15-Aug", "", "1", "0", true, false),
                spot_block("MCTM", "MARTYR CAPTAIN TUSHAR MAHAJAN", "06:14 15-Aug", "", "On Time", "06:16 15-Aug", "", "3", "19", false, false),
                spot_block("JAT", "JAMMUTAVI", "07:10 15-Aug", "", "On Time", "07:12 15-Aug", "", "1", "59", false, false),
                spot_block("NDLS", "NEW DELHI", "14:00 15-Aug", "", "On Time", "", "", "3", "597", false, true),
            ]
            .join("\n")
        );
        let norm = parse_train_status(&popup).unwrap();
        assert_eq!(norm["train_start_date"], "15-Aug-2026");
        assert_eq!(norm["at_src"], "true");
        assert_eq!(norm["at_dstn"], "false");
        // The next-station widget points at the origin until the train moves.
        assert_eq!(norm["next_station_code"], "SVDK");
        assert_eq!(norm["platform_number"], "1");
        for stop in norm["stops"].as_array().unwrap() {
            assert_eq!(stop["actual_arrival"], "");
        }
    }

    #[test]
    fn train_status_completed_run_is_at_destination() {
        let popup = format!(
            "<h3>12055 DDN JANSHTBDI</h3>\
             <div class=\"tab-pane active\" id=\"train13-aug-2026\">\
             <h5>NEW DELHI (NDLS) - DEHRADOON (DDN)</h5>\
             <h6 class =\"text-success\"><b>Arrived at DEHRADOON(DDN) at 21:23 13-Aug (Delay: 00:18)</b></h6>\
             {stops}\
             </div>",
            stops = [
                spot_block("NDLS", "NEW DELHI", "", "", "On Time", "15:20 13-Aug", "15:20 13-Aug", "9", "0", true, false),
                spot_block("GZB", "GHAZIABAD", "15:53 13-Aug", "15:53 13-Aug", "On Time", "15:55 13-Aug", "15:55 13-Aug", "1", "26", false, false),
                spot_block("DDN", "DEHRADOON", "21:05 13-Aug", "21:23 13-Aug", "18 Min", "", "", "3", "324", false, true),
            ]
            .join("\n")
        );
        let norm = parse_train_status(&popup).unwrap();
        assert_eq!(norm["at_src"], "false");
        assert_eq!(norm["at_dstn"], "true");
        assert_eq!(norm["train_start_date"], "13-Aug-2026");
        let stops = norm["stops"].as_array().unwrap();
        assert_eq!(stops[2]["actual_arrival"], "21:23");
        assert_eq!(stops[2]["delay_minutes"], 18);
    }

    #[test]
    fn train_status_parses_older_completed_run_instance() {
        // A completed 13-Aug active pane plus an older 12-Aug run still
        // mid-journey: each instance keeps its own position fields and
        // timeline so a date selection can switch between them.
        let run13 = [
            spot_block(
                "NDLS",
                "NEW DELHI",
                "",
                "",
                "On Time",
                "15:20 13-Aug",
                "15:20 13-Aug",
                "9",
                "0",
                true,
                false,
            ),
            spot_block(
                "GZB",
                "GHAZIABAD",
                "15:53 13-Aug",
                "15:53 13-Aug",
                "On Time",
                "15:55 13-Aug",
                "15:55 13-Aug",
                "1",
                "26",
                false,
                false,
            ),
            spot_block(
                "DDN",
                "DEHRADOON",
                "21:05 13-Aug",
                "21:23 13-Aug",
                "18 Min",
                "",
                "",
                "3",
                "324",
                false,
                true,
            ),
        ]
        .join("\n");
        let run12 = [
            spot_block(
                "NDLS",
                "NEW DELHI",
                "",
                "",
                "On Time",
                "15:20 12-Aug",
                "15:20 12-Aug",
                "9",
                "0",
                true,
                false,
            ),
            spot_block(
                "GZB",
                "GHAZIABAD",
                "15:53 12-Aug",
                "15:56 12-Aug",
                "3 Min",
                "15:55 12-Aug",
                "15:58 12-Aug",
                "1",
                "26",
                false,
                false,
            ),
            spot_block(
                "MTC",
                "MEERUT CITY",
                "16:32 12-Aug",
                "",
                "On Time",
                "16:34 12-Aug",
                "",
                "3",
                "74",
                false,
                false,
            ),
            spot_block(
                "DDN",
                "DEHRADOON",
                "21:05 12-Aug",
                "",
                "On Time",
                "",
                "",
                "3",
                "324",
                false,
                true,
            ),
        ]
        .join("\n");
        let popup = format!(
            "<html><h3>12055 DDN JANSHTBDI</h3>\
             <div class=\"tab-pane active\" id=\"train13-aug-2026\">\
             <h5>NEW DELHI (NDLS) - DEHRADOON (DDN)</h5>\
             <h6 class =\"text-success\"><b>Arrived at DEHRADOON(DDN) at 21:23 13-Aug (Delay: 00:18)</b></h6>\
             {run13}\
             </div>\
             <div class=\"tab-pane \" id=\"train12-aug-2026\">\
             <h5>NEW DELHI (NDLS) - DEHRADOON (DDN)</h5>\
             <h6 class =\"text-primary\"><b>Departed from GHAZIABAD(GZB) at 15:58 12-Aug (Delay: 00:03)</b></h6>\
             {run12}\
             </div></html>"
        );
        let norm = parse_train_status(&popup).unwrap();
        let instances = norm["instances"].as_array().unwrap();
        assert_eq!(instances[0]["start_date"], "13-Aug-2026");
        assert_eq!(
            instances[0]["at_dstn"], "true",
            "completed run is at destination"
        );
        assert_eq!(instances[0]["stops"][2]["actual_arrival"], "21:23");
        assert_eq!(instances[0]["stops"][2]["delay_minutes"], 18);
        assert_eq!(instances[1]["start_date"], "12-Aug-2026");
        assert_eq!(instances[1]["at_src"], "false");
        assert_eq!(instances[1]["next_station_code"], "MTC");
        assert_eq!(instances[1]["stops"][1]["actual_arrival"], "15:56");
    }

    #[test]
    fn train_status_shell_without_panes_is_none() {
        assert_eq!(
            parse_train_status(
                "<html><h3>Spot Your Train</h3>\
                 <div class='tab-pane container fade ' id='s1'><h3>&nbsp;</h3></div>"
            ),
            None
        );
    }

    #[test]
    fn train_status_badge_minutes_parses_spellings() {
        assert_eq!(badge_minutes("On Time"), 0);
        assert_eq!(badge_minutes("3 Min"), 3);
        assert_eq!(badge_minutes("41 Mins."), 41);
        assert_eq!(badge_minutes("1 Hr 26 Min"), 86);
        assert_eq!(badge_minutes("02:10"), 130);
        assert_eq!(badge_minutes(""), 0);
    }

    #[test]
    fn train_status_parses_real_ntes_popup() {
        let html = std::fs::read_to_string("testdata/ntes_spot_train_12055.html").unwrap();
        let norm = parse_train_status(&html).unwrap();
        assert_eq!(norm["train_number"], "12055");
        assert_eq!(norm["train_name"], "DDN JANSHTBDI");
        assert_eq!(norm["train_start_date"], "14-Aug-2026");
        assert_eq!(norm["next_station_code"], "MTC");
        assert_eq!(norm["next_station_name"], "MEERUT CITY");
        assert_eq!(norm["platform_number"], "3");
        assert_eq!(norm["at_src"], "false");
        assert_eq!(norm["at_dstn"], "false");
        assert_eq!(norm["source_stn_name"], "NEW DELHI");
        assert_eq!(norm["dest_stn_name"], "DEHRADOON");

        let instances = norm["instances"].as_array().unwrap();
        assert_eq!(instances.len(), 5, "five runs reported by NTES");
        assert_eq!(instances[1]["start_date"], "14-Aug-2026");

        let stops = norm["stops"].as_array().unwrap();
        assert_eq!(stops.len(), 9);
        assert_eq!(stops[0]["code"], "NDLS");
        assert_eq!(stops[1]["code"], "GZB");
        assert_eq!(stops[1]["actual_arrival"], "15:56");
        assert_eq!(stops[1]["delay_minutes"], 3);
        assert_eq!(stops[2]["code"], "MTC");
        assert_eq!(
            stops[2]["actual_arrival"], "",
            "expected stop has no real actual"
        );
        assert_eq!(stops[2]["platform"], "3");
        assert_eq!(stops[8]["code"], "DDN");
    }

    #[test]
    fn train_status_unknown_train_shell_is_none() {
        let html = std::fs::read_to_string("testdata/ntes_spot_train_unknown.html").unwrap();
        assert_eq!(parse_train_status(&html), None);
    }

    // -- journey-station basis / train-on-map ---------------------------------

    #[test]
    fn journey_stations_parses_option_list() {
        let html = r#"<input type="text" name="trainNo" id="trainNo" size="6" value="12055">
        <select  name="jStation" class="form-control"  id="jStation" onchange="onJourneyStationInput();">
          <option value="">---Select---</option>
          <option title="DAILY#DAILY" value="NDLS#false#1" >NEW DELHI - NDLS</option>
          <option title="MON,WED#TUE,THU" value="ABC#true#5" >SOME PLACE - ABC</option>
          <option title="DAILY#DAILY" value="DDN#false#53" >DEHRADOON - DDN</option>
        </select>"#;
        let data = parse_journey_stations(html).unwrap();
        assert_eq!(data["trainNo"], "12055");
        let list = data["list"].as_array().unwrap();
        assert_eq!(
            list.len(),
            3,
            "placeholder option without a title is skipped"
        );
        assert_eq!(list[0]["code"], "NDLS");
        assert_eq!(list[0]["name"], "NEW DELHI");
        assert_eq!(list[0]["seq"], 1, "seq serializes as a number");
        assert_eq!(list[0]["dayChange"], false);
        assert_eq!(list[0]["arrivalDays"], "DAILY");
        assert_eq!(list[0]["departureDays"], "DAILY");
        assert_eq!(list[1]["code"], "ABC");
        assert_eq!(list[1]["name"], "SOME PLACE");
        assert_eq!(list[1]["seq"], 5);
        assert_eq!(list[1]["dayChange"], true);
        assert_eq!(list[1]["arrivalDays"], "MON,WED");
        assert_eq!(list[1]["departureDays"], "TUE,THU");
        assert_eq!(list[2]["code"], "DDN");
        assert_eq!(list[2]["name"], "DEHRADOON");
    }

    #[test]
    fn journey_stations_shell_is_none() {
        let shell = "<table><tr><th>No data</th></tr></table>";
        assert_eq!(parse_journey_stations(shell), None);
    }

    #[test]
    fn journey_basis_parses_showruncstn_page() {
        // ShowRunCStn pane ids carry a trailing "1" (`train17-aug-20261`) that
        // the shared parse_train_status parser must tolerate.
        let popup = format!(
            "<html><h3>12055 DDN JANSHTBDI</h3>\
             <div class=\"tab-pane active\" id=\"train17-aug-20261\">\
             <h5>NEW DELHI (NDLS) - DEHRADOON (DDN)</h5>\
             <h6 class =\"text-primary\"><b>Departed from GHAZIABAD(GZB) at 15:58 17-Aug (Delay: 00:03)</b></h6>\
             {stops}\
             </div></html>",
            stops = [
                spot_block("NDLS", "NEW DELHI", "", "", "On Time", "15:20 17-Aug", "15:20 17-Aug", "9", "0", true, false),
                spot_block("GZB", "GHAZIABAD", "15:53 17-Aug", "15:56 17-Aug", "3 Min", "15:55 17-Aug", "15:58 17-Aug", "1", "26", false, false),
                spot_block("MTC", "MEERUT CITY", "16:32 17-Aug", "", "On Time", "16:34 17-Aug", "", "3", "74", false, false),
                spot_block("DDN", "DEHRADOON", "21:05 17-Aug", "", "On Time", "", "", "3", "324", false, true),
            ]
            .join("\n")
        );
        let norm = parse_train_status(&popup).unwrap();
        assert_eq!(norm["train_number"], "12055");
        assert_eq!(norm["train_name"], "DDN JANSHTBDI");
        assert_eq!(norm["train_start_date"], "17-Aug-2026");
        assert_eq!(norm["source_stn_name"], "NEW DELHI");
        assert_eq!(norm["dest_stn_name"], "DEHRADOON");
        assert_eq!(norm["at_src"], "false");
        assert_eq!(norm["at_dstn"], "false");
        assert_eq!(norm["next_station_code"], "MTC");
        let stops = norm["stops"].as_array().unwrap();
        assert_eq!(stops.len(), 4);
        assert_eq!(stops[1]["code"], "GZB");
        assert_eq!(stops[1]["actual_arrival"], "15:56");
        assert_eq!(stops[1]["delay_minutes"], 3);
    }

    #[test]
    fn train_route_map_parses_js_blocks() {
        let html = r##"
var myStns = ["NDLS","GZB","MTC","DDN"];
var myStnsF = ["NDLS","CSB","TKJ","GZB","MTC","DDN"];
var myStnNames = ["NEW DELHI","GHAZIABAD","MEERUT CITY","DEHRADOON"];
var train=["12055","DDN JANSHTBDI","NEW DELHI","DEHRADOON","NDLS","DDN",""];
var runInfo = ["#15:20#1#0#Daily#Daily","15:53#15:55#1#26#Daily#Daily","16:32#16:34#1#73#Daily#Daily","21:05##1#305#Daily#Daily"];
"##;
        let data = parse_train_route_map(html).unwrap();
        assert_eq!(data["trainNo"], "12055");
        assert_eq!(data["trainName"], "DDN JANSHTBDI");
        assert_eq!(data["source"], "NEW DELHI");
        assert_eq!(data["destination"], "DEHRADOON");
        assert_eq!(data["sourceCode"], "NDLS");
        assert_eq!(data["destCode"], "DDN");
        assert_eq!(data["startDate"], "");
        let route = data["route"].as_array().unwrap();
        assert_eq!(route.len(), 4);
        assert_eq!(route[0]["code"], "NDLS");
        assert_eq!(route[0]["name"], "NEW DELHI");
        assert_eq!(route[0]["arrival"], "");
        assert_eq!(route[0]["departure"], "15:20");
        assert_eq!(route[0]["day"], "1");
        assert_eq!(route[0]["distance"], "0");
        assert_eq!(route[0]["daysOfRun"], "Daily");
        assert_eq!(route[3]["code"], "DDN");
        assert_eq!(route[3]["name"], "DEHRADOON");
        assert_eq!(route[3]["arrival"], "21:05");
        assert_eq!(route[3]["departure"], "");
        let track = data["track"].as_array().unwrap();
        assert_eq!(track.len(), 6);
        assert_eq!(track[0], "NDLS");
        assert_eq!(track[1], "CSB");
        assert_eq!(track[3], "GZB");
    }

    #[test]
    fn train_route_map_shell_is_none() {
        assert_eq!(
            parse_train_route_map("<html><body>No data</body></html>"),
            None
        );
    }

    #[test]
    fn train_spot_map_parses_spot_js_blocks() {
        let html = r##"
var myStns = ["NDLS","GZB","DDN"];
var cStn = ["NDLS","--","--"];
var jStn = ["NDLS","New Delhi","","","<span class=blueS11L>Source</span>","17-Aug-2026 15:20","17-Aug-2026 15:20","On Time","9"];
var train=["12055","DDN JANSHTBDI","New Delhi","Dehradoon","NDLS","DDN","17-Aug-2026"];
var runInfo = ["Source|#17-Aug-2026 15:20|On Time","17-Aug-2026 15:53|On Time#17-Aug-2026 15:55|On Time","17-Aug-2026 21:05|On Time#Destination|"];
"##;
        let data = parse_train_spot_map(html).unwrap();
        assert_eq!(data["trainNo"], "12055");
        assert_eq!(data["trainName"], "DDN JANSHTBDI");
        assert_eq!(data["source"], "New Delhi");
        assert_eq!(data["destination"], "Dehradoon");
        assert_eq!(data["sourceCode"], "NDLS");
        assert_eq!(data["destCode"], "DDN");
        assert_eq!(data["startDate"], "17-Aug-2026");
        assert_eq!(data["currentStation"]["code"], "NDLS");
        assert_eq!(data["journeyStation"]["code"], "NDLS");
        assert_eq!(data["journeyStation"]["name"], "New Delhi");
        assert_eq!(
            data["journeyStation"]["label"], "Source",
            "label HTML tags stripped"
        );
        assert_eq!(
            data["journeyStation"]["expectedArrival"],
            "17-Aug-2026 15:20"
        );
        assert_eq!(data["journeyStation"]["actualArrival"], "17-Aug-2026 15:20");
        assert_eq!(data["journeyStation"]["delayStatus"], "On Time");
        assert_eq!(data["journeyStation"]["platform"], "9");
        let status = data["status"].as_array().unwrap();
        assert_eq!(status.len(), 3);
        assert_eq!(status[0]["code"], "NDLS");
        assert_eq!(status[0]["expectedArrival"], "", "Source marker is blanked");
        assert_eq!(status[0]["expectedDeparture"], "17-Aug-2026 15:20");
        assert_eq!(status[0]["arrivalDelay"], "");
        assert_eq!(status[0]["departureDelay"], "On Time");
        assert_eq!(status[1]["code"], "GZB");
        assert_eq!(status[1]["expectedArrival"], "17-Aug-2026 15:53");
        assert_eq!(status[1]["expectedDeparture"], "17-Aug-2026 15:55");
        assert_eq!(status[1]["arrivalDelay"], "On Time");
        assert_eq!(status[1]["departureDelay"], "On Time");
        assert_eq!(status[2]["code"], "DDN");
        assert_eq!(status[2]["expectedArrival"], "17-Aug-2026 21:05");
        assert_eq!(
            status[2]["expectedDeparture"], "",
            "Destination marker is blanked"
        );
        assert_eq!(status[2]["arrivalDelay"], "On Time");
        assert_eq!(status[2]["departureDelay"], "");
    }

    #[test]
    fn train_spot_map_shell_is_none() {
        assert_eq!(
            parse_train_spot_map("<html><body>No data</body></html>"),
            None
        );
    }

    #[tokio::test]
    async fn blocked_endpoints_are_honest_source_unavailable() {
        // localhost port is closed: hermetic, fails fast, no real network.
        let c = NtesWebClient::new(&http(), "http://127.0.0.1:1");
        assert!(matches!(
            c.train_exceptions("04138").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.live_station("NDLS", "NEW DELHI", 2).await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.trains_between("NDLS", "NEW DELHI", "MMCT", "MUMBAI CENTRAL").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.train_status("12055").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.station_timetable("NDLS", "NEW DELHI", Some("15-Aug-2026")).await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.average_delay("12055").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.heritage_trains(0).await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.parcel_special_trains().await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.journey_stations("12055").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.journey_station_basis("12055", "NDLS#false#1").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.train_route_map("12055", "17-Aug-2026").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
        assert!(matches!(
            c.train_spot_map("12055", "NDLS#false#1", "17-Aug-2026", "A").await,
            Err(AppError::SourceUnavailable { source, .. }) if source == "ntes"
        ));
    }

    #[test]
    fn probe_showruncstn_parses_with_existing_parser() {
        // Live NTES capture kept outside the repo; skip gracefully when absent.
        let Some(html) = std::fs::read_to_string("/tmp/jstn.html")
            .or_else(|_| {
                std::fs::read_to_string("/home/runner/workspace/.agents/fixtures/jstn.html")
            })
            .ok()
        else {
            return;
        };
        let norm = parse_train_status(&html);
        match norm {
            None => panic!("ShowRunCStn response did NOT parse with parse_train_status"),
            Some(n) => {
                println!(
                    "OK train={} instances={} stops={}",
                    n["train_number"],
                    n["instances"].as_array().unwrap().len(),
                    n["stops"].as_array().unwrap().len()
                );
            }
        }
    }

    #[test]
    fn probe_excpinfo_parses_with_new_parser() {
        // Real NTES per-train exception calendars captured live; skip
        // gracefully when the fixture is absent from the repo.
        let cases = [
            (
                "/home/runner/workspace/.agents/fixtures/mntes_excpinfo.html",
                "04138",
                false,
            ),
            (
                "/home/runner/workspace/.agents/fixtures/mntes_excpinfo_nodata.html",
                "12951",
                true,
            ),
        ];
        for (path, train, expect_no_data) in cases {
            let Ok(html) = std::fs::read_to_string(path) else {
                return;
            };
            let data = parse_train_exceptions(&html, train)
                .unwrap_or_else(|| panic!("excpInfo fixture {path} did not parse"));
            assert_eq!(data["noData"], expect_no_data);
            println!(
                "OK train={} noData={} exceptions={}",
                data["train"]["number"],
                data["noData"],
                data["exceptions"].as_array().unwrap().len()
            );
            if !expect_no_data {
                let exceptions = data["exceptions"].as_array().unwrap();
                assert!(
                    exceptions.iter().any(|e| e["kind"] == "cancelled"),
                    "fixture should contain at least one cancelled date"
                );
            }
        }
    }
}
