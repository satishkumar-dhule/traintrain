# railway-rs

RailCompanion - an Indian Railways companion backend (Rust rewrite). Axum JSON API plus a vanilla-JS SPA that serves live railway data straight from public Indian Railways sources - never simulated, never fabricated.

## Features

- Live PNR status, full train schedules, and live train status from Railyatri
- Live trains-at-station, trains-between-stations, and exceptional-train lists (cancelled / rescheduled / diverted) from NTES
- Offline autocomplete over a real 8,958-station and 10,609-train dataset (no network needed for search)
- Runtime observability endpoint with real request/source metrics
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
│   │   ├── pnr/ schedule/ live_status/        Railyatri-backed
│   │   ├── live_station/ trains_between/ exceptional/   NTES-backed
│   │   ├── stations/ search/                  offline dataset lookups
│   │   └── observability/                     runtime metrics snapshot
│   ├── core/
│   │   ├── source.rs          DataSource trait + SourceOutcome
│   │   ├── aggregator.rs      concurrent fan-out, first success wins
│   │   ├── cache.rs           TTL cache (Mutex<HashMap>)
│   │   ├── metrics.rs         request/source latency counters
│   │   ├── railyatri/         Railyatri HTML/JSON extraction
│   │   ├── ntes/              NTES mobile client (crypto) + web-form client
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

The three `*_BASE` variables let tests (or proxies) point sources at local mocks. `HTTP_TIMEOUT` and `CACHE_TTL` are parsed as seconds.

## API endpoints

Unmatched `/rail-api/*` paths return JSON 404; everything else falls through to the SPA.

| Method | Path                     | Params                                   | Description                                          |
| ------ | ------------------------ | ---------------------------------------- | ---------------------------------------------------- |
| GET    | `/healthz`               | -                                        | Liveness probe (`{"status":"ok"}`); also `/api/healthz` |
| GET    | `/rail-api/source-status`| -                                        | Per-source reachability + live-data notice            |
| GET    | `/rail-api/observability`| -                                        | Real runtime metrics (requests, latency, CPU/mem, origins) |
| GET    | `/rail-api/stations`     | `q` (substring, optional)                | Station search over the local dataset, up to 20 hits  |
| GET    | `/rail-api/search/trains`| `q` (query, optional)                    | Train search over the local dataset, up to 10 hits    |
| GET    | `/rail-api/search/stations` | `q` (query, optional)                 | Lite station search, up to 10 hits                    |
| GET    | `/rail-api/pnr`          | `pnr` (10 digits), `captcha_session`, `captcha_text`, `captcha_source` | PNR status (Railyatri). Captcha params echo a prior 428 challenge |
| GET    | `/rail-api/schedule`     | `train` (1-8 digit number)               | Full timetable: stops, run days, route description    |
| GET    | `/rail-api/live-status`  | `train` (1-8 digit number), `date` (YYYY-MM-DD, optional) | Live train position, current location, per-stop status |
| GET    | `/rail-api/ntes/live-station` | `station` (4-char code), `hours` (1-4, default 2) | Trains expected at a station (arrival board)      |
| GET    | `/rail-api/ntes/trains-between` | `src`, `dst` (4-char codes)        | Direct trains between two stations, with running days |
| GET    | `/rail-api/ntes/exceptional` | `type` = `cancelled` \| `rescheduled` \| `diverted` | Exceptional trains of the given kind          |

Each live response carries a `data_source` field naming the actual upstream that produced it (`Railyatri` or `NTES`). Search endpoints are offline; PNR, schedule, live-status, live-station, trains-between and exceptional are live and go through the shared cache + aggregator.

## Static data

`data/stations.json` holds 8,958 real Indian Railway stations (`code`, `name`, `state`, `zone`) and `data/trains.json` holds 10,609 real trains (`number`, `name`) from the NTES master list. They are loaded once at startup into shared `AppState` and used by the `stations` and `search` slices (never by the live slices).

- `scripts/convert_stations.cjs` - converts a GeoJSON stations file into the flat `stations.json` format (drops pseudo `XX-`/`YY-` codes).
- `scripts/fetch_trains.cjs` - fetches the real NTES `train_data.js` master list and writes `trains.json` (real trains only).

## Testing

- `cargo test` - hermetic integration tests in `tests/` plus unit tests in `src/`.
- Integration tests use `tests/common` (`MockServer` + `TestApp`): the real app is spawned bound to a random port, with three mock upstreams (railyatri / etrain / ntes) wired in via the `*_BASE` config, and driven over real HTTP.
- Set `RAILWAY_LIVE_TESTS=1` to run the live-data test suite, which hits the real upstreams and is therefore not hermetic and not run in CI.

## Deployment

- **Docker** - a multi-stage `Dockerfile` at the repo root builds the release binary and runs it as an unprivileged user on port 3000 with a `/healthz` healthcheck. `docker build -t railway-rs .`
- **systemd** - `deploy/railway-rs.service` is a hardened unit (`ProtectSystem=strict`, `NoNewPrivileges`, non-root `railway` user) reading `/etc/railway-rs/railway-rs.env`; see `deploy/README.md` for the full install/upgrade walkthrough.

## Known limitations and data sources

This project is strictly live-data-only: every non-search endpoint fetches from public Indian Railways sources at request time. Nothing is simulated or fabricated.

- **Honest errors** - when an upstream is unreachable or fails, the API returns a real HTTP error (502 bad gateway, 404 not found) with a JSON `{"error": "..."}` body. There is no fallback that invents data.
- **NTES availability** - the NTES mobile and web endpoints are sometimes blocked from datacenter/sandbox networks. In those environments the NTES-backed endpoints (`live-station`, `trains-between`, `exceptional`) return 502; this is reported honestly via `/rail-api/source-status`, which probes each source and lists reachability.
- **Railyatri extraction** - PNR, schedule and live-status parse Railyatri's server-rendered pages/JSON. The SSR payload for live status contains the next station but not per-stop actual times, so per-stop statuses are honestly derived from the real `next_station_code` (`departed` / `expected` / `scheduled`), `actual_arrival` is always empty and `delay_minutes` always 0. The schedule and PNR paths carry real times.
- **CAPTCHA** - upstreams may challenge the client (HTTP 428 with a `captcha_required` body); the PNR endpoint accepts the `captcha_*` echo params to complete the flow.
- **Upstreams** - Railyatri (`railyatri.in`), etrain.info, and NTES (`enquiry.indianrail.gov.in`). All are public, free, and require no API keys. Source base URLs are configurable for testing and proxying.
