# Railway-rs ("Train Bro") — Full-Stack Review & Recommendations

**Date:** 2026-08-29
**Scope:** `railway-rs/` (Rust axum backend, Svelte 5 + Vite frontend, served vanilla-JS SPA) plus repo root (`render.yaml`, `.replit`, CI).
**Method:** A 10-agent specialist review team (backend architecture, data integrations, AI/insights, Svelte frontend, SPA frontend, security, testing/devops, observability/SRE, performance/resilience, docs & data). All key findings cross-checked with direct source inspection.

---

## 1. Executive summary

The application is **functionally strong**: a coherent vertical-slice architecture (`src/slices/`), clean `core/` infrastructure, honest `data_source` reporting in most slices, a genuinely good hermetic mock-based test suite, correct security headers, and a real (not fabricated) metrics stack. **The core risks are drift and dead code** — the code evolved (Svelte rewrite, `fanout_n2` refactor, ~10 more slices and sources, an SRE/resilience layer) faster than the docs, quality gates, and cleanup of superseded code.

**Top 5 things to fix first (biggest correctness/integrity impact):**

1. **Fabricated live data is still served as real** (ConfirmTkt/Ixigo/Erail/IndiaRailInfo/Etrain synthesize fake trains & waitlists that win the fan-out race, labeled "live"). — *correctness / trust*
2. **Hardcoded personal Replit proxy** as a production data source, building a fresh `reqwest::Client` per request. — *supply-chain / perf*
3. **The configured browser User-Agent is never actually sent** (`.user_agent("railway-rs")` overrides it). — *upstream blocking / WAF evasion*
4. **Admin/observability endpoints are unauthenticated & internet-reachable**, internal error strings/upstream URLs leak to clients, and a spoofable `X-Forwarded-For` defeats rate limiting. — *security*
5. **The three Rust quality gates are red** (fmt, clippy, 2 failing ai_chat tests) and CI can't actually run its own UI/JS jobs. — *release confidence*

---

## 2. Critical — fix immediately

### 2.1 Fabricated data served as "live" (correctness / trust)
**Files:** `src/core/{confirmtkt,ixigo,erail,indiarailinfo,etrain}.rs`, wired in `src/slices/availability/service.rs:94-107,146-180` and `src/slices/trains_between/service.rs:90-119`.

- `confirmtkt.rs` returns a **hardcoded HYB→AK 17605 snapshot with exact waitlists** (`GNWL77/WL58`, fares, `prediction: 95`) on *any* network failure or non-2xx (blocks at `:58-80`, `:87-109`, `:117-139`, `:167-189`) — for *any* date, past or future. Same block duplicated 4×, no shared const.
- `ixigo.rs`, `erail.rs`, `indiarailinfo.rs`, `etrain.rs` gate "success" on `html.contains("train")` / `html.contains("Train No")` and then emit `CT{src}{dst}` / `Erail {train}` / `IndiaRailInfo {train}` fake trains with fake times and `at_src`/`at_dstn` `"false"`.
- Because fan-out is first-success-wins and these return `Ok` instantly, during the frequent Singapore IP-block the **fabricated 17605 with fake waitlists is what users get**, labeled "Live availability from ConfirmTkt … reachable worldwide" (`availability/service.rs:396-403`).

**Recommendation:** Delete or rewrite these as real parsers (mirror the reference-quality `corover.rs`). Every synthetic branch **must** return `data_source: "synthetic"` with an explicit notice and must **never** contain hardcoded day-specific waitlist/availability numbers. At minimum, drop the "HYB→AK success on Err" shortcuts.

### 2.2 Hardcoded third-party Replit proxy as a production source + per-request client
**Files:** `src/slices/live_status/service.rs:65,68,90-99`; `src/slices/average_delay/service.rs:48,51`.

- A personal `*.pike.replit.dev:3000/rail-api/...` URL is hardcoded as a fan-out candidate (not in `config.rs`, unlike every other `RAILWAY_SOURCE_*`).
- It builds a **fresh `reqwest::Client` per request** (`service.rs:68`, `average_delay/service.rs:51`) — no connection pooling in the hot N² path.
- The response reshape (`platform_number`→`next_station_code`, substring-inferred `at_src`/`at_dstn`) is semantically dubious; `state_proxy` at `live_status/service.rs:49` is dead (clippy-flagged).

**Recommendation:** Move the proxy behind `config` (`RAILWAY_REPLIT_PROXY_BASE`) or drop the candidate entirely. Reuse `state.http`. Delete the leftover clone. Fix the field mapping.

### 2.3 Configured browser UA is never sent (upstream WAF bait)
**File:** `src/core/http.rs:41` (and `:20-34`, `config.rs:98`, `state.rs:63`).

`Client::builder().default_headers({UA: passed_ua}).user_agent("railway-rs")` — reqwest's `.user_agent()` overrides `default_headers`. So every shared-client request loudly announces `User-Agent: railway-rs`. Also `http.rs:27-31` defaults `Content-Type: application/json` on GETs and **`Referer: enquiry.indianrail.gov.in` on all sources** (wrong origin for Paytm/Ixigo/etc.). Only `corover.rs` builds a correct identity.

**Recommendation:** Remove the `.user_agent("railway-rs")` override (or drop UA from defaults — one source of truth); move per-source `Referer` to per-call headers; drop `Content-Type` from GET defaults.

### 2.4 Vanilla SPA / Tabs-registry is dead; the live app is the Svelte build
**Files:** `static/index.html:14-15` loads only `theme-boot.js` + built `assets/index-*.js`/`.css`; `static/tabs/` and `static/app.js` **do not exist**.

The whole `window.Tabs` registry pattern, `static/api.js`, `routes.js`, `ui.js`, `styles.css`, `palette.js` (≈125 KB) are **orphaned** — served by the backend (`src/web.rs:112` `ServeDir`) but never referenced by the served `index.html`. Skills `spa-tab`, `vertical-slice-rust` (Phase B), `observability-stack`, `ui-layout-harness` describe this vanished architecture, misdirecting agents/contributors.

**Recommendation:** Decide the canonical UI (Svelte `frontend/` is clearly it), prune/relocate orphaned `static/*` loose files, delete stale `assets/index-*.js` bundles (keep only the one `index.html` references), and rewrite the affected skills to target `frontend/src/lib/pages/` + `lib/components/`.

---

## 3. High — fix next

### 3.1 Security: unauthenticated admin/observability endpoints
**Files:** `src/system.rs:24-38`, `src/slices/observability/mod.rs:29-57`, `src/main.rs:102,107` (binds 0.0.0.0).

`GET /metrics`, `/rail-api/source-status`, `/rail-api/capacity`, `/rail-api/observability`, `/rail-api/logs`, `POST /rail-api/debug`, `POST /rail-api/ai/chat` are all public. Anyone can read full runtime metrics, deep source health/circuit state, live logs (up to 500 entries), and **inject forged log lines** (`POST /rail-api/debug`, `system.rs:438-451`).

**Recommendation:** Gate these behind an admin token / separate `127.0.0.1` listener / basic-auth. Keep `/healthz`, `/readyz`, and public data endpoints open. Sanitize/escape the debug-log-injection endpoint.

### 3.2 Security: internal errors & upstream URLs leak to clients
**File:** `src/core/error.rs:94-139`.

`AppError::Internal(msg)` returns the full `msg` verbatim (HTTP 500); `From<reqwest::Error>` embeds the **complete upstream URL**; `SourceUnavailable{reason}` echoes the reason. Confirmed at `error.rs:120-127`.

**Recommendation:** Return a generic `{"error":"internal server error"}` (optionally a generated request-id); log detail server-side only (already done at `error.rs:117,121`). Keep user-facing `BadRequest`/`NotFound` messages.

### 3.3 Security: rate limiter trusts spoofable `X-Forwarded-For` + permissive default
**Files:** `src/web.rs:163-177,206-207`; `src/config.rs:119-120`.

`client_ip()` takes the **first** entry of the client-supplied `X-Forwarded-For` verbatim → a caller can rotate it per request to bypass the per-IP bucket entirely. Default is 1000 rps / 1000 burst; `.env.example` documents 100/50 (10× mismatch). All direct connections collapse into one `"unknown"` bucket.

**Recommendation:** Only trust forwarded headers from a known trusted proxy (check socket peer first); tighten defaults (e.g. 60–150 rps / burst 30–60); align `.env.example`.

### 3.4 AI chat: cost/abuse controls missing
**Files:** `src/core/ai/client.rs:184-191`, `src/slices/ai_chat/mod.rs:92-223`, `src/web.rs:34-41,89-98,163-222`.

- No `max_tokens`/`temperature` → a model can stream an arbitrarily long/expensive answer (`client.rs:184-191`).
- Client disconnect is not honored: the spawned tool loop uses `let _ = tx.send().await` everywhere and keeps running real rail tools + up to 4 rounds after the client leaves.
- `/rail-api/ai/chat` is unauthenticated with no dedicated quota/cost cap (shares only the spoofable generic limiter).

**Recommendation:** Set `max_tokens`/`temperature`; `break` the loop when `tx.send` fails (receiver dropped) or a `CancellationToken` fires; add a dedicated AI per-IP + global rate/cost cap; consider gating the endpoint on the configured `RAILWAY_AI_API_KEY`.

### 3.5 Perf: unbounded fan-out amplification (no cap on concurrent upstream calls)
**File:** `src/core/fanout.rs:59-84` (and callers).

Races N×2 delegates × 2 retries = up to N×2×2 concurrent upstream HTTP calls **per uncached request**, with no global semaphore on fan-out child tasks. The `web.rs` bulkhead counts handler permits, not upstream calls. A slow/blocked source is re-hit by every in-flight request.

**Recommendation:** Add a process-wide semaphore capping concurrent upstream fetches; apply per-source rate limiting + jittered retry; rely on the (working) circuit breaker to skip already-open sources.

### 3.6 Perf: unbounded cache with O(n) write-path lock hold
**File:** `src/core/cache.rs:15-19,77`.

`Mutex<HashMap<String,Entry>>` has no capacity limit; `set_with_ttl` holds the global lock while `retain` scans the **entire map** on every write (O(n) under lock). High-cardinality keys (`live_status:{train}:{date}`, `availability:...:date:source`, `search:stations:{q}`) grow without bound.

**Recommendation:** Cap the map (LRU / max-entries eviction); move sweeping to a background task; avoid O(n) scan-per-write.

### 3.7 Perf: no cache-stampede coalescing on expensive fan-out misses
**Files:** `src/slices/live_status/service.rs:34-38`, `average_delay/service.rs:22-24`.

On cache miss, every concurrent request fires the full N² fan-out to the same upstreams (hot train right after expiry = storm).

**Recommendation:** Add per-key single-flight / request coalescing (share one in-flight future among waiters).

### 3.8 Perf: every buffered response is fully buffered in memory to count bytes
**File:** `src/web.rs:300` `axum::body::to_bytes(body, 64MiB)`.

Drains and re-serializes the whole body (up to 64 MiB each) on every non-SSE response. Also means in-flight/byte metrics undercount AI/streaming traffic (`web.rs:89-98`), so load-shedding (`web.rs:260`) misses streaming load.

**Recommendation:** Count bytes via a streaming wrapper or `content-length`; lower the byte cap; only buffer where strictly needed.

### 3.9 Observability: fabricated metric — hardcoded 10 ms source latency
**File:** `src/slices/pnr/service.rs:748` — `record_source_latency("railyatri", Duration::from_millis(10))`.

A constant 10 ms is recorded regardless of real fetch time. This pollutes the source-latency EMA, the `origins` table, `/series` charts, and the Prometheus `railway_source_latency_ms{source="railyatri"}` gauge — one whole source's latency signal is wrong and it inflates "live" confidence.

**Recommendation:** Measure real elapsed time (`Instant::now()` … `started.elapsed()`) like every other slice.

### 3.10 CI cannot run its own JS/UI gates; builds are cold
**Files:** `.github/workflows/ci.yml:40-56`, `Makefile:33-50`, `package.json`.

- `make check-js` runs `node --test tests/js/` but **`jsdom` is never installed** in CI (no `npm ci`/`npm install` step) → `MODULE_NOT_FOUND: jsdom` → the whole target aborts before `check-ui`.
- The real-browser UI suite **silently skips** on bootstrap failure (`UI_STRICT` never set in CI), and the frontend job never builds the Rust binary the UI harness requires — so it can be green-with-zero-coverage.
- No cache: fmt/clippy/test/release build is a cold rebuild every push; the Dockerfile invalidates the whole dependency build on any `src/` change (`Dockerfile:5-7`).

**Recommendation:** Add `npm ci` before `make check-js`; run the UI suite strictly (`UI_STRICT=1`) on the job that builds the binary, and cache the harness; add `Swatinem/rust-cache`; use BuildKit cache mounts in the Dockerfile; run `make build-ui` + `git diff --exit-code static/` in CI to force committed-up-to-date bundles.

### 3.11 Tests assert the wrong default AI model (gate red)
**File:** `tests/ai_chat.rs:72,:167` assert default model `"x-preview-f-free"`, but code default is `"muse-spark-1.2-contributor-free"` (`src/config.rs:111`). Two tests fail (`happy_path_relays_sse...`, `status_reports_configuration_truth`).

**Recommendation:** Sync the tests to the real default (the model changed in commit 1136f6d without updating tests).

### 3.12 NTES crypto integrity not verified; secrets hardcoded
**File:** `src/core/ntes/crypto.rs:69-98,18-20`.

`decrypt` discards the MD5 hash segment (split at `#`) instead of verifying it; `String::from_utf8_lossy` silently mangles corrupt plaintext. KEY/IV/SCKEY hardcoded at `:18-20` (rotation = silent breakage; no env override).

**Recommendation:** Verify `MD5(payload+SCKEY)` of the decrypted text and reject mismatch; gate secrets behind env (document as public protocol constants); add a round-trip fixture test.

### 3.13 HTML parsing is brittle regex/split with minimal entity unescaping
**File:** `src/core/ntes/web.rs` (regex captures at ~26 sites, `split("<tr")`, `strip_tags` unescapes only `&nbsp;`).

Any markup change silently degrades to garbage rows; `pos_code` requires exact "Departed from NAME(CODE)" text. CSRF relies on a hidden-input regex.

**Recommendation:** Switch to a real HTML parser (`scraper`/`kuchiki`) with entity decoding and structure-based extraction; add golden fixtures per form.

### 3.14 `make check-js` breaks on the built ESM bundle; Svelte build never cleans up
**Files:** `Makefile:33-35`, `frontend/vite.config.js:15-16`.

`node --check` over `static/*.js` includes the minified **ESM** bundle → parse error (gate red, verified). Vite `outDir: '../static'` with `emptyOutDir: false` → every build leaves stale hashed bundles (multiple `index-*.js` ≈868 KB each + `embed-*.js`), all served.

**Recommendation:** Exclude the Svelte output dir from `check-js`; point Vite at a scoped out dir with `emptyOutDir: true`; prune stale bundles.

---

## 4. Medium — plan for next iteration

### 4.1 Dead code / drift after the `fanout_n2` refactor
- **`src/core/aggregator.rs`** (202 lines) + `DataSource` trait + `SourceOutcome` are **never referenced** in any production path (superseded by `fanout_n2`); kept alive only by `pub use` at `core/mod.rs:29`. Delete, or keep one fan-out engine.
- **Dead models** `SourceHealth` / `SourceStatus` (`src/models.rs`), unused import `system.rs:21`.
- **PNR CAPTCHA flow is inert**: `challenge`/`answer`/`send_get` at `pnr/service.rs:283,462,1000` never used; `AppError::CaptchaRequired` (428) has no live solving route.
- **Dead PNR stub**: `availability/service.rs:167-169` "IndiaRailInfo fallback" always errors immediately.
- **Live-station leftover**: `live_station/service.rs:119-121` computes `static_board(...)` then discards it (`resp` dropped, empty `{trainList:[]}`).

**Recommendation:** Sweep the above; clippy `-D warnings` currently fails at ~50 lints across unused imports/vars, dead code, and style (`live_status/service.rs:49`, `live_station/service.rs:33-36,51-52,71,73,119,128`, `availability/service.rs:163-165`, `pnr/service.rs:62,334,335`, `train_on_map/service.rs:62-63`, `obs.rs:124`, `railyatri/mod.rs:339`, plus doc-comment spacing).

### 4.2 `cargo fmt` is red in ~30 files
`cargo fmt --all --check` produces diffs across many `src/**/service.rs`, `system.rs`, `web.rs`, `sre.rs`, `obs.rs`, `fanout.rs`, plus `tests/*`. Run once to restore.

### 4.3 Config drift & inconsistent boolean-env semantics
- `RAILWAY_AI_ENABLED` uses inverse-falsy parsing (unknown→true) (`config.rs:159-166`); `ASKDISHA_ENABLED` uses strict truthy (unknown→false) (`config.rs:176-181`). Same knob type, two contracts.
- Doc: `RAILWAY_RATE_LIMIT_RPS` default `100` (`config.rs:76`) vs actual `1000` (`config.rs:119`).

### 4.4 Rate limiting collapses without a proxy / peer address
`client_ip()` returns `"unknown"` for direct connections (no `X-Forwarded-For`) → all such clients share one bucket (a global limiter, not per-IP). Use `ConnectInfo` peer IP + trusted-proxy config.

### 4.5 AI/observability endpoints use the spoofable generic limiter
`/metrics`, `/rail-api/source-status`, `/rail-api/capacity`, `/rail-api/observability`, `/rail-api/logs`, `/rail-api/ai/*` all share the generic (spoofable) limiter with no dedicated cap.

### 4.6 Svelte frontend gaps
- **No per-route titles / meta** (H1): only `index.html:8` static `<title>`; no `document.title`/OG for deep links.
- **No code-splitting** (H2): single ~868 KB bundle + Leaflet statically imported (`RouteMap.svelte:4`) even when the Map tab is never opened.
- **Duplicate icon packages** (M1): `lucide-svelte@^1.0.1` + `@lucide/svelte@^1.33.0` both imported.
- **No `svelte-check`/typecheck step** (M5).
- **Captcha UX** (M4): `Pnr.svelte:123-127` re-submits a stale `captchaText` with no "refresh captcha".
- **Document-level click vs dialog race** (M2): `AutoCompleteInput.svelte:182-185` outside-click can dismiss the nearby-station dialog.

### 4.7 SPA legacy (if not fully deleted) — XSS & broken helpers
- `static/ui.js:9,68,163,175-178` — raw `innerHTML` sinks; `esc()` isn't enforced; `statusCell` interpolates upstream text. CSP mitigates `<script>` but not event-handler/markup injection (`<img onerror>` would run).
- `static/api.js:40-60` — `RailLog` is **undefined** → `ReferenceError` on first API call.
- `static/ui.js:145-152` — `withLoading` docs promise `[setLoading,setError]` but returns only `setLoading`.
- `static/ui.js:48,:303` — `emptyState` name collision (two component generations); `ui.js` is a 1,135-line monolith.
- ARIA: `seg()`/`console()` tabs lack arrow-key nav + `aria-controls`; autocomplete/palette aren't proper listbox/combobox.
- `styles.css` uses `light-dark()`/`color-mix()` with no legacy fallback (fails on older Android WebViews).

### 4.8 Observability refinements
- **Prometheus path label cardinality** (`src/core/obs.rs:186-199`): raw URL `path` as a label → unbounded label set (`/rail-api/train/12345`, PNR, captcha sessions). Normalize to route templates.
- **`req_per_sec` is uptime-average** (`metrics.rs:344-350`), not rolling — capacity/scale-up decisions (`system.rs:315`) run on a stale number. (Rolling RPS is computed in `sample_series` but not propagated.)
- **Source failures invisible** (`metrics.rs:273`): `record_source_latency` only stores successful samples → a failing source shows stale/zero latency. Add `source_failures_total`.
- **`/readyz`/`source-status` do live upstream GETs every call** (`system.rs:564-598`), and treat 5xx as "reachable". Cache probes with a short TTL; distinguish up vs erroring.
- **`railway_ready` initializes to 1** (`obs.rs:384`).
- **No panic hook** (`main.rs:75` spawn, `tokio::spawn` fan-out) → panics bypass the JSON/file/ring pipeline and can silently kill background tasks.

### 4.9 Resilience details
- Retry is fixed-delay (400 ms), retries 4xx/404, no jitter (`http.rs:55-81`); fan-out `RETRY_DELAY` fixed 200 ms (`fanout.rs:31`); HTTP-layer POSTs have no retry.
- `failover.ordered` allocates/locks per fan-out (`fanout.rs:70-87`); rate-limiter eviction is arbitrary `keys().next()` not LRU (`resilience.rs:50`).
- AI SSE backpressure: a slow/disconnected client can block the upstream LLM read + bulkhead permit for the full round (`ai_chat/mod.rs:92`); no overall streaming deadline (`TOOL_TIMEOUT` 20s × 4 rounds can exceed the client budget).
- `std::sync::Mutex` on the Tokio executor in `Cache`/`Failover`/`RateLimiter`/`Metrics` — safe today (no cross-await), monitor hold time; prefer `parking_lot`.

### 4.10 Error classification by string-sniffing
`availability/service.rs:120-138`, `exceptional/service.rs:59-64`, `journey_basis/service.rs:47-52` match on `"no direct trains"`, `"not_found"`, `"timeout"`, `"circuit open"` substrings. Add typed error kinds on `AppError` instead of message-text matching.

### 4.11 Static/local fallbacks answer 200-with-empty instead of honest degradation
`availability/service.rs:182-192`, `exceptional/service.rs:65-73`, `journey_basis/service.rs:129-139`, `live_station/service.rs:112-123,319-370` (a hardcoded 7-train HYB board with static times + `ST####/Static` filler), `pnr/service.rs:167-181` (a `local-validator` candidate that can only ever fail, inflating candidate count N). Return honest `503`/`204` + notice, or compute from the dataset.

### 4.12 Normalization duplication + one gap
- `date_compact` twice (`paytm/normalize.rs:15-23`, `irctc/normalize.rs:26-30`); `deep_get` twice; 7-day-run parsing in ≥5 places.
- Paytm times drop the day (`iso_time`, `paytm/normalize.rs:146-155`) — UI shows day-less times.
- The HYB→AK 17605 literal is duplicated 7×.

---

## 5. Low — maintenance / polish

- **Unused dep:** `thiserror = "1"` (`Cargo.toml:25`); `AppError` hand-implements `Display`/`From` instead of `#[derive(thiserror::Error)]`.
- **`time = "=0.3.36"` pin** (`Cargo.toml:32`) + no `cargo audit`/`cargo deny`/Dependabot — add vulnerability scanning.
- **Obs:** `use_errors_total` is a gauge named like a counter (`obs.rs:344-347`); redundant fine-print aliases in `sre.rs`; `HEDGING_NOTE` const exists only to echo a string.
- **`code_known`** (`station_codes.rs:22-33`) linear-scans all stations+trains, token-splitting every train name per request (hot slices `trains_between`, `availability`). Build a `HashSet<String>` at startup → O(1).
- **BM25 exact-match O(n)** (`retrieval.rs:146-151`) on the async executor — precompute an exact-code map.
- **Startup duplicates dataset memory** (~3 copies of station/train identity: `Datasets` Arc vecs + `station_lc`/`train_lc` + `retrieval_entries()` clone).
- **AI:** raw chain-of-thought streamed & rendered (`Assistant.svelte:496-501`); `PERSONA` doesn't forbid code fences (known formatting complaint); single-shot `chat_complete` path built but unused; inbound roles restricted to user/assistant (good).
- **MCP** builds the full `AppState` incl. AI backend though never used (`railway-mcp.rs:18`).
- **Logs capture client IP at info** (`web.rs:320-325`) — sampling/debug consideration.
- **systemd unit** `EnvironmentFile` lacks `-` prefix (`deploy/railway-rs.service:10`) vs "optional" README claim.
- **Svelte:** `RouteMap` rebuilds layers per change (minor); `Availability` width-breakpoint duplication; router drops trailing slash / no normalize on popstate.
- **CORS:** none configured — the safe default; document it, add an origin allowlist only if a separate client domain appears.
- **`.replit`** run block sets dead env vars (`RAILWAY_AI_BACKEND`, `RAILWAY_LOCAL_THREADS`) that don't exist in `config.rs`.

---

## 6. Documentation & data hygiene

- **C1 — README is materially stale:** the `src` tree and endpoint table cover only ~a third of the real surface. Missing from docs: slices `average_delay`, `heritage`, `parcel`, `journey_basis`, `station_timetable`, `train_on_map`, `askdisha`, `mcp`; and endpoints `/rail-api/ntes/*`, all `/rail-api/askdisha/*`, `/rail-api/capacity`, `/rail-api/debug`, `/rail-api/nearby/stations`, `/rail-api/stations/:code`.
- **C2 — `.replit`** references removed local-LLM config.
- **H1 — `RAILWAY_AI_MODEL` default mismatch:** README/.env.example say `x-preview-f-free`; code is `muse-spark-1.2-contributor-free` (`config.rs:111`).
- **H2 — `.env.example`** documents only ~half the real env vars and lists some that don't exist (missing all `RAILWAY_SOURCE_*_BASE`, `RAILWAY_AI_API_KEY`, resilience/rate-limit/load-shed vars).
- **H3 — `PLAN.md`** (605 lines) describes a vanilla-JS rearchitecture superseded by Svelte; all `[ ]` checkboxes still unchecked. `docs/REARCHITECTURE.md` likewise stale. Either banner as historical or rewrite.
- **M1 — Orphaned 101 MB model files** (`models/trainbro.gguf` 105 MB, `tokenizer.json` 2 MB) with zero code references; gitignored but add to `.dockerignore`.
- **M2 — `scripts/convert_stations.cjs`** produces a 4-field schema; committed `data/stations.json` has 10 fields.
- **M3 — stale Svelte bundles committed** (~2.5 MB of `static/assets/`), largest history blobs.
- **M4 — `docs/SRE.md`, `src/core/resilience.rs`, `src/core/sre.rs` are untracked** (uncommitted work).
- **L3 — `/readyz`, `/rail-api/capacity`** missing from the README endpoint table.

---

## 7. What's genuinely good (protect these)

- **Vertical-slice architecture:** clean, consistent `router()` export per slice; shared `AppState`; single well-built `HttpClient`; correct timeout/fan-out/circuit-breaker/resilience layering.
- **Honest `data_source` reporting** in the well-implemented slices (`corover.rs` is the reference normalizer; `ntes/client.rs` returns honest `SourceUnavailable`).
- **Hermetic tests:** ~167 integration tests across 26 files + ~36 in-src `#[cfg(test)]` modules, all mock-driven; `tests/live.rs` correctly env-gated; real page fixtures in `testdata/`.
- **Svelte frontend:** disciplined Svelte 5 runes, `AsyncState` primitive, `api.js` stale-while-revalidate, WeakMap memo caches, ARIA on combobox/DataTable, XSS-safe markdown renderer (escape-before-inline + URL allowlist).
- **Security headers:** strong baseline (CSP, `nosniff`, `X-Frame-Options: DENY`, Referrer-Policy).
- **Real metrics:** bounded in-memory path cap, structured JSON logs with rotation, `/healthz` + Render `PORT` fallback.

---

## 8. Suggested remediation roadmap

**Phase 1 — integrity & trust (day 1):**
1. Remove fabricated-data branches from ConfirmTkt/Ixigo/Erail/IndiaRailInfo/Etrain (return `data_source:"synthetic"` or honest errors).
2. Gate/remove the hardcoded Replit proxy; reuse `state.http`.
3. Fix the User-Agent override bug.
4. Delete dead `aggregator.rs`/`DataSource`/`SourceHealth`/`SourceStatus`/PNR-CAPTCHA remnants.
5. Fix the fabricated `railyatri` 10 ms latency metric.
6. Sync the two failing ai_chat tests.

**Phase 2 — security (week 1):**
1. Auth-gate admin/observability + AI chat endpoints (admin token / `127.0.0.1` listener).
2. Stop leaking internal errors/URLs to clients.
3. Fix rate-limit IP source (trusted proxy/peer IP) + tighten defaults.
4. Add dedicated AI rate/cost cap.
5. Add `cargo audit`/Dependabot; reconsider the `time` pin.

**Phase 3 — reliability & perf (week 2):**
1. Bound concurrent fan-out (semaphore) + jittered/exponential retry.
2. Bound the cache (LRU) + move eviction off the lock; add request coalescing on cache miss.
3. Stop fully buffering every response.
4. Normalize Prometheus `path` labels; expose rolling RPS; add source-failure counters; add a panic hook.

**Phase 4 — release confidence (week 3):**
1. `cargo fmt` once; clear the ~50 clippy lints.
2. Fix CI: install `jsdom`/`npm ci`, run UI suite strictly (`UI_STRICT=1`) on the binary-building job, add rust-cache + BuildKit cache, run `make build-ui` + `git diff --exit-code static/`.
3. Clean stale Svelte bundles + orphaned vanilla SPA; decide canonical UI.

**Phase 5 — documentation (week 3-4):**
1. Rewrite README `src` tree + full endpoint table + `.env.example` to match reality.
2. Banner `PLAN.md`/`REARCHITECTURE.md` as historical; update the affected skills (`spa-tab`, `vertical-slice-rust`, `observability-stack`, `ui-layout-harness`) to the Svelte architecture.
3. Write actual SRE runbooks in `docs/SRE.md`; commit `resilience.rs`/`sre.rs`/`SRE.md`.
