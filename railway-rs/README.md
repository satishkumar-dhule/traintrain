# railway-rs

RailCompanion - an Indian Railways companion backend (Rust rewrite). Axum JSON API plus a vanilla-JS SPA that serves live railway data straight from public Indian Railways sources - never simulated, never fabricated.

## Features

- Live PNR status from Railyatri, plus live train status ("Spot Train") and full train schedules from NTES (`enquiry.indianrail.gov.in`), with Railyatri as fallback
- Live trains-at-station, trains-between-stations, and exceptional-train lists (cancelled / rescheduled / diverted) from NTES
- Train availability and prepared-chart (per-coach berth) data from IRCTC without login (`www.irctc.co.in`)
- Offline autocomplete over a real 8,958-station and 10,609-train dataset (no network needed for search)
- State-of-the-art observability: a Prometheus `/metrics` endpoint (counters, gauges, histograms) for Grafana/Loki ingestion, structured JSON logs (stdout + rolling daily files) mirrored into a live in-memory log ring, and a real-time dashboard with graphs, gauges, tables, stats and a log stream
- Honest errors: upstream failures surface as HTTP 502/404 with a JSON `{"error": ...}` body - no made-up data
- No API keys, no accounts, no configuration required to run
- TTL response cache and per-source fan-out with first-success-wins aggregation

## Architecture

The app is a single axum process: a top-level router (`src/web.rs`) merges one router per vertical slice, applies shared middleware (metrics, trace, catch-panic, 30s timeout), and serves the SPA from `static/` with an `index.html` fallback for client-side routing. Each vertical slice under `src/slices/` is self-contained (`mod.rs` router + `service.rs` logic). All live data flows through the `DataSource` abstraction in `src/core/source.rs`; the aggregator in `src/core/aggregator.rs` races every registered source concurrently and returns the first success. Responses are cached in a TTL `Cache` and instrumented by a real `Metrics` collector.

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
│   │   ├── availability/ chart/               IRCTC-backed (no login)
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
│   │   └── error.rs           AppError -> 400/404/428/500/502
│   └── config.rs              env-var configuration
├── data/                      stations.json, trains.json (real datasets)
├── static/                    vanilla-JS SPA
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
| `RAILWAY_LOG_DIR`                 | `./logs`            | Directory for rolling daily JSON log files   |
| `RAILWAY_LOG_FORMAT`              | `json`              | Console log format: `json` or `pretty`       |

The three `*_BASE` variables let tests (or proxies) point sources at local mocks. `HTTP_TIMEOUT` and `CACHE_TTL` are parsed as seconds.

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
| GET    | `/rail-api/search/stations` | `q` (query, optional)                 | Lite station search, up to 10 hits                    |
| GET    | `/rail-api/search/suggest` | `q` (query, optional)                  | Combined station + train IntelliSense autocomplete, up to 10 hits (one round trip) |
| GET    | `/rail-api/pnr`          | `pnr` (10 digits), `captcha_session`, `captcha_text`, `captcha_source` | PNR status (Railyatri). Captcha params echo a prior 428 challenge |
| GET    | `/rail-api/schedule`     | `train` (1-8 digit number)               | Full timetable: stops, run days, route description    |
| GET    | `/rail-api/live-status`  | `train` (1-8 digit number), `date` (YYYY-MM-DD, optional) | Live train position ("Spot Train"), current location, per-stop status, actual arrivals and delay from NTES |
| GET    | `/rail-api/ntes/live-station` | `station` (4-char code), `hours` (1-4, default 2) | Trains expected at a station (arrival board)      |
| GET    | `/rail-api/ntes/trains-between` | `src`, `dst` (4-char codes)        | Direct trains between two stations, with running days |
| GET    | `/rail-api/irctc/availability`  | `src`, `dst` (4-char codes), `date` (optional) | Direct trains with class availability, running days and times (IRCTC, no login) |
| GET    | `/rail-api/irctc/chart`         | `train` (1-8 digits), `date` (optional), `station` (4-char code, optional) | Prepared-chart per-coach berth status for a journey date (IRCTC online-charts) |
| GET    | `/rail-api/ntes/exceptional` | `type` = `cancelled` \| `rescheduled` \| `diverted` | Exceptional trains of the given kind          |

Each live response carries a `data_source` field naming the actual upstream that produced it (`Railyatri`, `NTES` or `IRCTC`). `live-status` ("Spot Train") and `schedule` prefer NTES and fall back to Railyatri when NTES is unreachable; `trains-between` prefers NTES and falls back to the IRCTC availability API. `live-status` drives the NTES "Spot Your Train (Live Status)" web form (POST `/mntes/tr?opt=TrainRunning&subOpt=FindRunningInstancePop`), parses the returned popup's run panes and per-station timeline, and serves the active run's start date plus every reported run instance. Search endpoints are offline; PNR, schedule, live-status, live-station, trains-between, availability, chart and exceptional are live and go through the shared cache.

## Static data

`data/stations.json` holds 8,958 real Indian Railway stations (`code`, `name`, `state`, `zone`) and `data/trains.json` holds 10,609 real trains (`number`, `name`) from the NTES master list. Both lists are **pre-warmed** at startup: every record is normalized once into lowercase indexes in `AppState.datasets`, so the `stations` and `search` slices (and the combined `/rail-api/search/suggest` autocomplete) never re-normalize or refetch the datasets - they are used wherever a station code or train number is needed, with no network involved.

Search is IntelliSense-style: train queries match **numbers and names**, station queries match **codes and names**, and results are ranked exact > prefix > contains (multi-word queries rank all-token matches first). The SPA wires this autocomplete into every relevant input: the header shell search, the Schedule and Spot Train train inputs, and the Live Station / Trains B/W station inputs.

- `scripts/convert_stations.cjs` - converts a GeoJSON stations file into the flat `stations.json` format (drops pseudo `XX-`/`YY-` codes).
- `scripts/fetch_trains.cjs` - fetches the real NTES `train_data.js` master list and writes `trains.json` (real trains only).

## Testing

- `cargo test` - hermetic integration tests in `tests/` plus unit tests in `src/`.
- Integration tests use `tests/common` (`MockServer` + `TestApp`): the real app is spawned bound to a random port, with mock upstreams (railyatri / etrain / ntes / ir / irctc) wired in via the `*_BASE` config, and driven over real HTTP.
- Set `RAILWAY_LIVE_TESTS=1` to run the live-data test suite, which hits the real upstreams and is therefore not hermetic and not run in CI.

## Deployment

- **Docker** - a multi-stage `Dockerfile` at the repo root builds the release binary and runs it as an unprivileged user on port 3000 with a `/healthz` healthcheck. `docker build -t railway-rs .`
- **systemd** - `deploy/railway-rs.service` is a hardened unit (`ProtectSystem=strict`, `NoNewPrivileges`, non-root `railway` user) reading `/etc/railway-rs/railway-rs.env`; see `deploy/README.md` for the full install/upgrade walkthrough.

## Known limitations and data sources

This project is strictly live-data-only: every non-search endpoint fetches from public Indian Railways sources at request time. Nothing is simulated or fabricated.

- **Honest errors** - when an upstream is unreachable or fails, the API returns a real HTTP error (502 bad gateway, 404 not found) with a JSON `{"error": "..."}` body. There is no fallback that invents data.
- **NTES availability** - the NTES mobile JSON API (`/crisns/AppServAnd`) is blocked by Akamai from many datacenter/sandbox networks (it answers an empty `200 OK`), so the NTES-backed endpoints go through the public **web forms** instead (`/mntes/` bootstrap -> `GetCSRFToken` -> form POST to `/mntes/q` for the live boards and `/mntes/tr` for Spot Train), parsing the returned HTML tables and the "Spot Your Train" popup. When the web forms are also unreachable those endpoints return 502, or fall back to Railyatri/IRCTC where a fallback exists; this is reported honestly via `/rail-api/source-status`, which probes each source and lists reachability.
- **IRCTC geo-fencing** - IRCTC (`www.irctc.co.in`) is Akamai-protected and IP-geofenced to India: datacenter / foreign IPs get HTTP 403. The availability and chart slices report that honestly as a 502. The client mirrors a browser session (harvests the `TS018d84e5` Akamai cookie, sends `Greq` / `Referer` / `Origin`), but nothing bypasses the geo-block. The exact `trainComposition` response envelope is undocumented (reconstructed from the online-charts UI), so the chart normalizer is tolerant and fails honestly on unrecognized shapes.
- **Source precedence** - live-status and schedule prefer NTES (Spot Train via the `/mntes/tr` web form, `GetTrainSchedule` from `enquiry.indianrail.gov.in`) and fall back to Railyatri only when NTES is unreachable; PNR comes from Railyatri. NTES provides real per-stop `actual_arrival` values and run-pane positions, surfaced verbatim, with `delay_minutes` taken from the NTES delay badge when present and otherwise derived only from those real actuals when the delta is unambiguous. Only in the Railyatri fallback are per-stop statuses derived from the real `next_station_code` (`departed` / `expected` / `scheduled`) with `actual_arrival` empty and `delay_minutes` 0, because that SSR payload carries no actual times.
- **CAPTCHA** - upstreams may challenge the client (HTTP 428 with a `captcha_required` body); the PNR endpoint accepts the `captcha_*` echo params to complete the flow.
- **Upstreams** - Railyatri (`railyatri.in`), etrain.info, NTES (`enquiry.indianrail.gov.in`), and IRCTC (`www.irctc.co.in`). All are public, free, and require no API keys or logins. Source base URLs are configurable for testing and proxying.
