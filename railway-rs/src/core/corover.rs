//! AskDISHA / CoRover guest read-only client (`api.disha.corover.ai`).
//!
//! Implements only the wave-0 verified headerless endpoints (see
//! `docs/ASKDISHA_MODULE.md`): station search, train schedule enquiry and the
//! two CDN bucket files (`{lang}.json` FAQ arrays, `getSettings.json` flags).
//! The chat surface (`sendQuery`/dSession/RSA) is deliberately out of scope -
//! upstream answers 401 even with a valid session, so there is no code path
//! for it here.
//!
//! Error handling mirrors [`crate::core::http`]: one retry for transient
//! failures (timeouts, connection errors, 5xx) and honest
//! [`AppError::SourceUnavailable`] reporting tagged with the real source id
//! (`corover-api` / `corover-cdn`) - never fabricated data.

use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::error::AppError;

/// Source id reported for calls against `api.disha.corover.ai`.
pub const SOURCE_API: &str = "corover-api";
/// Source id reported for calls against the `cdn.corover.ai` bucket.
pub const SOURCE_CDN: &str = "corover-cdn";

/// Desktop Chrome UA; the CoRover API sits behind edge bot filtering, so a
/// browser-like UA is sent even though these endpoints are headerless.
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

/// Pause before the single retry of a transient failure (matches `HttpClient`).
const RETRY_DELAY: Duration = Duration::from_millis(400);

/// Guest-mode client for the AskDISHA enquiry endpoints and CDN bucket.
///
/// Cheap to clone (`reqwest::Client` shares its connection pool); designed to
/// live in `AppState` behind an `Option<Arc<..>>` gated by `ASKDISHA_ENABLED`.
#[derive(Clone)]
pub struct CoroverClient {
    http: reqwest::Client,
    /// e.g. `https://api.disha.corover.ai` (no trailing slash, no path).
    corover_base: String,
    /// e.g. `https://cdn.corover.ai` (bucket path is appended per call).
    cdn_base: String,
}

impl CoroverClient {
    /// Build a client rooted at `corover_base` (API origin) and `cdn_base`
    /// (CDN origin, without the `askdisha-bucket/` suffix). Trailing slashes
    /// on either base are stripped.
    pub fn new(
        corover_base: impl Into<String>,
        cdn_base: impl Into<String>,
        timeout: Duration,
    ) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(BROWSER_UA)
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(8))
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http: client,
            corover_base: corover_base.into().trim_end_matches('/').to_string(),
            cdn_base: cdn_base.into().trim_end_matches('/').to_string(),
        }
    }

    /// Typeahead station search: `GET /dishaAPI/bot/searchStation/{q}`.
    ///
    /// `q` is percent-encoded as a path segment (it may contain spaces or
    /// unicode from IME input). Returns up to the upstream cap of rows;
    /// callers apply their own limit.
    pub async fn search_station(&self, q: &str) -> Result<Vec<StationRow>, AppError> {
        let url = format!(
            "{}/dishaAPI/bot/searchStation/{}",
            self.corover_base,
            urlencoding::encode(q)
        );
        self.get_json_retry(&url, SOURCE_API).await
    }

    /// Train schedule enquiry: `GET /dishaAPI/bot/trnscheduleEnq/{train_no}`.
    ///
    /// `journey_date` (`YYYY-MM-DD`) and `from_code` (boarding station code)
    /// are appended as `journeyDate` / `startingStationCode` query params only
    /// when present, exactly like the web client omits them.
    pub async fn trnschedule_enq(
        &self,
        train_no: &str,
        journey_date: Option<&str>,
        from_code: Option<&str>,
    ) -> Result<ScheduleResponse, AppError> {
        let mut url = format!(
            "{}/dishaAPI/bot/trnscheduleEnq/{}",
            self.corover_base,
            urlencoding::encode(train_no)
        );
        let mut params: Vec<String> = Vec::with_capacity(2);
        if let Some(date) = journey_date {
            params.push(format!("journeyDate={}", urlencoding::encode(date)));
        }
        if let Some(from) = from_code {
            params.push(format!("startingStationCode={}", urlencoding::encode(from)));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        self.get_json_retry(&url, SOURCE_API).await
    }

    /// FAQ strings for `lang` (`en` | `hi` | `gu`; route layer validates):
    /// `GET {cdn_base}/askdisha-bucket/{lang}.json`.
    pub async fn fetch_faqs(&self, lang: &str) -> Result<Vec<String>, AppError> {
        let url = format!(
            "{}/askdisha-bucket/{}.json",
            self.cdn_base,
            urlencoding::encode(lang)
        );
        self.get_json_retry(&url, SOURCE_CDN).await
    }

    /// Feature-flag document: `GET {cdn_base}/askdisha-bucket/getSettings.json`.
    pub async fn fetch_settings(&self) -> Result<SettingsFlag, AppError> {
        let url = format!("{}/askdisha-bucket/getSettings.json", self.cdn_base);
        self.get_json_retry(&url, SOURCE_CDN).await
    }

    /// GET `url`, retry once on transient failures (timeout / connect error /
    /// 5xx), then decode the body as `T`. Every failure is mapped onto
    /// [`AppError::SourceUnavailable`] tagged with `source` so the API layer
    /// answers 502 honestly.
    async fn get_json_retry<T: DeserializeOwned>(
        &self,
        url: &str,
        source: &str,
    ) -> Result<T, AppError> {
        let mut last: Option<AppError> = None;
        for attempt in 0..2 {
            match self.http.get(url).send().await {
                Ok(res) => {
                    let status = res.status();
                    if status.is_success() {
                        let bytes = res.bytes().await.map_err(|e| {
                            AppError::source_unavailable(source, format!("read body of {url}: {e}"))
                        })?;
                        return serde_json::from_slice(&bytes).map_err(|e| {
                            AppError::source_unavailable(
                                source,
                                format!("invalid JSON from {url}: {e}"),
                            )
                        });
                    }
                    last = Some(AppError::source_unavailable(
                        source,
                        format!("GET {url} returned {status}"),
                    ));
                    // 4xx will not improve on retry - bail out immediately.
                    if !status.is_server_error() {
                        return Err(last.unwrap_or_else(|| AppError::internal("GET failed")));
                    }
                }
                Err(e) => {
                    let transient = e.is_timeout() || e.is_connect() || e.is_request();
                    last = Some(AppError::source_unavailable(
                        source,
                        format!("GET {url}: {e}"),
                    ));
                    if !transient {
                        return Err(last.unwrap_or_else(|| AppError::internal("GET failed")));
                    }
                }
            }
            if attempt == 0 {
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
        Err(last.unwrap_or_else(|| AppError::internal("GET failed")))
    }
}

/// Deserialize an upstream `"Y"`/`"N"` string into a `bool`.
///
/// The AskDISHA schedule payload reports run-day flags as `"Y"`/`"N"`
/// strings (case-insensitive tolerated); a native JSON boolean is accepted
/// too so mirror deployments still parse. Serialized back as a plain bool.
fn de_yn_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match <serde_json::Value as Deserialize>::deserialize(deserializer)? {
        serde_json::Value::Bool(b) => Ok(b),
        serde_json::Value::String(s) => match s.to_ascii_uppercase().as_str() {
            "Y" => Ok(true),
            "N" => Ok(false),
            other => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(other),
                &r#""Y" or "N""#,
            )),
        },
        other => Err(serde::de::Error::invalid_type(
            match other {
                serde_json::Value::Null => serde::de::Unexpected::Unit,
                serde_json::Value::Number(n) => {
                    serde::de::Unexpected::Signed(n.as_i64().unwrap_or_default())
                }
                serde_json::Value::Array(_) => serde::de::Unexpected::Seq,
                serde_json::Value::Object(_) => serde::de::Unexpected::Map,
                _ => unreachable!("string and bool handled above"),
            },
            &r#""Y"/"N" string or boolean"#,
        )),
    }
}

/// Deserialize an optional `f64` where upstream emits an empty string or
/// `null` for "no value" (real `searchStation` captures contain
/// `"latitude": ""`). Numeric strings are tolerated as well.
fn de_opt_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match <serde_json::Value as Deserialize>::deserialize(deserializer)? {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(n) => Ok(n.as_f64()),
        serde_json::Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                return Ok(None);
            }
            t.parse::<f64>().map(Some).map_err(|_| {
                serde::de::Error::invalid_value(
                    serde::de::Unexpected::Str(&s),
                    &"a number, empty string or null",
                )
            })
        }
        other => Err(serde::de::Error::invalid_type(
            match other {
                serde_json::Value::Bool(_) => serde::de::Unexpected::Bool(false),
                serde_json::Value::Array(_) => serde::de::Unexpected::Seq,
                serde_json::Value::Object(_) => serde::de::Unexpected::Map,
                _ => unreachable!("null, number and string handled above"),
            },
            &"a number, empty string or null",
        )),
    }
}

/// One row of `searchStation` typeahead output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationRow {
    pub name: String,
    pub code: String,
    #[serde(default)]
    pub utterances: Vec<String>,
    // Upstream keys are snake_case (`name_hi`) even though the rest of the
    // payload is camelCase, so these two pin their wire names explicitly
    // (the `alias` keeps the camelCase spelling accepted as well).
    #[serde(default, rename = "name_hi", alias = "nameHi")]
    pub name_hi: Option<String>,
    #[serde(default, rename = "name_gu", alias = "nameGu")]
    pub name_gu: Option<String>,
    #[serde(default)]
    pub district: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub train_count: Option<String>,
    // Upstream sends "" / null for stations without coordinates.
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub latitude: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_f64")]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub address: Option<String>,
}

/// One halt of a train schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleStop {
    pub station_code: String,
    pub station_name: String,
    #[serde(default)]
    pub arrival_time: Option<String>,
    #[serde(default)]
    pub departure_time: Option<String>,
    #[serde(default)]
    pub route_number: Option<String>,
    #[serde(default)]
    pub halt_time: Option<String>,
}

/// `trnscheduleEnq` response. Run-day flags arrive as `"Y"`/`"N"` strings and
/// are normalized to booleans via [`de_yn_bool`]; they serialize back as
/// plain JSON booleans. Unknown upstream fields are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleResponse {
    pub train_number: String,
    #[serde(default)]
    pub train_name: Option<String>,
    #[serde(default)]
    pub station_from: Option<String>,
    #[serde(default)]
    pub station_to: Option<String>,
    #[serde(rename = "trainRunsOnMon", default, deserialize_with = "de_yn_bool")]
    pub runs_mon: bool,
    #[serde(rename = "trainRunsOnTue", default, deserialize_with = "de_yn_bool")]
    pub runs_tue: bool,
    #[serde(rename = "trainRunsOnWed", default, deserialize_with = "de_yn_bool")]
    pub runs_wed: bool,
    #[serde(rename = "trainRunsOnThu", default, deserialize_with = "de_yn_bool")]
    pub runs_thu: bool,
    #[serde(rename = "trainRunsOnFri", default, deserialize_with = "de_yn_bool")]
    pub runs_fri: bool,
    #[serde(rename = "trainRunsOnSat", default, deserialize_with = "de_yn_bool")]
    pub runs_sat: bool,
    #[serde(rename = "trainRunsOnSun", default, deserialize_with = "de_yn_bool")]
    pub runs_sun: bool,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub station_list: Vec<ScheduleStop>,
}

/// CDN `getSettings.json` feature flags (43-byte document upstream).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFlag {
    pub id: i64,
    pub is_disabled: bool,
    pub booking: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEDULE_SAMPLE: &str = r#"{
        "trainNumber": "12951",
        "trainName": "MUMBAI CENTRAL - NEW DELHI RAJDHANI EXPRESS",
        "stationFrom": "BCT",
        "stationTo": "NDLS",
        "trainRunsOnMon": "Y",
        "trainRunsOnTue": "N",
        "trainRunsOnWed": "Y",
        "trainRunsOnThu": "N",
        "trainRunsOnFri": "Y",
        "trainRunsOnSat": "N",
        "trainRunsOnSun": "Y",
        "someFutureField": { "ignored": true },
        "stationList": [
            {
                "stationCode": "BCT",
                "stationName": "Mumbai Central",
                "arrivalTime": "--",
                "departureTime": "17:00",
                "routeNumber": "1",
                "haltTime": "--"
            },
            {
                "stationCode": "NDLS",
                "stationName": "New Delhi",
                "arrivalTime": "08:32",
                "departureTime": "--"
            }
        ]
    }"#;

    const STATIONS_SAMPLE: &str = r#"[
        {
            "name": "AHMEDABAD JN",
            "code": "ADI",
            "utterances": ["Ahmedabad", "અમદાવાદ"],
            "name_hi": "अहमदाबाद जं.",
            "name_gu": "અમદાવાદ જં.",
            "district": "Ahmedabad",
            "state": "Gujarat",
            "trainCount": "373",
            "latitude": 23.022505,
            "longitude": 72.571365,
            "address": "Kalupur, Ahmedabad"
        },
        { "name": "NEW DELHI", "code": "NDLS" }
    ]"#;

    const SETTINGS_SAMPLE: &str = r#"{"id":1,"isDisabled":false,"booking":true}"#;

    const FAQS_SAMPLE: &str = r#"[
        "How can I book a ticket?",
        "मुझे PNR स्थिति कैसे देखनी चाहिए?",
        "Where is my refund?"
    ]"#;

    #[test]
    fn parses_schedule_with_yn_flags_and_serializes_bools_back() {
        let parsed: ScheduleResponse =
            serde_json::from_str(SCHEDULE_SAMPLE).expect("schedule sample parses");

        assert_eq!(parsed.train_number, "12951");
        assert_eq!(parsed.station_from.as_deref(), Some("BCT"));
        assert_eq!(parsed.station_to.as_deref(), Some("NDLS"));
        assert!(parsed.runs_mon && parsed.runs_wed && parsed.runs_fri && parsed.runs_sun);
        assert!(!parsed.runs_tue && !parsed.runs_thu && !parsed.runs_sat);
        assert_eq!(parsed.station_list.len(), 2);

        let first = &parsed.station_list[0];
        assert_eq!(first.station_code, "BCT");
        assert_eq!(first.departure_time.as_deref(), Some("17:00"));
        assert_eq!(first.halt_time.as_deref(), Some("--"));
        // Optional fields absent in the sample stay None.
        assert_eq!(parsed.station_list[1].route_number, None);

        // Unknown upstream field ignored, and flags serialize back as bools
        // (under their explicit upstream `trainRunsOn<Day>` wire names).
        let json = serde_json::to_string(&parsed).expect("serializes");
        assert!(!json.contains("someFutureField"));
        assert!(json.contains(r#""trainRunsOnMon":true"#));
        assert!(json.contains(r#""trainRunsOnTue":false"#));
        assert!(!json.contains("\"Y\""));

        // Round-trip: re-parsing our own serialization yields identical JSON.
        let reparsed: ScheduleResponse =
            serde_json::from_str(&json).expect("own serialization re-parses");
        let json2 = serde_json::to_string(&reparsed).expect("re-serializes");
        assert_eq!(json, json2);
    }

    #[test]
    fn parses_station_rows_with_localized_names_and_defaults() {
        let rows: Vec<StationRow> =
            serde_json::from_str(STATIONS_SAMPLE).expect("stations sample parses");

        assert_eq!(rows.len(), 2);
        let adi = &rows[0];
        assert_eq!(adi.name, "AHMEDABAD JN");
        assert_eq!(adi.code, "ADI");
        assert_eq!(adi.name_hi.as_deref(), Some("अहमदाबाद जं."));
        assert_eq!(adi.name_gu.as_deref(), Some("અમદાવાદ જં."));
        assert_eq!(adi.latitude, Some(23.022505));
        assert_eq!(adi.longitude, Some(72.571365));
        assert_eq!(adi.state.as_deref(), Some("Gujarat"));
        assert_eq!(adi.utterances.len(), 2);

        // Missing optionals default to None, utterances to empty vec.
        let ndls = &rows[1];
        assert_eq!(ndls.name_hi, None);
        assert_eq!(ndls.latitude, None);
        assert!(ndls.utterances.is_empty());

        // Round-trip through serialization (wire names preserved).
        let json = serde_json::to_string(&rows).expect("serializes");
        assert!(json.contains(r#""name_hi":"अहमदाबाद जं.""#));
        assert!(json.contains(r#""trainCount":"373""#));
        let reparsed: Vec<StationRow> = serde_json::from_str(&json).expect("re-parses");
        let json2 = serde_json::to_string(&reparsed).expect("re-serializes");
        assert_eq!(json, json2);
    }

    #[test]
    fn parses_settings_flags() {
        let settings: SettingsFlag =
            serde_json::from_str(SETTINGS_SAMPLE).expect("settings sample parses");
        assert_eq!(settings.id, 1);
        assert!(!settings.is_disabled);
        assert!(settings.booking);

        let json = serde_json::to_string(&settings).expect("serializes");
        assert_eq!(json, SETTINGS_SAMPLE);
    }

    #[test]
    fn parses_faq_strings() {
        let faqs: Vec<String> = serde_json::from_str(FAQS_SAMPLE).expect("faqs sample parses");
        assert_eq!(faqs.len(), 3);
        assert!(faqs[0].starts_with("How can I book"));
        assert_eq!(faqs[1], "मुझे PNR स्थिति कैसे देखनी चाहिए?");
    }

    #[test]
    fn station_row_tolerates_empty_string_and_null_coordinates() {
        let rows: Vec<StationRow> = serde_json::from_str(
            r#"[
                { "name": "A", "code": "AAA", "latitude": "", "longitude": null },
                { "name": "B", "code": "BBB" },
                { "name": "C", "code": "CCC", "latitude": "23.5", "longitude": 77.25 }
            ]"#,
        )
        .expect("tolerant coordinates parse");

        assert_eq!(rows[0].latitude, None);
        assert_eq!(rows[0].longitude, None);
        assert_eq!(rows[1].latitude, None);
        assert_eq!(rows[1].longitude, None);
        assert_eq!(rows[2].latitude, Some(23.5));
        assert_eq!(rows[2].longitude, Some(77.25));

        // Garbage strings still fail loudly rather than silently zeroing.
        let bad: Result<Vec<StationRow>, _> =
            serde_json::from_str(r#"[{ "name": "D", "code": "DDD", "latitude": "n/a" }]"#);
        assert!(bad.is_err());
    }

    #[derive(Deserialize)]
    struct YnProbe {
        #[serde(deserialize_with = "super::de_yn_bool")]
        v: bool,
    }

    fn yn(raw: serde_json::Value) -> bool {
        serde_json::from_value::<YnProbe>(serde_json::json!({ "v": raw }))
            .expect("probe parses")
            .v
    }

    #[test]
    fn de_yn_bool_maps_both_directions_and_rejects_garbage() {
        assert!(yn(serde_json::Value::String("Y".into())));
        assert!(!yn(serde_json::Value::String("N".into())));
        // Case-insensitive and native-bool tolerance.
        assert!(yn(serde_json::Value::Bool(true)));
        assert!(!yn(serde_json::Value::String("n".into())));

        let bad = serde_json::from_value::<YnProbe>(serde_json::json!({ "v": "maybe" }));
        assert!(bad.is_err());
    }

    #[test]
    fn url_building_encodes_segments_and_omits_absent_params() {
        let client = CoroverClient::new(
            "https://api.disha.corover.ai/",
            "https://cdn.corover.ai/",
            Duration::from_secs(10),
        );

        assert_eq!(client.corover_base, "https://api.disha.corover.ai");
        assert_eq!(client.cdn_base, "https://cdn.corover.ai");

        let stations_url = format!(
            "{}/dishaAPI/bot/searchStation/{}",
            client.corover_base,
            urlencoding::encode("new delhi")
        );
        assert_eq!(
            stations_url,
            "https://api.disha.corover.ai/dishaAPI/bot/searchStation/new%20delhi"
        );

        let base = format!(
            "{}/dishaAPI/bot/trnscheduleEnq/{}",
            client.corover_base,
            urlencoding::encode("12951")
        );
        let mut with_params = base.clone();
        with_params.push_str("?journeyDate=2026-08-22&startingStationCode=BCT");
        assert_eq!(with_params, "https://api.disha.corover.ai/dishaAPI/bot/trnscheduleEnq/12951?journeyDate=2026-08-22&startingStationCode=BCT");
        assert_eq!(
            base,
            "https://api.disha.corover.ai/dishaAPI/bot/trnscheduleEnq/12951"
        );

        assert_eq!(
            format!("{}/askdisha-bucket/{}.json", client.cdn_base, "hi"),
            "https://cdn.corover.ai/askdisha-bucket/hi.json"
        );
        assert_eq!(
            format!("{}/askdisha-bucket/getSettings.json", client.cdn_base),
            "https://cdn.corover.ai/askdisha-bucket/getSettings.json"
        );
    }
}
