# AskDISHA Module — Interface Contract & Plan (v2, 2026-08-22)

Single source of truth for the **v2 embedded integration** of the CoRover/AskDISHA
backends. Supersedes v1 (standalone-tab design — retired). All agents implement
against THIS document. Upstream research: `docs/ASKDISHA_DATA_CALLS.md`.

## 0. Pivot rationale

v1 shipped a standalone Ask DISHA tab. Product decision (user): **no separate
section**. The corover origin is instead embedded as a *data source + unique
feature provider* inside existing sections; UI adapts only genuinely unique
capabilities.

## 1. Probe verdicts (binding evidence, all live-verified 2026-08-22)

| Upstream | Verdict | Consequence |
|---|---|---|
| `GET /bot/searchStation/{q}` | 200 headerless | F2 enrichment source |
| `GET /bot/stationsByLocation/{lat}/{lng}` | 200 headerless, rows carry `distance` km | F3 nearby feature |
| `GET /bot/trnscheduleEnq/{train}?journeyDate=&startingStationCode=` | 200 headerless | F4 schedule **primary** source (works from non-India IPs; NTES -> Railyatri follow as fallbacks) |
| `GET /bot/pin/{pincode}` | 200 → `{state,stateList,cityList,serverId,timeStamp}` | F6 hidden utility |
| CDN `stationupdated.json` | open, 8,491 rows w/ hi·gu·geo·district·address·trainCount | F1 offline hydration |
| CDN `{en\|hi\|gu}.json`, `getSettings.json` | open | kept-but-hidden endpoints (D4) |
| Availability (`avlFarenquiry`,`trnenquiry`,`getAvailability`) | **404 Fastify route-not-found** — endpoints do not exist | availability OUT of scope |
| `POST /bot/sendQuery/{lang}` (chat/NLU incl. availability calendar) | 401 guest-gated even with dSession+headers+cookies | chat OUT (v1 verdict stands) |
| `GET /addservices/eticket/{pnr}` | **500 `undefined (reading 'bookingData')` even with REAL PNR ± Ao() headers** — reads caller's logged-in booking list; session-bound, not a public lookup | PNR fallback OUT (was O1) |
| `popular.json`, `countries.json` | open but low/no value in app | skipped by decision |

## 2. Locked product decisions

- **D1** No separate section: standalone tab deleted (page/nav/route).
- **D2** Hindi/Gujarati station names render as muted subtitle under English.
- **D3** `ASKDISHA_ENABLED` defaults to **true**; `ASKDISHA_ENABLED=0` hard-disables
  every outbound corover call (hydrated dataset still ships in-repo and works).
- **D4** `/faqs`, `/settings`, `/pin` endpoints stay API-only — zero UI links.

## 3. Config & state (delta from v1)

```rust
Config.askdisha_enabled   // env ASKDISHA_ENABLED — DEFAULT NOW true ("0"/"false"/"no"/"off" ⇒ false)
Config.corover_base       // unchanged: https://api.disha.corover.ai
Config.corover_cdn_base   // unchanged: https://cdn.corover.ai (bucket path appended)
AppState.askdisha         // Option<Arc<CoroverClient>> — Some iff enabled (unchanged)
```

## 4. Backend surface (final)

Existing slices consume the client directly; the askdisha slice keeps only the
hidden/utility routes. v1 public `/stations` + `/schedule` routes are REMOVED
(their data now flows through the stations/schedule slices).

### 4.1 Kept from v1 (unchanged behavior)
| Route | Cache key / TTL | Notes |
|---|---|---|
| `GET /rail-api/askdisha/status` | none | `{"enabled":bool,"sources":["corover-api","corover-cdn"]}` |
| `GET /rail-api/askdisha/faqs?lang=en\|hi\|gu` | `askdisha:faqs:{lang}` / 24 h | bad lang → 400 `{"error":"invalid language"}`; absent lang = en |
| `GET /rail-api/askdisha/settings` | `askdisha:settings` / 1 h | flag object passthrough |

### 4.2 New routes (this wave)
| Route | Upstream | Validation | Cache key / TTL | Success body |
|---|---|---|---|---|
| `GET /rail-api/askdisha/nearby?lat=&lng=` | `/bot/stationsByLocation/{lat}/{lng}` | lat∈[-90,90], lng∈[-180,180] else 400 `{"error":"invalid coordinates"}`; missing → 400 `{"error":"missing coordinates"}` | `askdisha:nearby:{lat:.3},{lng:.3}` / 30 min | `{"source":"corover-api","cached":bool,"count":n,"stations":[NearbyRow]}` (cap 50) |
| `GET /rail-api/askdisha/pin/{pincode}` | `/bot/pin/{pincode}` | `^[1-9][0-9]{5}$` else 400 `{"error":"invalid pincode"}` | `askdisha:pin:{pincode}` / 7 d | `{"source":"corover-api","cached":bool,"state":String,"cityList":[String]}` |

Disabled behavior for both: router not merged ⇒ 404 fall-through (unchanged v1 semantics).

### 4.3 Embedded integrations (no new routes)
| # | Where | Change |
|---|---|---|
| F1 | offline | `src/bin/hydrate_stations.rs`: merge `testdata/askdisha/stationupdated_full.json` into `data/stations.json` by code. Adds optional `name_hi,name_gu,district,address,train_count,lat,lng`. **Local `state`/`zone` always win on conflict.** Prints unmatched-code report; idempotent. |
| F2 | search + stations slices | Search response rows and `GET /stations/:code` gain the optional fields (passthrough from hydrated `StationRecord`). |
| F4 | schedule slice | Chain **NTES → Railyatri → Corover**: same normalized `ScheduleResponse`; winning source honest in `data_source` ("CoRover"); `record_source_latency("corover-api")`; cache key `schedule:{train}` reused. Stops may now carry `distance_km:f64` + `day:u8` (absent when NTES/Railyatri win). |
| — | observability | `record_source_latency` on success per house mechanism; failures = warn-log + honest error (no fabricated metrics). |

## 5. Rust types (exact names — fixtures/tests compile against these)

```rust
// src/core/corover.rs — client additions (agent B owns this file)
pub struct NearbyStation {          // upstream row from stationsByLocation
    pub name: String,
    pub code: String,
    #[serde(default)] pub utterances: Vec<String>,
    #[serde(default)] pub name_hi: Option<String>,
    #[serde(default)] pub name_gu: Option<String>,
    #[serde(default)] pub district: Option<String>,
    #[serde(default)] pub state: Option<String>,
    #[serde(default)] pub distance: Option<f64>,     // km, upstream "distance"
}
pub struct PinLookup {              // upstream row from bot/pin
    pub state: String,
    #[serde(default)] pub city_list: Vec<String>,    // upstream "cityList"
}
impl CoroverClient {
    pub async fn stations_by_location(&self, lat: f64, lng: f64) -> Result<Vec<NearbyStation>, AppError>;
    pub async fn pin_lookup(&self, pin: &str) -> Result<PinLookup, AppError>;
}

// src/data/mod.rs — StationRecord gains (all #[serde(default)], skip_serializing_if None on Serialize)
pub name_hi: Option<String>, pub name_gu: Option<String>,
pub district: Option<String>, pub address: Option<String>,
pub train_count: Option<String>,
pub lat: Option<f64>, pub lng: Option<f64>,

// models.rs — public Station mirrors the same Optionals; ScheduleStop gains:
pub distance_km: Option<f64>, pub day: Option<u8>,

// slices/askdisha — response DTOs
pub struct NearbyRow { pub code,name,name_hi?,name_gu?,distance_km,state?,district? } // distance rounded 1 decimal
```

## 6. Frontend (Svelte SPA `frontend/`)

| File | Change |
|---|---|
| `src/lib/pages/AskDisha.svelte` | **DELETE**; remove nav entry + route (App.svelte / Layout.svelte) |
| `src/lib/components/StationSearch.svelte` | muted second line `हिंदी · District` under English name when fields present |
| `src/lib/pages/Station.svelte` | header subtitle (hi/gu) + district/state/address meta row; NEW compact “Nearby” button → `navigator.geolocation` → calls `/rail-api/askdisha/nearby` → distance-sorted list; tap row loads that board; permission-denied inline copy; control hidden entirely on fetch failure ≠ permission |
| `src/lib/pages/Train.svelte` | Distance/Day columns already exist rendering `-` — populate from `distance_km`/`day` when present (no structural change) |

## 7. Fixtures & tests

- New captures (Wave 0b): `testdata/askdisha/stationupdated_full.json`,
  `nearby_mumbai.json`, `pin_400001.json`.
- Extend `tests/askdisha_fixtures.rs`: nearby parse (distance floats, cap),
  pin parse, hydration merge unit tests (collision policy, unmatched report).
- Config tests updated for default-true flip.
- Schedule chain: follow existing hermetic test style (no live network).

## 8. Wave plan & exclusive file ownership

| Wave | Owner | Exclusive files |
|---|---|---|
| 0 (orchestrator) | me | docs (this file), fixtures download/capture, `config.rs` flip+tests, delete AskDisha.svelte/nav/route |
| 1-A dataset+search | agent | `src/bin/hydrate_stations.rs`, `src/data/mod.rs`, station structs in `models.rs`, `src/slices/search/`, `src/slices/stations/`, `frontend/src/lib/components/StationSearch.svelte`, regenerated `data/stations.json` |
| 1-B core+schedule | agent | `src/core/corover.rs` (sole owner: adds §5 types+methods), `src/slices/schedule/`, `frontend/src/lib/pages/Train.svelte`, schedule fixtures/tests |
| 1-C nearby+station | agent | `src/slices/askdisha/` (rework: drop v1 /stations+/schedule routes; add §4.2), `frontend/src/lib/pages/Station.svelte`, nearby/pin fixtures+tests |
| 2 (orchestrator) | me | gates fmt/clippy/test/vite-build · release rebuild · live verify checklist · observability check · summary |

Agents code against §5 signatures verbatim so parallel merges compile.

## 9. Acceptance criteria

1. Gates green: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `npm run build` — zero warnings.
2. Live default-env: autocomplete shows हिंदी subtitle; `/askdisha/nearby?lat=19.07&lng=72.87` returns km distances; 12951 schedule shows Day/Distance populated whichever source won; `data_source` truthful everywhere; `/askdisha/pin/400001` works.
3. `ASKDISHA_ENABLED=0`: zero outbound calls (log-verifiable), app fully functional off hydrated dataset, standalone tab absent, utility routes 404.
4. Observability lists corover sources with real latencies only.

## 10. Risks

| Risk | Mitigation |
|---|---|
| Upstream shape drift | fixture-pinned parsers, honest 502s |
| Hydration degrades zone/state data | local-authority collision policy + unmatched report |
| Geolocation UX friction | inline denial copy; never blocks board flow |
| Parallel-agent drift | this contract pins signatures/routes/cache keys |
