# railway-rs

Train Bro - an Indian Railways companion backend (Rust rewrite). Axum JSON API plus a Svelte 5 single-page app that serves live railway data straight from public Indian Railways sources - never simulated, never fabricated.

## Features

- Live PNR status from Railyatri, plus live train status ("Spot Train") and full train schedules from NTES (`enquiry.indianrail.gov.in`), with Railyatri as fallback
- Live trains-at-station and trains-between-stations from NTES, plus per-train exceptional dates (cancelled / rescheduled / diverted calendar, cached 2 hours) from NTES
- Train availability from Paytm Travel (`travel.paytm.com`, no login, no IP geofencing) with per-class status, fare and PNR prediction, falling back to IRCTC; prepared-chart (per-coach berth) data from IRCTC without login (`www.irctc.co.in`)
- Offline autocomplete over a real 8,958-station and 10,609-train dataset (no network needed for search)
- AI assistant (`POST /rail-api/ai/chat`, SSE): tool-calling chat that can execute the live rail endpoints and stream answers/cards; runs on the OpenCode Zen gateway by default or fully in-process with a ~105 MB GGUF micro-model (`RAILWAY_AI_BACKEND=local-first`, see models/README.md), keeping zen as once-per-request fallback
- State-of-the-art observability: a Prometheus `/metrics` endpoint (counters, gauges, histograms) for Grafana/Loki ingestion, structured JSON logs (stdout + rolling daily files) mirrored into a live in-memory log ring, and a real-time dashboard with graphs, gauges, tables, stats and a log stream
- Honest errors: upstream failures surface as HTTP 502/404 with a JSON `{"error": ...}` body - no made-up data
- No API keys, no accounts, no configuration required to run
- TTL response cache and per-source fan-out with first-success-wins aggregation

## Architecture

The app is a single axum process: a top-level router (`src/web.rs`) merges one router per vertical slice, applies shared middleware (metrics, trace, catch-panic, 30s timeout), and serves the SPA from `static/` with an `index.html` fallback for client-side routing. Each vertical slice under `src/slices/` is self-contained (`mod.rs` router + `service.rs` logic). All live data flows through the `DataSource` abstraction in `src/core/source.rs`; the aggregator in `src/core/aggregator.rs` races every registered source concurrently and returns the first success. Responses are cached in a TTL `Cache` and instrumented by a real `Metrics` collector.

The frontend is a **Svelte 5 + Vite + Tailwind** single-page app built from `frontend/` and served as a static bundle from `static/assets/` (the `index.html` entry). It uses pathname-based routing (not hash routing): `/train/12559`, `/station/NDLS`, `/plan/NDLS/BSB/2026-08-20` (plan deep links carry an optional journey date), `/pnr/...`, `/assistant`, `/system`. Live train status is tabbed (`/train/12559/{status,schedule,delay,map,exceptions}`); exceptions (cancelled / rescheduled / diverted dates) live only as the train page's Exceptions tab. The UI is built from shadcn-style primitives under `frontend/src/lib/components/ui/` plus a small set of reusable, enterprise-grade components (`PageHeader`, `StationPairInput`, `ResultMeta`, `StatPill`, `EntityChip`, `Breadcrumbs`, `SourceTrustChip`). The legacy vanilla-JS SPA source (`static/routes.js`, `static/palette.js`, `static/api.js`, `static/ui.js`) is retained only because its pure route-table, palette and fetch helpers are still covered by the `tests/js/` unit suite; it is not served to users.

```
railway-rs/
├── src/
│   ├── web.rs                 top-level router: slices + system + static fallback
│   ├── system.rs              /healthz, /api/healthz, /rail-api/source-status
│   ├── state.rs               AppState: config, http, cache, metrics, datasets
│   ├── models.rs              wire models for every /rail-api/* response
│   ├── slices/                one directory per vertical slice
│   │   ├── pnr/                          Railyatri-backed
│   │   ├── schedule/ live_status/        NTES-backed (Railyatri fallback)
│   │   ├── live_station/ trains_between/ exceptional/   NTES-backed
│   │   ├── availability/                      Paytm-backed (IRCTC fallback)
│   │   ├── chart/                             IRCTC-backed (no login)
│   │   ├── stations/ search/                  offline dataset lookups
│   │   └── observability/                     runtime metrics snapshot
│   ├── core/
│   │   ├── source.rs          DataSource trait + SourceOutcome
│   │   ├── aggregator.rs      concurrent fan-out, first success wins
│   │   ├── cache.rs           TTL cache (Mutex<HashMap>) with hit/miss counters
│   │   ├── metrics.rs         request/source latency counters + time-series ring
│   │   ├── obs.rs             Prometheus registry, proc stats, structured-log ring
│   │   ├── railyatri/         Railyatri HTML/JSON extraction
│   │   ├── ntes/              NTES mobile client (crypto) + web-form client
│   │   ├── irctc/             IRCTC client (Akamai bootstrap + signed requests) + normalizers
│   │   ├── paytm/             Paytm Travel client (public search API) + normalizer
│   │   └── error.rs           AppError -> 400/404/428/500/502
│   └── config.rs              env-var configuration
├── data/                      stations.json, trains.json (real datasets)
├── static/                    served SPA build (Svelte bundle in static/assets/) + legacy JS kept for tests/js
├── frontend/                  Svelte 5 source (components, pages, lib)
├── tests/                     hermetic integration tests (mock upstreams)
├── scripts/                   data generation scripts
└── deploy/                    systemd unit + deployment guide
```

## Quickstart

Requires Rust 1.8x (edition 2021). No configuration, no API keys, no database - just run:

```sh
cargo run
```

Open <http://localhost:3000>. The server binds `0.0.0.0:3000`, loads the real datasets from `data/`, and logs the station/train counts on startup.

## Configuration

Everything is optional; defaults are built in. Read from environment variables at startup (`src/config.rs`).

| Variable                          | Default            | Description                                         |
| --------------------------------- | ------------------ | --------------------------------------------------- |
| `RAILWAY_PORT`                    | `3000`             | TCP port the HTTP server listens on                 |
| `RAILWAY_DATA_DIR`                | `./data`           | Directory containing `stations.json` and `trains.json` |
| `RAILWAY_STATIC_DIR`              | `./static`         | Directory with the SPA static files                 |
| `RAILWAY_HTTP_TIMEOUT`            | `15`               | Outbound upstream HTTP timeout, seconds             |
| `RAILWAY_CACHE_TTL`               | `120`              | TTL for cached upstream responses, seconds          |
| `RAILWAY_USER_AGENT`              | Chrome desktop UA  | `User-Agent` header sent to upstream sources        |
| `RAILWAY_SOURCE_RAILYATRI_BASE`   | `https://www.railyatri.in`     | Base URL of the Railyatri upstream source |
| `RAILWAY_SOURCE_ETRAIN_BASE`      | `https://etrain.info`          | Base URL of the etrain.info upstream source |
| `RAILWAY_SOURCE_NTES_BASE`        | `https://enquiry.indianrail.gov.in` | Base URL of the NTES upstream source    |
| `RAILWAY_SOURCE_IR_BASE`          | `https://www.indianrail.gov.in`     | Base URL of the Indian Railways portal  |
| `RAILWAY_SOURCE_IRCTC_BASE`       | `https://www.irctc.co.in`           | Base URL of the IRCTC upstream source   |
| `RAILWAY_SOURCE_PAYTM_BASE`       | `https://travel.paytm.com`          | Base URL of the Paytm Travel upstream source |
| `RAILWAY_LOG_DIR`                 | `./logs`            | Directory for rolling daily JSON log files   |
| `RAILWAY_LOG_FORMAT`              | `json`              | Console log format: `json` or `pretty`       |
| `RAILWAY_AI_BACKEND`              | `zen`               | AI backend: `zen` (upstream gateway), `local` (in-process GGUF engine), `local-first` (local, zen fallback) |
| `RAILWAY_LOCAL_MODEL_PATH`        | `models/trainbro.gguf` | GGUF weights for the local engine          |
| `RAILWAY_LOCAL_CTX`               | `1024`              | Local context window, tokens                 |
| `RAILWAY_LOCAL_THREADS`           | `0`                 | Local CPU threads (`0` = auto: min(cores,4)) |
| `RAILWAY_LOCAL_MAX_TOKENS`        | `192`               | Generation cap per local round               |

The `*_BASE` variables let tests (or proxies) point sources at local mocks. `HTTP_TIMEOUT` and `CACHE_TTL` are parsed as seconds.

## API endpoints

Unmatched `/rail-api/*` paths return JSON 404; everything else falls through to the SPA.

| Method | Path                     | Params                                   | Description                                          |
| ------ | ------------------------ | ---------------------------------------- | ---------------------------------------------------- |
| GET    | `/healthz`               | -                                        | Liveness probe (`{"status":"ok"}`); also `/api/healthz` |
| GET    | `/metrics`               | -                                        | Prometheus text-format metrics (counters, gauges, histograms) for Grafana/Prometheus scraping |
| GET    | `/rail-api/source-status`| -                                        | Per-source reachability + live-data notice            |
| GET    | `/rail-api/observability`| -                                        | Full runtime snapshot: requests, latency, CPU/mem, cache, status distribution, time-series for charts, recent logs |
| GET    | `/rail-api/logs`         | `limit` (1-500), `level` (`debug`\|`info`\|`warn`\|`error`) | Tail the structured-log ring (newest-first JSON records) |
| GET    | `/rail-api/stations`     | `q` (substring, optional)                | Station search over the local dataset, up to 20 hits  |
| GET    | `/rail-api/search/trains`| `q` (query, optional)                    | Train search by number or name over the local dataset, up to 10 hits |
| GET    | `/rail-api/search/stations` | `q` (query, optional)                 | Station search, up to 10 hits — CoRover (Ask DISHA) first, local dataset fallback |
| GET    | `/rail-api/search/suggest` | `q` (query, optional)                  | Combined station + train IntelliSense autocomplete, up to 10 hits (one round trip) |
| GET    | `/rail-api/pnr`          | `pnr` (10 digits), `captcha_session`, `captcha_text`, `captcha_source` | PNR status (Railyatri). Captcha params echo a prior 428 challenge |
| GET    | `/rail-api/schedule`     | `train` (1-8 digit number)               | Full timetable: stops, run days, route description    |
| GET    | `/rail-api/live-status`  | `train` (1-8 digit number), `date` (YYYY-MM-DD, optional) | Live train position ("Spot Train"), current location, per-stop status, actual arrivals and delay from NTES |
| GET    | `/rail-api/ntes/live-station` | `station` (4-char code), `hours` (1-4, default 2) | Trains expected at a station (arrival board)      |
| GET    | `/rail-api/ntes/trains-between` | `src`, `dst` (4-char codes)        | Direct trains between two stations, with running days |
| GET    | `/rail-api/availability`        | `src`, `dst` (4-char codes), `date` (optional), `source` = `auto` \| `paytm` \| `irctc` | Direct trains with class availability, running days and times; Paytm primary (per-class status, fare and PNR prediction), IRCTC fallback |
| GET    | `/rail-api/irctc/availability`  | same params as `/rail-api/availability` | Legacy alias of `/rail-api/availability` (same handler) |
| GET    | `/rail-api/irctc/chart`         | `train` (1-8 digits), `date` (optional), `station` (4-char code, optional) | Prepared-chart per-coach berth status for a journey date (IRCTC online-charts) |
| GET    | `/rail-api/ntes/exceptional` | `train` (4-5 digit number), `type` = `cancelled` \| `rescheduled` \| `diverted` (optional) | Per-train exceptional dates (cancelled / rescheduled / diverted calendar, cached 2h); the train name is resolved from the local master list when NTES does not echo it, and the NTES verdict `No Exceptional Details found for train X !!!` is echoed verbatim as `message` when there are none |
| GET    | `/rail-api/ai/status`    | -                                        | AI assistant config: enabled, model, keyed, active `backend` and `fallback` |
| POST   | `/rail-api/ai/chat`      | JSON body: `{ "messages": [...] }`       | SSE chat with tool calling; the assistant can execute live rail tools and stream deltas/cards |

Each live response carries a `data_source` field naming the actual upstream that produced it (`Railyatri`, `NTES`, `IRCTC` or `Paytm`). `live-status` ("Spot Train") and `schedule` prefer NTES and fall back to Railyatri when NTES is unreachable; `trains-between` prefers NTES and falls back to the IRCTC availability API; `availability` prefers Paytm (per-class booking status like `GNWL82/WL59` or `AVAILABLE 0022`, fare, quota and PNR prediction - no login, no IP geofencing) and falls back to IRCTC, with `source=paytm`/`irctc` pinning one source and an invalid value rejected as 400. `live-status` drives the NTES "Spot Your Train (Live Status)" web form (POST `/mntes/tr?opt=TrainRunning&subOpt=FindRunningInstancePop`), parses the returned popup's run panes and per-station timeline, and serves the active run's start date plus every reported run instance. Search endpoints are offline; PNR, schedule, live-status, live-station, trains-between, availability, chart and exceptional are live and go through the shared cache.

## Static data

`data/stations.json` holds 8,958 real Indian Railway stations (`code`, `name`, `state`, `zone`) and `data/trains.json` holds 10,609 real trains (`number`, `name`) from the NTES master list. Both lists are **pre-warmed** at startup: every record is normalized once into lowercase indexes in `AppState.datasets`, so the `stations` and `search` slices (and the combined `/rail-api/search/suggest` autocomplete) never re-normalize or refetch the datasets - they are used wherever a station code or train number is needed, with no network involved.

Search is IntelliSense-style: train queries match **numbers and names**, station queries match **codes and names**, and results are ranked exact > prefix > contains (multi-word queries rank all-token matches first). The SPA wires this autocomplete into every relevant input: the header shell search, the Schedule and Spot Train train inputs, and the Live Station / Trains B/W station inputs.

- `scripts/convert_stations.cjs` - converts a GeoJSON stations file into the flat `stations.json` format (drops pseudo `XX-`/`YY-` codes).
- `scripts/fetch_trains.cjs` - fetches the real NTES `train_data.js` master list and writes `trains.json` (real trains only).

## Testing

- `cargo test` - hermetic integration tests in `tests/` plus unit tests in `src/`.
- `make check-js` (or `node --test tests/js/`) - frontend gates: every served `static/*.js` parses (`node --check`), and the pure route-table unit tests (`tests/js/routes.test.mjs`) plus a DOM-smoke suite (`tests/js/dom-smoke.test.mjs`, a fake-DOM boot + hash-navigation harness that loads all real scripts with the network stubbed) pass. Requires Node >= 18. The JS test files live in `tests/js/` so they are never publicly served.
- `make check-ui` (or `npm run test:ui`) - real-browser UI suite (`tests/ui/`): drives the actual SPA on :3000 through headless Chromium, pinning per-route rendering, single-`<h1>` outline, zero horizontal overflow at desktop + phone widths, working vertical scroll, accessible control/button names, honest live-status flow resolution (data or explicit error, never blank), theme persistence, and no uncaught JS exceptions. Reuses a running server or starts one from `target/`; bootstraps its private browser runtime on first use; see `tests/ui/README.md`.
- Integration tests use `tests/common` (`MockServer` + `TestApp`): the real app is spawned bound to a random port, with mock upstreams (railyatri / etrain / ntes / ir / irctc) wired in via the `*_BASE` config, and driven over real HTTP.
- Set `RAILWAY_LIVE_TESTS=1` to run the live-data test suite, which hits the real upstreams and is therefore not hermetic and not run in CI.

## Deployment

- **Docker** - a multi-stage `Dockerfile` at the repo root builds the release binary and runs it as an unprivileged user on port 3000 with a `/healthz` healthcheck. `docker build -t railway-rs .`
- **systemd** - `deploy/railway-rs.service` is a hardened unit (`ProtectSystem=strict`, `NoNewPrivileges`, non-root `railway` user) reading `/etc/railway-rs/railway-rs.env`; see `deploy/README.md` for the full install/upgrade walkthrough.

## Known limitations and data sources

This project is strictly live-data-only: every non-search endpoint fetches from public Indian Railways sources at request time. Nothing is simulated or fabricated.

- **Honest errors** - when an upstream is unreachable or fails, the API returns a real HTTP error (502 bad gateway, 404 not found) with a JSON `{"error": "..."}` body. There is no fallback that invents data.
- **NTES availability** - the NTES mobile JSON API (`/crisns/AppServAnd`) is blocked by Akamai from many datacenter/sandbox networks (it answers an empty `200 OK`), so the NTES-backed endpoints go through the public **web forms** instead (`/mntes/` bootstrap -> `GetCSRFToken` -> form POST to `/mntes/q` for the live boards and `/mntes/tr` for Spot Train), parsing the returned HTML tables and the "Spot Your Train" popup. When the web forms are also unreachable those endpoints return 502, or fall back to Railyatri/IRCTC where a fallback exists; this is reported honestly via `/rail-api/source-status`, which probes each source and lists reachability.
- **IRCTC geo-fencing** - IRCTC (`www.irctc.co.in`) is Akamai-protected and IP-geofenced to India: datacenter / foreign IPs get HTTP 403. The chart slice reports that honestly as a 502; the availability slice only reaches IRCTC when `source=irctc` is pinned or Paytm (its default primary, not geofenced) fails first, and returns a 502 naming both sources when neither answers. The client mirrors a browser session (harvests the `TS018d84e5` Akamai cookie, sends `Greq` / `Referer` / `Origin`), but nothing bypasses the geo-block. The exact `trainComposition` response envelope is undocumented (reconstructed from the online-charts UI), so the chart normalizer is tolerant and fails honestly on unrecognized shapes.
- **Source precedence** - live-status and schedule prefer NTES (Spot Train via the `/mntes/tr` web form, `GetTrainSchedule` from `enquiry.indianrail.gov.in`) and fall back to Railyatri only when NTES is unreachable; PNR comes from Railyatri. NTES provides real per-stop `actual_arrival` values and run-pane positions, surfaced verbatim, with `delay_minutes` taken from the NTES delay badge when present and otherwise derived only from those real actuals when the delta is unambiguous. Only in the Railyatri fallback are per-stop statuses derived from the real `next_station_code` (`departed` / `expected` / `scheduled`) with `actual_arrival` empty and `delay_minutes` 0, because that SSR payload carries no actual times.
- **CAPTCHA** - upstreams may challenge the client (HTTP 428 with a `captcha_required` body); the PNR endpoint accepts the `captcha_*` echo params to complete the flow.
- **Upstreams** - Railyatri (`railyatri.in`), etrain.info, NTES (`enquiry.indianrail.gov.in`), IRCTC (`www.irctc.co.in`), and Paytm Travel (`travel.paytm.com`). All are public, free, and require no API keys or logins. Source base URLs are configurable for testing and proxying.
