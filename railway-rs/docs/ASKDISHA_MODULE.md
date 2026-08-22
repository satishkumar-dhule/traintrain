# AskDISHA Module — Interface Contract (v1, 2026-08-22)

Single source of truth for the pluggable `askdisha` module. All agents implement
against THIS document. Upstream research: `docs/ASKDISHA_DATA_CALLS.md`.

## Wave-0 probe verdicts (binding)

| Upstream call | Verdict | Consequence |
|---|---|---|
| `POST /dishaAPI/bot/sendQuery/{lang}` | **401** even with valid dSession + full browser headers + cookie bootstrap | Chat proxy DROPPED. No `/query` endpoint. No RSA/dSession code. |
| `GET /dishaAPI/bot/trnscheduleEnq/{train}?journeyDate=&startingStationCode=` | **200 headerless**, rich JSON | ✅ schedule fallback endpoint |
| `GET /dishaAPI/bot/searchStation/{q}` | **200 headerless** | ✅ station search endpoint |
| CDN `askdisha-bucket/*.json` | open | ✅ faqs/settings endpoints |

## Feature gate

- `Config.askdisha_enabled: bool` — env `ASKDISHA_ENABLED`, default `false`.
- `Config.corover_base: String` — env `COROVER_BASE`, default
  `https://api.disha.corover.ai`. CDN base derived: `https://cdn.corover.ai/askdisha-bucket`.
- When disabled: router NOT merged in `web.rs`; zero network footprint;
  frontend page shows disabled empty-state (404 on API).

## AppState

- `AppState.askdisha: Option<Arc<CoroverClient>>` — `Some` iff enabled.

## Backend endpoints (`/rail-api/askdisha/*`, all JSON)

| Route | Upstream | Cache key / TTL | Success body | Errors |
|---|---|---|---|---|
| `GET /status` | none | none | `{"enabled":true,"sources":["corover-api","corover-cdn"]}` | — |
| `GET /stations?q=<q>` | `/dishaAPI/bot/searchStation/{q}` | `askdisha:stations:{q.to_lowercase()}` / 6 h | `{"source":"corover-api","cached":bool,"count":n,"stations":[StationRow]}` limit 20 | upstream err → 502 `{"error":...}`; disabled → 404 route absent |
| `GET /schedule/{train_no}?date=YYYY-MM-DD&from=<code>` | `/dishaAPI/bot/trnscheduleEnq/{train}?journeyDate=&startingStationCode=` | `askdisha:schedule:{train}:{date}:{from}` / 30 min | `{"source":"corover-api","cached":bool,"schedule":ScheduleResponse}` | invalid train (non 1-5 digits) → 400; upstream err → 502 |
| `GET /faqs?lang=en|hi|gu` | CDN `{lang}.json` | `askdisha:faqs:{lang}` / 24 h | `{"source":"corover-cdn","cached":bool,"faqs":[string]}` | bad lang → 400 |
| `GET /settings` | CDN `getSettings.json` | `askdisha:settings` / 1 h | `{"source":"corover-cdn","cached":bool,"settings":{"id":1,"isDisabled":false,"booking":true}}` | — |

`date` param: pass through as given; if absent omit query param. `from` optional.
Query strings to upstream are URL-encoded path segments (station q).

## Rust types (exact names — fixtures test compiles against these)

```rust
// src/core/corover.rs
pub struct CoroverClient { /* reqwest::Client, corover_base: String, cdn_base: String */ }
impl CoroverClient {
    pub fn new(corover_base: impl Into<String>, cdn_base: impl Into<String>, timeout: Duration) -> Self;
    pub async fn search_station(&self, q: &str) -> Result<Vec<StationRow>, AppError>;
    pub async fn trnschedule_enq(&self, train_no: &str, journey_date: Option<&str>, from_code: Option<&str>) -> Result<ScheduleResponse, AppError>;
    pub async fn fetch_faqs(&self, lang: &str) -> Result<Vec<String>, AppError>;
    pub async fn fetch_settings(&self) -> Result<SettingsFlag, AppError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationRow {
    pub name: String,
    pub code: String,
    #[serde(default)] pub utterances: Vec<String>,
    #[serde(default)] pub name_hi: Option<String>,
    #[serde(default)] pub name_gu: Option<String>,
    #[serde(default)] pub district: Option<String>,
    #[serde(default)] pub state: Option<String>,
    #[serde(default)] pub train_count: Option<String>,
    #[serde(default)] pub latitude: Option<f64>,
    #[serde(default)] pub longitude: Option<f64>,
    #[serde(default)] pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleStop {
    pub station_code: String,
    pub station_name: String,
    #[serde(default)] pub arrival_time: Option<String>,
    #[serde(default)] pub departure_time: Option<String>,
    #[serde(default)] pub route_number: Option<String>,
    #[serde(default)] pub halt_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleResponse {
    pub train_number: String,
    #[serde(default)] pub train_name: Option<String>,
    #[serde(default)] pub station_from: Option<String>,
    #[serde(default)] pub station_to: Option<String>,
    #[serde(rename = "trainRunsOnMon", default)] pub runs_mon: bool, // "Y"/"N" strings -> custom deser or post-process
    // ... tue..sun same pattern
    #[serde(default)] pub error_message: Option<String>,
    #[serde(default)] pub station_list: Vec<ScheduleStop>,
}
// NOTE: upstream sends "Y"/"N" strings for runs_on flags. Implement via
// #[serde(deserialize_with)] helper `de_yn_bool` and serialize back as bool.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFlag { pub id: i64, pub is_disabled: bool, pub booking: bool }
```

Unknown upstream fields MUST be ignored (no deny_unknown_fields).

## HTTP conventions

- Reuse house client patterns from `src/core/http.rs` / existing core modules:
  browser-like UA from `config.user_agent`, timeout from `config.http_timeout`,
  retry once on transient (5xx/timeout), map errors to `AppError` variants used by
  other slices. No new crates required.
- Source tagging: mirror how ntes slice reports sources via `SourceOutcome`
  (`src/core/source.rs`) using source ids **`corover-api`** and **`corover-cdn`**
  so the observability tab lists them truthfully.

## Fixtures (`testdata/askdisha/`)

`schedule_12951.json` (real capture), `stations_new.json` (real capture),
`faqs_en.json` (real capture, truncated ok ≥50 entries),
`getSettings.json` (43 B real), `unauthorized.json` (401 body sample).
Integration test `tests/askdisha_fixtures.rs` parses all through the exact structs above.

## Frontend contract (`AskDisha.svelte`)

- Reads only the four GET endpoints above + graceful 404/502 handling.
- Three panels/tabs: **Stations** (typeahead table: code/name/hi/state),
  **Schedule** (train no + date + from → stops table, runs-on badges),
  **FAQs** (lang selector en/hi/gu, client-side filter box).
- Disabled state: any 404 ⇒ show "Module disabled — set ASKDISHA_ENABLED=1".
- Follow existing page conventions (`frontend/src/lib/pages/*.svelte`,
  ui components under `frontend/src/lib/components/ui/`).
