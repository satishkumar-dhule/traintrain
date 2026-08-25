# DRY — Reusable Controls Plan (Super-N² Fan-Out)

> **Date:** 2026-08-25 · **Method:** 5 parallel research agents (N² = 25 sub-investigations) covering slices-backend, frontend-components, frontend-pages, observability/system, core-infra/ntes, models/web/state. All citations are `file:line` · **Read-only** — no code edits in research phase. Total duplicated LOC: **~1.4–1.6 kLOC** across Rust + Svelte (22–25 % of app code). Plan groups duplicates into **reusable libs/controls** with explicit ownership, effort/risk, and sequencing.

---

## 0. Topology — What Is Already DRY (keep)

| Good | Location | Why reference shape |
|---|---|---|
| `station_codes.rs` | `src/slices/station_codes.rs:11-55` | Single station validation (`normalize_code`, `require_station`) reused by 6 slices — **exemplar** for new `validate` lib |
| `Cache` + `HttpClient` + `AppError` | `src/core/cache.rs:15-107`, `http.rs:14-159`, `error.rs:39-84` | Single ownership, all slices inject via `AppState` |
| `Railyatri` normalize | `src/core/railyatri/mod.rs:11-51` | Extracted `extract_next_data`, `minutes_to_hhmm` — slices call, not copy |
| `Recent` | `frontend/src/lib/recent.js:3` | `loadRecent/rememberRecent/clearStored` — correct single source (but 1 page bypasses) |
| `StatusBadge` + `DataTable` + `AsyncState` | `frontend/src/lib/components/badges/status-badge.svelte:5`, `DataTable.svelte:1-440`, `AsyncState.svelte:1-47` | Real reusable cores — debt is **incomplete adoption**, not missing abstraction |
| `utils.js` hrefs | `frontend/src/lib/utils.js:21-41` | `trainHref/stationHref/journeysHref` single source (but 4 pages inline `encodeURIComponent`) |

---

## 1. The 20 Duplication Families (evidence-backed)

### Backend (Rust) — 12 families · ~850 LOC

| ID | Family | Dupe LOC | Files × | Evidence (representative) |
|---|---|---|---|---|
| **B1** | `today_ist()` (IST UTC+5:30) | 35 | 5× | `availability/mod.rs:111-117`, `chart/mod.rs:85-91`, `trains_between/service.rs:131-140`, `train_on_map/service.rs:123-129`, `ai_chat/tools.rs:779-785` — byte-identical 7 LOC |
| **B2** | `is_valid_date` / date normalization | 100 | 7× | `availability/mod.rs:105-109` ≡ `chart/mod.rs:79-83` (4 formats), `live_status/service.rs:217-227` `normalize_date`, `pnr/service.rs:455-474`, `irctc/normalize.rs:26-47` `date_compact` ≡ `paytm/normalize.rs:15-23` |
| **B3** | Train/PNR validation (1-8 vs 5-digit vs 10-digit) | 43 | 9× | `live_status/mod.rs:73-75` `len<=8`, `schedule/mod.rs:42-44` `len>8`, `average_delay/mod.rs:42` `len==5`, `pnr/mod.rs:43-44` `len==10`, `exceptional/mod.rs:39-43` `4..=5` — same concept, 8 spellings |
| **B4** | Cache key + `get`→`from_value`→`set` boilerplate | 100 | 14 slices | `live_status/service.rs:31-36/59`, `schedule/service.rs:40-44/62`, `availability/service.rs:33-36/47`, `search/service.rs:43-47/139-143` … same 6-LOC hit-check ×14 |
| **B5** | `str_field` / `int_field` / `non_empty` / `day_bool` Value helpers | 90 | 8× | `trains_between/service.rs:176-182` ≡ `heritage/service.rs:81-87` ≡ `parcel/service.rs:76-82` ≡ `average_delay/service.rs:75-81` (7 LOC `str_field` ×8); `irctc/normalize.rs:192-227` same; `day_bool` identical `trains_between:185-194` ≡ `availability:214-223` |
| **B6** | Source latency `Instant::now()` + `record_source_latency` + `tracing::info!` | 60 | 16× | `schedule/service.rs:77-82`, `live_status/service.rs:42-47/194`, `availability/service.rs:112-113`, `askdisha/service.rs:244/273/306/333`, `ai_chat/mod.rs:119/147` |
| **B7** | Cookie-jar `merge_cookie` / `cookie_str` / `merge_cookies` / `capture_cookies` | 90 | 3× byte-identical | `ntes/web.rs:2040-2057` ≡ `irctc/client.rs:195-212` ≡ `pnr/service.rs:500-524`; `merge_cookies` `web.rs:904-917` ≡ `irctc:178-192` |
| **B8** | `BROWSER_UA` + header assembly (`USER_AGENT`/`REFERER`/`ORIGIN`/`X-Requested-With`/`Greq`) | 45 | 3× | `ntes/web.rs:35` ≡ `corover.rs:28` (same literal); header blocks `web.rs:796-801` ≡ `843-848` ≡ `irctc/client.rs:150-153` ≡ `pnr/service.rs:482-485` |
| **B9** | Retry policy (400 ms + transient vs 4xx) | 85 | 3 impls | `http.rs:57-82` (naive retry), `corover.rs:165-211` (4xx bail `190-193`), `ntes/web.rs:715-761` (3-attempt staged `recover`) — same `RETRY_DELAY=400ms` but divergent semantics |
| **B10** | Runs-on / day-of-week parsing | 50 | 5× | `trains_between/service.rs:185-194`, `irctc/normalize.rs:142-176`, `paytm/normalize.rs:114-129`, `railyatri/mod.rs:55-68`, `ntes/web.rs:1028-1043` — 5 spellings of `[bool;7]` Mon..Sun |
| **B11** | `CODE - NAME` station label + linear `station_name` scan | 35 | 8× | `trains_between/service.rs:33-34` `station_name(src).unwrap_or(src)`, `live_station/service.rs:33-44`, `ntes/web.rs:359-365` `format!("{code} - {name}")` ×5; `data/mod.rs:336-341` O(n) scan over 8958 stations |
| **B12** | NTES form-method skeleton (`post_form`→`parse_*`→`source_unavailable`) | 220 | 12 methods in `web.rs` | `web.rs:341-376` `live_station` ≡ `383-412` `trains_between` ≡ `421-700` 10 more — 18 LOC ×12 = 216 LOC + parsers’ `split("<tr")`→`marker`→`if empty None` 56 LOC |

### Frontend (Svelte) — 5 families · ~600 LOC

| ID | Family | Dupe LOC | Evidence |
|---|---|---|---|
| **F1** | `todayISO()` / `DATE_RE` / `isoShift` / `diffDays` | 45 | `format.js:4,70` ≡ `utils.js:14,19` ≡ `DateStrip.svelte:18,23,28,34` + `static/routes.js:52,161` + `static/ui.js:190,557` — 3 definitions in Svelte app + 2 in legacy |
| **F2** | Fetch state machine `phase/error/data + loadX + liveKey/ttKey + untrack + if(key!==k) return` | **400** | 14 sites: `Train.svelte:138-225` ×5, `Station.svelte:157-213` ×2, `Availability.svelte:251-270`, `Pnr.svelte:102-132`, `Exceptions.svelte:32-46`, `Extras.svelte:113-143`, `Home.svelte:234-251`, `JourneysTable.svelte:118-153`, `System.svelte:30-52`, `About.svelte:36-40` — same 8-line `if(res.ok)…else` |
| **F3** | Delay/time parsing (4 regex variants) | 40 | `delay-badge.svelte:4` `"5m"`, `train-kind.js:49` `"HH:MM"`, `Train.svelte:500,512` copy of `train-kind:49`, `Availability.svelte:107` `hmMin`, `DataTable.svelte:67` time regex — 4 parsers for `"On Time"/"HH:MM"/"+12"/"5m"` |
| **F4** | Autocomplete / suggest fetch (debounce 180-200 ms + abort + portal) | 150 | `AutoCompleteInput.svelte:51-93` 75 LOC ≡ `PowerSearch.svelte:32-51` 35 LOC ≡ `Home.svelte:224-251` 40 LOC ≡ `static/ui.js:1048-1134` 85 LOC legacy |
| **F5** | Badge wrappers (`train-number` ≡ `station-code` + 5 `*KindBadge` identical templates) | 90 | `train-number-badge.svelte:1-38` ≡ `station-code-badge.svelte:1-38` (2 tokens differ); `exception-kind/log-level/halt/pnr/availability-status-badge` 5× 20-LOC same shape; `AvailabilityChip.svelte:21-29` `TONE_MAP` duplicates `STATUS_TONES` |

### Models / Web / Observability — 3 families · ~180 LOC

| ID | Family | Dupe LOC | Evidence |
|---|---|---|---|
| **M1** | Response envelope `data_source/notice/cache_ttl/freshness` + `skip_serializing_if` | 70 | 13 types `models.rs:55,94,132,182,205,232,261,283,307,333,384,445,507,545` — same 2-LOC tail ×13 + 30 attribute lines |
| **M2** | Hydrated station optionals (`name_hi/name_gu/district/address/train_count/lat/lng`) | 22 | `models.rs:579-592` ≡ `search/mod.rs:48-65` `StationRow` (6 fields) + `search/mod.rs:107-120` `SuggestHit` (3 fields) — 3 wire shapes kept in sync by test `search/mod.rs:344-416` |
| **M3** | Router merge chain + middleware layers + `proc_stats` double-sampling + `SourceStatus` dual shape | 80 | `web.rs:40-58` 18 hand-maintained `.merge(slices::X::router())` (add slice → edit 2 files or 404); `web.rs:66-75` `CatchPanicLayer+TraceLayer` ×2; `main.rs:79-84` ≡ `system.rs:105-109` ≡ `observability/service.rs:17` triple `proc_stats` (global `CPU_SAMPLE` Mutex skew); `System.svelte` vs `SourceStatus.svelte` vs `SourceTrustChip.svelte` 3 renderings of same `NTES/Railyatri/IRCTC/Paytm` set |

---

## 2. Reusable Libs / Controls — Catalog (20 controls)

> Ownership rule: slices never own other slices. Extracted code goes to `src/core/*` or `frontend/src/lib/*` or `src/models/envelope.rs`. Every control keeps wire shape byte-identical unless explicitly noted.

### P0 — Must-have (highest leverage, low risk) — ship first

| # | Control | Replaces | API sketch | Owner | LOC saved | Effort | Risk |
|---|---|---|---|---|---|---|---|
| **C1** | `src/core/validate.rs` | B3 (train/pnr) + B2 `is_valid_date` + station `require_station` delegation | `pub fn train_id(s:Option<&str>)->Result<String,AppError>` (1-8), `train_id_5` (len 5, ≠00000), `pnr(s)->Result`, `is_valid_date(s)->bool`, `clamp_query(s,128)->String` | `core` | 80 | S (1-2 h) | Low — pure fns, no async |
| **C2** | `src/core/time.rs` | B1 `today_ist` (5×) + B2 `normalize_date/date_compact/date_iso/ist_offset` | `pub fn today_ist_iso()->String`, `today_ist_ntes()->String` (`DD-Mon-YYYY`), `parse_date(s)->Option<NaiveDate>` (4 fmts + NTES/pnr variants), `date_iso(s)->String`, `date_compact(s)->String`, `const DATE_FORMATS:&[&str]` | `core` | 120 | S (2 h) | Low — existing fixtures pin golden strings |
| **C3** | `src/core/json.rs` (`ValueExt`) | B5 `str_field/int_field/day_bool` (8×) | `pub trait ValueExt { fn str_field(&self,&str)->String; fn str_one_of(&[&str])->String; fn opt_str(&str)->Option<String>; fn i64_one_of(&[&str])->Option<i64>; } impl ValueExt for Value` — keep `str_field` alias for minimal churn | `core` | 90 | S (1 h) | Very Low — pure |
| **C4** | `src/core/cache/ext.rs` + `keys.rs` | B4 cache hit/set (14 slices) | `trait CacheExt { fn get_json<T:DeserializeOwned>(&str)->Option<T>; fn set_json<T:Serialize>(&str,&T); }` + `keys::live_status(train,date)->String`, `schedule(train)`, … (type-safe keys, single `:` convention). Helper `cached_or_fetch(&cache,&key,ttl,|| async { fetch })` that logs hit/miss, handles `from_value` error uniformly | `core/cache` | 100 | M (half day) | Medium — preserves `EXCEPTIONAL_CACHE_TTL` (2 h), `SEARCH_TTL` (30 m), `live_station` raw-`Value` cache nuance (old entries re-fetched on `from_value` error, safe but latency spike) |
| **C5** | `frontend/src/lib/dates.js` | F1 `DATE_RE/todayISO/isoShift/diffDays` | `export const DATE_RE=/^\d{4}-\d{2}-\d{2}$/; export const todayISO=()=>…, isoShift(days), diffDays(a,b), clampDate(iso,min,max), parseIso` — re-export from `format.js`+`utils.js` for back-compat; delete inline `DateStrip:23,28,34` | `lib` | 45 | S | Low |
| **C6** | `frontend/src/lib/delay.js` | F3 delay parsing (4 variants) | `export function parseDelay(v:string):number\|null` handles `"On Time"`, `""`, `"HH:MM"`, `"+12"`, `"5m"` — replaces `delay-badge:4`, `train-kind:49`, `Train.svelte:500,512`, `Station:128`, `Availability:107`, `DataTable:67` | `lib` | 40 | S | Low |
| **C7** | `frontend/src/lib/async.svelte.js` (`createResource`) | **F2** fetch machine (14 sites, 400 LOC) | `createResource(keyFn, fetcher, {rememberKey, cachePaint}) -> {phase, data, error, reload, refresh, abort}` — handles `loading vs refreshing`, key dedup, `AbortController` (like `AutoCompleteInput:54`), `api()` stale repaint, `rememberRecent` hook | `lib` | **400** | M (half day) | Low* — preserve `if(key!==k) return` race guard + `phase==='refreshing'` keep-old-data semantics (`Train:140`, `Station:164`) |
| **C8** | `src/core/cookies.rs` (`CookieJar`) | B7 cookie-jar (93 LOC) | `pub struct CookieJar(Arc<Mutex<Vec<(String,String)>>>); impl CookieJar { fn ingest(&Response); fn header_value()->Option<String>; fn merge(name,value); }` — replaces `Vec` in `NtesWebClient:302`, `IrctcClient:36`, `pnr/service.rs:34` `CaptchaSession` | `core/http` | 90 | S (1-2 h) | Low — covered by `irctc/client.rs:219`, `web.rs:2080` |

\* C7 risk is low if `AbortController` + `key !== k` serialized exactly as `Station:169` pattern. Wire via `api.js:12` `api(path)` which already has `TIMEOUT_MS=12000` + `try JSON.parse(text)`.

### P1 — Should-have (next sprint)

| # | Control | Replaces | Owner | LOC saved | Effort | Risk |
|---|---|---|---|---|---|---|
| **C9** | `src/core/source.rs` (`Source` enum + `LABELS`) | B6 label strings (36 magic `data_source` + latency labels) | `core/source` | 20 | XS | None — fixes `NTES` vs `ntes` lowercase bug (`journey_basis/service.rs:34`) that creates duplicate Prometheus series |
| **C10** | `src/core/http/headers.rs` + retry constants | B8 header assembly + B9 retry (400 ms, `is_transient`, `is_server_error`) | `core/http` | 50 | M | Medium — NTES staged `recover` stays bespoke (`web.rs:755-761`), only extract `RETRY_DELAY` + `is_transient` + `is_server_error` constants |
| **C11** | `src/core/geo.rs` + `station_label()` | B11 `CODE - NAME` + `station_name`HashMap index | `core/data` | 35 | S | Low — build `HashMap<String,String>` alongside `coords` in `Datasets::new` (currently only `coords` hashed, `station_name` is O(n) scan) |
| **C12** | `src/core/runs.rs` (`RunsOn`) | B10 runs-on (5 spellings of `[bool;7]`) | `core` | 60 | S | Low — pure, test-covered per module |
| **C13** | `frontend/src/lib/validation.js` | Frontend train/station/PNR/date regex (5×) | `lib` | 20 | XS | Low |
| **C14** | `frontend/src/components/EntityBadge.svelte` | F5 `train-number-badge` ≡ `station-code-badge` (2 tokens differ) | `components` | 25 | S | Low — `type='train'|'station'` prop, keep aliases for back-compat |
| **C15** | `frontend/src/lib/metrics.js` (+ `format.js` ext) | Observability helpers `num/memMb/pctFromFrac/latest/seriesRange/sparkPoints/hitRate` (System `66-109` vs About `46,74-93` 60 LOC overlap) | `lib` | 60 | S | Low |
| **C16** | `frontend/src/lib/stores/sourceStatus.svelte.js` | `SourceStatus.svelte:11-34` + `SourceTrustChip.svelte:11-30` dual fetch of `/rail-api/source-status` | `lib/stores` | 40 | S | Low — single-flight `api()` + `TIMEOUT_MS` |
| **C17** | `frontend/src/components/MetricTile.svelte` + `MetricBar.svelte` + `Sparkline.svelte` + `Gauge.svelte` | System `COMPACT_HERO_* 388-531` ↔ About `ABOUT_CARD_* 128-286` ↔ Train `avgDelayBar 585-593` ↔ Availability `chanceBar 306-322` (4 identical `h-1.5 bg-muted → bg-primary` bars) | `components` | 120 | S | Low — keeps `Card` chrome + `Signal & Steel` tokens (`app.css:40-51`) untouched |

### P2 — Consider (ship constants-only first, defer traits/macros)

| # | Control | Replaces | Notes | Effort | Risk |
|---|---|---|---|---|---|
| **C18** | `src/models/envelope.rs` (`Meta {data_source,notice,cache_ttl}`) or macro `impl_envelope!()` | M1 envelope (70 LOC) — 13 types × `skip_serializing_if` tail | Do **constants only** first (`Source::label()` via C9). Defer `#[serde(flatten)] Meta` until wire-contract test frozen — `tests/*.rs` assert exact JSON shape (`tests/schedule.rs:14`) | M | Medium — flatten changes key order, must verify frontend key-order-agnostic |
| **C19** | `src/models/hydration.rs` (`StationHydration {name_hi…lat/lng}`) | M2 hydrated optionals (22 LOC) — `Station` ≡ `StationRow` ≡ `SuggestHit` | Compose via `#[serde(flatten)] hydration: StationHydration` | S | Low |
| **C20** | `src/slices/registry.rs` (`register_slices!` macro) | M3 router checklist (18 merges, silent 404 if new slice added to `slices/mod.rs:10-28` but forgotten in `web.rs:40-59`) | Macro expands to both `pub mod` list and `web.rs` merge chain; also `build.rs` check that every `mod.rs` with `pub fn router` is registered | S | Low — already drift seen: `station_codes` has `mod.rs` but no `router` |
| **C21** | `src/core/ntes/forms.rs` (`FormSpec {endpoint,opt,subOpt,marker,fields,parser}`) | B12 NTES 12 form methods (220 LOC) | Collapse 12×18-LOC skeleton to 1×40 + 12×5 specs; keep per-endpoint `rows_marker` (some endpoints like `TrainRunning/FindRunningInstancePop` rely on it). **Do last** — touches all 12 NTES endpoints + 2-stage `recover`; needs live fixture regression (`testdata/ry_*.html`, `ntes::web` 20 tests) | M (1 day) | **High** |
| **C22** | `frontend/src/lib/search.svelte.js` (`createSuggestStore`) | F4 autocomplete core (80 LOC saved, UI stays separate) | Extract fetch+debounce+abort only; keep portal positioning `AutoCompleteInput:144` and `Command` UI separate — otherwise medium risk | M | Medium |
| **C23** | Sunset `static/*` (`api.js`+`ui.js` Autocomplete+`routes.js` hash router) | Legacy duplication (300 LOC) — two full routing tables, two `DATE_RE/today()` copies, two API clients | Document `static/` as frozen legacy; do not mechanically unify Svelte `history.pushState` router (`router.svelte.js:1` + `App.svelte:15`) vs hash SPA (`static/routes.js:25` 11 regexes) — sunset after Svelte migration verified | M | Medium |

---

## 3. Sequencing — Recommended Order (lowest risk first)

### Wave 1 — Pure helpers, zero wire impact (1 day, parallelizable)

1. **C8** `CookieJar` → C1 `validate` → C3 `json::ValueExt` — all leaf, covered by existing unit tests. Bundle `C9` source constants + C10 `RETRY_DELAY` const at same time.
2. **C2** `time.rs` + **C5** `dates.js` + **C6** `delay.js` + **C13** `validation.js` — union date/delay/validation parsers, preserve passthrough `unwrap_or(s.to_string())` so unparseable input still fails honestly upstream. Re-export wrappers in `irctc/normalize.rs`, `paytm/normalize.rs` to keep import paths.
3. **C11** `station_label` + HashMap index for `station_name` (perf win).

### Wave 2 — Fetch/cache consolidation (1–2 days)

4. **C7** `createResource` — replace 14 phase/error/stale blocks; standardize all pages on `AsyncState` (currently 4 pages bypass: Availability, System, About, Train map/exceptions tabs). Fixes stale bug `Exceptions.svelte:38` (compares input not key) and `Train:72` plain `let` vs `$state`.
5. **C4** `cache/ext` + `keys` — collapse 14 `cache.get`/`set` into `cached_or_fetch(state,key,ttl,||fetch)` preserving `set_with_ttl` distinction (`exceptional` 2 h, `search` 30 m). Keep `live_status` raw-`Value` cache until version bump (old DTO entries still re-fetched via `from_value` error path, safe but latency spike).
6. **C15** `metrics.js` + **C16** `sourceStatus` store + **C10** `timed(state,label,fut)` wrapper for latency (collapses B6 `Instant::now` ×16).

### Wave 3 — UI consolidation (1 day, parallelizable with Wave 2)

7. **C14** `EntityBadge` (type prop) + `KindBadge` generic — unify 12 badge files to 2; fold `AvailabilityChip:21-29` `TONE_MAP` into `STATUS_TONES`.
8. **C17** `MetricTile/MetricBar/Sparkline/Gauge` — replace hero grids + 4 bars + 2 sparklines (bar vs SVG polyline) that currently render same `SeriesData {rps,latency_ms,mem_mb,cpu_frac}` via incompatible impls. Nets ~120 LOC.

### Wave 4 — Structural (defer, do after Wave 1–3 scaffolding)

9. **C20** `registry!` macro + `common_layers()` helper — prevents next-slice 404; `apply_common_http_layers(router)` removes `CatchPanicLayer+TraceLayer` duplication.
10. **C12** `RunsOn` → **C18/C19** envelope/hydration (constants first, `flatten` later) → **C21** `FormSpec` table (hardest, do last when all NTES fixtures green).
11. **C22** `createSuggestStore` + **C23** sunset `static/` (audit serving first, behind feature flag).

---

## 4. Quality Gates & Verification

- **Backend:** `cargo fmt --all --check` && `cargo clippy --all-targets -- -D warnings` && `cargo test` — must keep `ntes::web` (20 tests) + `irctc::normalize` (6) + `paytm::normalize` (5) + `railyatri` (6) + `corover` (6) + `tests/schedule.rs:14` wire-shape assertions green. New helpers need own unit tests (`validate::train_5` rejects `00000`, `time::parse_date` tries 4 fmts, `ValueExt::str_one_of` prefers first key).
- **Frontend:** `npm run build` (vite) + `node --check` on all `lib/*.js` + `tests/js/form.test.mjs:118,249` update for single `DATE_RE`. Snapshot test that `#[serde(flatten)]` does not reorder keys for frontend `DataSourceBadge` consumption.
- **Telemetry trap:** `proc_stats` is delta-based via `core/obs.rs:24` `CPU_SAMPLE: Mutex<Option<(Instant,u64)>>`. After Wave 2, sampler owns `proc_stats`; `/metrics` and `/observability` only `encode()`/`snapshot()` without re-sampling. Add test that two rapid scrapes don't lose CPU delta.
- **Cache shape trap:** `live_status/service.rs:32-35` caches raw `Value` not DTO — if moved to `CacheExt::get_or_fetch_json::<LiveStatusResponse>` the cache contents change. Keep `Value` path until cache version bump or handle `from_value` error by re-fetching (already does).
- **Session `Arc<Mutex>` clone semantics:** new `CookieJar` must stay `Clone` with `Arc` inside, same as `NtesWebClient:302-311` and `IrctcClient:36-41`, else concurrent requests lose cookies. Add `#[test] cookie_jar_clone_shares_state`.

---

## 5. LOC Impact Summary

| Wave | LOC removed | LOC added (helpers) | Net |
|---|---|---|---|
| Wave 1 (C1-C3,C8-C11) | ~360 | ~120 | **-240** |
| Wave 2 (C4,C7,C15-C16) | ~560 | ~140 | **-420** |
| Wave 3 (C14,C17) | ~210 | ~90 | **-120** |
| Wave 4 (C18-C23) | ~320 | ~110 | **-210** |
| **Total** | **~1,450** | **~460** | **~-990** |

Every control preserves the vertical-slice contract (`slices/*/mod.rs` owns `router()`, `service.rs` owns mapping). Extracted code ownership is `core` or `lib` or `models`, never slice-owns-slice. No public API break — error messages, cache TTLs, latency labels, `data_source` strings stay byte-identical unless caller opts into cleaned constants.

---

## 6. Concrete Next Step (if you want to act now)

1. **PR-1 (30 min):** `src/core/cookies.rs` + `src/core/source.rs` — 2 files, 0 behaviour change, highest confidence. Deletes 90+36 LOC, fixes UA rotation drift (`BROWSER_UA` literal ×2).
2. **PR-2 (1 h):** `src/core/validate.rs` + `src/core/time.rs` + `src/core/json.rs` — 3 pure libs, 9 one-line edits per slice, deletes 43+120+90 = 253 LOC. Re-export wrappers keep import paths green.
3. **PR-3 (half day):** `frontend/src/lib/async.svelte.js` + `dates.js` + `delay.js` — replaces 400+45+40 = 485 LOC of the ~1100 frontend duplication; prevents next drift.

All three keep current `Card` chrome, `Signal & Steel` tokens (`app.css:40-51`), and Svelte 5 `$state` idioms untouched — only call sites shrink.

---

## 7. Open Questions for Owner

- **Q1:** Confirm `static/` (`static/api.js`, `ui.js:1032-1134` Autocomplete, `routes.js` hash router) is frozen legacy and safe to sunset behind flag (C23), or is it still served alongside Svelte build (would need dual bug-fix)?
- **Q2:** Approve deferring `#[serde(flatten)]` envelope (C18) to after wire-contract snapshot tests — or allow key-order change now if frontend is JSON-object (not positional)?
- **Q3:** Cache TTL table: keep per-slice TTLs as `Option<Duration>` param to `cached_or_fetch` (preserves `exceptional` 2 h, `search` 30 m) — or normalize to single default 60 s?
- **Q4:** Priority: ship Wave 1 pure helpers this week (parallel PRs) vs. block on `createResource` (Wave 2) first?

*Research artifacts retained: 5 subagent reports (backend slices, frontend components, observability/system, core infra NTES, models/web/state — ~800 lines each). Ask to expand any family into per-file diff plan.*

