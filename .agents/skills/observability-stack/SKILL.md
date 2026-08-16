---
name: observability-stack
description: Extend or fix railway-rs observability — the GET /rail-api/observability and /rail-api/logs endpoints plus the Observability SPA tab (metrics, gauges, uptime, source status, graphs/tables/stats). Use for "improve observability", "add metrics/gauges", "OpenTelemetry", "logs endpoint", "observability tab", or "monitoring dashboards". All numbers are real runtime metrics; never fabricate.
---

# Observability stack (railway-rs)

Observability is a real-runtime, ready-to-use stack — no external shipper. All
numbers come from live processes: request counters, per-source latency, cache
stats, CPU/mem from `/proc/self/stat*`, and an in-memory structured-log ring.

## Backend surface (`src/slices/observability/`)

- `GET /rail-api/observability` → `ObservabilityResponse`
  (from `service::Service::snapshot(&state)`).
- `GET /rail-api/logs?limit=&level=` → recent structured-log records
  (ring buffer, newest-first, default limit 100 max 500, min-level filter
  `debug|info|warn|error`).

`mod.rs` is the router (thin handlers); `service.rs` builds the snapshot from:
- `state.metrics.snapshot()` — request totals, per-source latency,
  `req_per_sec`, in-flight/active connections, top paths.
- `crate::core::obs::proc_stats()` — CPU from `/proc/self/stat`, mem from
  `/proc/self/statm` (honest 0 fallback when `/proc` is unavailable).
- `crate::core::obs::log_ring` — the structured log ring for `/rail-api/logs`.
- `origin` statuses for the real sources: Railyatri, etrain, NTES, IRCTC.
  Names are the actual live sources — never fake relays.
- `Service::snapshot` computes latency EMA/rolling averages itself when
  needed; `state.metrics.record_source_latency("ntes", elapsed)` feeds it.

## Recording metrics from a slice

In any slice service, after an upstream call:

```rust
let start = std::time::Instant::now();
let data = state.ntes_web.whatever(...).await?;
state.metrics.record_source_latency("ntes", start.elapsed());
```

`core/metrics.rs` holds the counters/EMA series (`SeriesPoint`, per-source
snapshots); extend it there, not inline. Keep every gauge/stat backed by a real
measurement.

## Frontend tab (`static/tabs/observability.js`)

Follows the `spa-tab` conventions. Read `static/tabs/observability.js` and
`tests/observability.rs` for the exact response shape before rendering. It
should render:

- **Status section**: uptime (h/m/s), total requests, cache hits/misses and
  hit-rate `hit/(hit+miss)*100` with a divide-by-zero guard.
- **Source status table**: one row per origin with current status + latency
  from `/rail-api/source-status` / the observability payload.
- **Stats/graphs**: render whatever series the endpoint exposes using the
  table/stat helpers in `ui.js`. Keep it scroll-friendly — long unbounded
  gauges/lists overflow the tab (a recurring UX bug); cap heights and use
  summary cards for aggregates.

## Conventions & gotchas

- **No fabricated numbers anywhere** — UI included. On API error render the
  error string.
- Auto-refresh sparingly; re-fetch on mount and explicit refresh, not an
  infinite polling loop.
- Guard all math (division by zero, empty series, missing keys).
- If the tab shows an infinite layout, the cause is almost always a growing
  unbounded gauge/list — bound it.
- Quality gates and run steps: see `rust-workflow`.
