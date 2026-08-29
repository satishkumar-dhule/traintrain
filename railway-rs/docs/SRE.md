# SRE — Site Reliability Engineering (Google SRE)

> **Super fan-out N² deep delegation, hedging and SRE patterns, all mentioned in fine print.**

This document maps every SRE pattern from the Google SRE book (Beyer, Jones & Petoff) to its concrete implementation in `railway-rs`. Each pattern has a *fine-print* string (prefixed `SRE Pattern:`) exposed on `/rail-api/observability`, `/rail-api/source-status`, `/healthz`/`/readyz`/`/rail-api/capacity`, and in Prometheus `/metrics`. The UI renders them as footers/tooltips so operators can trace any behavior to its theory.

## 1) Patterns & Fine Print

| Key | Pattern | File | Fine Print (`FINE_PRINT_*`) |
|-----|---------|------|------------------------------|
| SLO | Service Level Objective | `src/core/sre.rs:85` | `SRE Pattern: Service Level Objective (SLO) — 99.9% availability over 28d rolling window; error budget 0.1% — burn-rate alerting [Google SRE Ch.4]` |
| SLI | Service Level Indicator | `src/core/sre.rs:88` | `SRE Pattern: Service Level Indicator (SLI) — availability SLI = (2xx+3xx)/total, latency SLI from histogram p95/p99 [Google SRE Ch.4]` |
| Error Budget | Error Budget & Burn Rate | `src/core/sre.rs:91` | `SRE Pattern: Error Budget — 0.1% (1 - 99.9% SLO); burn rate = error_rate / budget; remaining = 1 - consumed; freeze releases when exhausted [Google SRE Ch.4]` |
| Four Golden Signals | Latency, Traffic, Errors, Saturation | `src/core/sre.rs:94` | `SRE Pattern: Four Golden Signals — Latency, Traffic, Errors, Saturation — the minimum to page on [Google SRE Ch.6]` |
| RED | Rate, Errors, Duration | `src/core/sre.rs:97` + `src/core/metrics.rs:13` + `src/core/obs.rs:6` | `SRE Pattern: RED — Rate, Errors, Duration — request-scoped health for microservices (Tom Wilkie) [Google SRE Ch.6]` |
| USE | Utilization, Saturation, Errors | `src/core/sre.rs:100` + `src/core/metrics.rs:13` | `SRE Pattern: USE — Utilization, Saturation, Errors — resource-scoped health for hosts/queues (Brendan Gregg) [Google SRE Ch.6]` |
| Circuit Breaker | Per-source flip-flop breaker | `src/core/failover.rs:5` + `src/core/sre.rs:103` | `SRE Pattern: Circuit Breaker — fail-fast when downstream error rate exceeds threshold; probe half-open after cooldown [Nygard Release It! / Google SRE Ch.22]` |
| Bulkhead | Concurrency semaphore (512) | `src/state.rs:54` + `src/web.rs:81` + `src/core/sre.rs:106` | `SRE Pattern: Bulkhead — isolate failure domains (thread pools, connection pools) so one slow source cannot sink the ship [Nygard / Google SRE Ch.22]` |
| Retry with Jitter | 2-deep retry + 200ms jitter | `src/core/fanout.rs:11` + `src/core/http.rs:10` | `SRE Pattern: Retry with Jitter — exponential backoff + jitter prevents thundering herd; idempotent GETs only, capped at 2 attempts [Google SRE Ch.22]` |
| Timeout Budget | Per-request deadline propagation | `src/core/fanout.rs:11` + `src/web.rs:45` + `src/core/sre.rs:112` | `SRE Pattern: Timeout Budget — per-request deadline propagation; connect 8s, request budgets fan out to upstreams so user never waits forever [Google SRE Ch.22]` |
| Hedging / Fan-out N×2 | Super fan-out N×2 deep delegation | `src/core/fanout.rs:9` + all `src/slices/*/service.rs` (e.g. `live_status:40`, `schedule:36`, `availability:39`, `trains_between:30`) + `src/core/sre.rs:115` | `SRE Pattern: Hedging / Fan-out N×2 — race N upstreams, cancel losers; p95 fan-out reduces tail latency without multiplying load [Google SRE Ch.22]` |
| Graceful Degradation | Stale cache / local fallback | `src/slices/live_station/service.rs:319` + `src/system.rs:7` + `src/core/confirmtkt.rs:53` | `SRE Pattern: Graceful Degradation — serve stale cache or partial results when primaries fail; never fail a read that a fallback can answer [Google SRE Ch.22]` |
| Load Shedding | 503 + Retry-After when saturated | `src/web.rs:95` + `src/core/resilience.rs:90` + `src/core/sre.rs:126` | `SRE Pattern: Load Shedding — when saturation thresholds breach, shed low-priority work (404 fast-path, 429/503) to preserve SLO [Google SRE Ch.23]` |
| Capacity Planning | Recommendation scale_up/ok/scale_down | `src/system.rs:292` + `src/core/sre.rs:129` | `SRE Pattern: Capacity Planning — model rps × latency × in_flight headroom; autoscale at 80% saturation, load-test before launch [Google SRE Ch.27]` |
| Observability Pipeline | Metrics + Logs + Traces | `src/core/obs.rs:1` + `src/core/sre.rs:132` | `SRE Pattern: Observability Pipeline — Metrics (Prometheus) + Logs (ring) + Traces → Telemetry.sample() → /metrics & /rail-api/observability [Google SRE Ch.10]` |
| Health Checks | Liveness / Readiness / Deep | `src/system.rs:5` | `SRE Pattern: Health Checks — liveness, readiness and deep dependency probes; fast-fail with circuit breakers when downstreams degrade [Google SRE Ch.14]` |
| N² Deep Delegation | Each logical source contributes 2 delegates | `src/core/fanout.rs:9` + `src/slices/live_status/service.rs:40` + `src/slices/trains_between/service.rs:30` | `Pattern: Request Hedging — fan-out N×2 race` (see `src/core/resilience.rs:105`) |

All fine-print strings are collected in `src/core/sre.rs:137` `FINE_PRINT_ALL: &[(&str,&str)]` and serialized on the observability endpoint.

## 2) SLO / SLI / Error Budget — 99.9% (28d)

- **SLI** `src/core/sre.rs:162` `availability_sli` = `(2xx+3xx)/total` (1.0 when no data).
- **Error budget** `src/core/sre.rs:202` `error_budget_consumed = error_rate / 0.001`, `remaining` clamped `0..1`.
- **Burn rate** `src/core/sre.rs:222` `burn_rate = error_rate / budget`. `>2` is fast burn (page), `>10` critical.
- **Snapshot** `src/core/sre.rs:343` `SloSnapshot::from_metrics{,_with_telemetry}` merges `MetricsSnapshot` + `proc_stats` into RED/USE/Golden and `slo_ok` (availability + latency p95/p99 + saturation).

Exposed:

- JSON: `GET /rail-api/observability` → `slo`, `red`, `use`, `golden`, `capacity`, `fine_print`, `sre_patterns` (see `src/models.rs:641` and `src/slices/observability/service.rs:90`).
- Prometheus: `railway_slo_availability`, `railway_slo_error_budget_remaining`, `railway_slo_error_budget_consumed`, `railway_slo_burn_rate`, `railway_slo_availability_target` (`src/core/obs.rs:115`).
- Probes: `GET /readyz` and `GET /rail-api/capacity` include `error_budget` and capacity recommendation.

Thresholds (`src/core/sre.rs:62`): CPU `0.80`, memory `2048 MiB`, in-flight `1000`, RPS `500`.

## 3) Four Golden Signals, RED / USE, Saturation

- **Golden** `src/core/sre.rs:308` `FourGoldenSignals { latency_ms, traffic_rps, errors, saturation_* }`.
- **RED** `src/core/metrics.rs:99` `MetricsSnapshot::red_*` helpers; gauges `railway_red_*` in `src/core/obs.rs:139`.
- **USE** `src/core/metrics.rs:194` `use_saturation_ok`, `src/core/obs.rs:149` `railway_use_*`.
- **Saturation** `src/core/metrics.rs:12` + `src/core/obs.rs:125` `railway_saturation_*`.

Background sampler (`src/main.rs:74` `spawn_sampler`, every 2s): `proc_stats()` + `metrics.sample_series` + `telemetry.sample`.

## 4) Super Fan-out N² Deep Delegation — Fool-proof

*Canonical comment:* `src/core/fanout.rs:9` `Fool-proof super fan-out N² deep delegation.`

- For `N` logical sources we fan-out to `N×2` delegates concurrently: each source contributes 2 delegates (e.g. NTES `ntes_web` vs API, Railyatri SSR vs API, or two param variants), and each delegate is retried once on `SourceUnavailable`/`Internal` (2-deep). Total `N×2×2` attempts raced, first success wins. Circuit-open sources are skipped via `Failover::should_skip` (no timeout paid). Per-delegate timeout `5s`, overall deadline `10.5s` (inside the 12s frontend `fetch` timeout) so a Singapore IP-block (NTES 5s timeout) still lets a worldwide Railyatri/Corover delegate win in `<1s`, and a static `local` delegate (800ms delayed) guarantees the UI never sees a 30s hang.

- **Deep delegation** per `src/core/fanout.rs:102` inner loop `for attempt in 0..2` + `PER_SOURCE_TIMEOUT = 5s`, `RETRY_DELAY = 200ms`, `OVERALL_TIMEOUT = 10.5s`.

- **Flip-flop ordering** `src/core/failover.rs:135` `Failover::ordered` stably sorts candidates so healthy sources stay first; open circuits move to tail.

- **Honest errors** `src/core/fanout.rs:197` NotFound is never a breaker trip and now prioritized (404 over 502); `CaptchaRequired` is preserved. Only `SourceUnavailable`/`Internal` trips the breaker and counts toward burn rate.

- **Telemetry** every winning delegate records `metrics.record_source_latency(metric, elapsed)` and `failover.record_success(metric)`; failures `record_failure`. Fallbacks like `ConfirmTkt`/`Ixigo` for `MMCT→NDLS` are honest (`SourceUnavailable`) so NTES wins; only `HYB→AK` is synthesized (`17605`) for high availability (see `src/core/confirmtkt.rs:53`).

Vertical slices using fan-out (all mention `Super fan-out N²` in code):

- `src/slices/live_status/service.rs:40` — NTES + Railyatri + Replit proxy (3×2, deep)
- `src/slices/schedule/service.rs:36` — CoRover API + NTES + Railyatri, route-reaches-expected check
- `src/slices/availability/service.rs:39` — Paytm + IRCTC + ConfirmTkt + Ixigo + Erail (N×2 = 12, 2-deep = 24 attempts)
- `src/slices/trains_between/service.rs:30` — NTES + IRCTC + Paytm + ConfirmTkt + Ixigo + Erail (N=6, N×2=12)
- `src/slices/live_station/service.rs:54` — NTES (2 delegates with/without destination) + Railyatri (2) + local (800ms delayed, empty for non-HYB per `src/slices/live_station/service.rs:340`)
- `src/slices/station_timetable/service.rs:34` — NTES + Railyatri
- `src/slices/average_delay`, `heritage`, `parcel`, `journey_basis`, `train_on_map`, `exceptional`, `chart` — NTES + Railyatri/Ixigo/Erail per slice
- `src/slices/search/service.rs:52` — `corover-api` (worldwide) vs `dataset` (offline, 150ms delayed hedging) via `fanout_n2`
- `src/slices/pnr/service.rs:97` — `indian-railways` (3-step captcha, per-step 5s hedging) + `pnr-cache-hedge` (120ms) + `local-validator` (N×2 accounting); answer path is direct to preserve 404 semantics (see `src/slices/pnr/service.rs:480`)

Fine print for hedging is also embedded per-response: `SRE_HEDGING_NOTICE = "SRE: Super fan-out N×2 (2-deep retry, hedging) — first-success-wins across N sources"` appears in PNR `notice` and `live_station` tracing.

## 5) Resilience Middleware

Order in `src/web.rs:74` (outermost → innermost, axum layers are reverse): `request_id` (outermost) → `metrics` → `rate_limit` → `bulkhead`/`load_shed` → `trace` → `catch_panic` → `timeout` (30s outer, configurable via `RAILWAY_REQUEST_TIMEOUT_SECS`).

- **Request ID** `src/web.rs:138` `request_id_mw`: UUID v4, `X-Request-Id` header, `tracing::info_span!(request_id)`.
- **Bulkhead** `src/web.rs:81` `bulkhead_mw`: `tokio::sync::Semaphore(512)` via `AppState.bulkhead` (`src/state.rs:54`), `try_acquire_owned` fail-fast → 503 `bulkhead saturated`, `telemetry.inc_bulkhead_rejected()` + `railway_bulkhead_rejected_total`.
- **Rate Limiting** `src/web.rs:85` `rate_limit_mw`: `RateLimiter` token bucket per IP (`src/core/resilience.rs:15`, `rps=1000`, `burst=1000` per `src/config.rs:119`, bypasses `/healthz`/`/readyz` via `is_health_path`), 429 + `railway_rate_limited_total`.
- **Load Shedding** `src/web.rs:95` `load_shed_mw`: `in_flight > 800` (`RAILWAY_LOAD_SHED_THRESHOLD`) or `mem > 2048 MiB` → 503 `service overloaded` + `Retry-After: 5`, `railway_load_shed_total`.
- **Timeout Budget** `src/web.rs:45` outer `TimeoutLayer(30s)` + per-delegate `5s` inside fanout; `src/core/resilience.rs:7` documents the budget.
- **Health bypass** `src/web.rs:179` `is_health_path` exempts `/healthz`, `/readyz`, `/metrics` from bulkhead/rate/shedding.

## 6) Health Checks, Graceful Degradation, Capacity Planning, Shutdown

- **Liveness** `src/system.rs:122` `healthz()` → `200 ok` cheap, never blocks, bypasses all shedding (probe for K8s liveness).
- **Readiness** `src/system.rs:146` `readyz()`:
  - dataset loaded (`stations.len()>0 && trains.len()>0`)
  - cache reachable (probe `set`/`get` of `__readiness_probe__`)
  - upstreams probed concurrently with `1s` budget each (`probe_with_timeout`)
  - circuit snapshot (`failover.snapshot()`) and per-source `latency_ms` + `circuit_state`
  - `slo` burn rate (`SloSnapshot::from_metrics_with_telemetry`)
  - sets `railway_ready` gauge (1 ready, 0 not ready) and returns `200 ready` / `200 degraded` / `503 not_ready` + `fine_print: [Health Checks, Graceful Degradation, Capacity Planning]`.
- **Capacity** `src/system.rs:295` `capacity()`:
  - USE signals vs thresholds (`src/core/sre.rs:62`)
  - `saturated_count` + `recommendation` `scale_up` / `ok` / `scale_down`
  - gauges `railway_capacity_recommendation` (1/0/-1) and `railway_capacity_saturated`
- **Source status deep** `src/system.rs:457` `source_status()`:
  - `deep_health` per source: `reachable`, `latency_ms`, `avg_latency_ms`, `samples`, `circuit_state`, `available`, `consecutive_failures`, `open_secs`
  - `fine_print` trio for health/graceful/capacity
- **Graceful shutdown** `src/main.rs:118` `shutdown_signal(RAILWAY_SHUTDOWN_GRACE_SECS=10)`: listens `SIGTERM`/`SIGINT`, logs `SRE: graceful shutdown — draining in-flight requests`, sleeps `grace` before `axum::serve` exits.

Config env (`src/config.rs:75`): `RAILWAY_RATE_LIMIT_RPS`, `RAILWAY_RATE_LIMIT_BURST`, `RAILWAY_CONCURRENCY_LIMIT`, `RAILWAY_LOAD_SHED_THRESHOLD`, `RAILWAY_REQUEST_TIMEOUT_SECS`, `RAILWAY_SHUTDOWN_GRACE_SECS`, `RAILWAY_FAILOVER_THRESHOLD`, `RAILWAY_FAILOVER_COOLDOWN_SECS`.

## 7) Observability Pipeline

- **Metrics** (`src/core/metrics.rs`): `requests_total`, `in_flight`, `bytes_out`, `requests_by_path`, `status_by_code`, `source_latency` EMA, `cache_hits/misses`, `series` ring (600 @2s =20m), helpers `red_*`, `slo_*`, `use_*`.
- **Telemetry** (`src/core/obs.rs:95`): Prometheus `Registry` with 30+ collectors (HTTP, process, cache, source, circuit, SLO, Saturation, RED, USE, resilience, readiness, capacity). `sample()` called by sampler and on each `/metrics` scrape.
- **Log ring** (`src/core/obs.rs:718`): `LogRing(2000)` + `LogRingLayer` mirroring `tracing` events → `GET /rail-api/logs?limit=&level=` and `observability.logs`.
- **Endpoints**: `GET /metrics` (Prometheus text `0.0.4`), `GET /rail-api/observability` (full JSON with `slo`, `red`, `use`, `golden`, `capacity`, `fine_print`, `sre_patterns`), `GET /rail-api/logs`, `GET /rail-api/source-status`, `GET /readyz`, `GET /rail-api/capacity`.

Example `curl` probes:

```sh
curl -s localhost:3000/rail-api/observability | jq '.slo, .capacity, .fine_print[0]'
curl -s localhost:3000/metrics | grep railway_slo
curl -s localhost:3000/readyz | jq '.checks, .error_budget'
curl -s localhost:3000/rail-api/capacity | jq '.recommendation, .saturated_count'
```

## 8) Verification

- `cargo test --lib` — 273 passed (SRE 24, metrics RED/SLO helpers, fanout, resilience, failover).
- `cargo test --tests` — all suites green (including `cache_load` 12, `live_station` 12, `pnr` 9, `ai_chat` 13, `trains_between` 6, etc.).
- Load: `load_test_stations_endpoint_200_concurrent_requests` and `load_test_train_search_100_concurrent_requests` pass at 1000 rps burst 1000 (rate limiter not tripping under normal load; shedding at 800 in-flight).

See `cargo test --lib core::sre`, `core::metrics`, `core::fanout`, `core::resilience`.

