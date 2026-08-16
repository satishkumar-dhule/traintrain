---
name: vertical-slice-rust
description: Implement or extend a vertical slice in railway-rs (src/slices/<name>/). Use for "add X feature", "build a slice", "mirror NTES feature", "implement spot/live station/schedule/pnr search", or any feature following the Phase A (deep module + models + plumbing) then Phase B (service + test + SPA tab) workflow. Covers file ownership, reference slices, endpoint validation, cache + data_source pattern, and quality gates.
---

# Vertical slice in railway-rs

Features are delivered as **vertical slices**, one per NTES feature. A slice is
one self-contained unit: HTTP endpoint + validation in `mod.rs`, logic in
`service.rs`, a hermetic integration test, and a SPA tab. The codebase already
contains complete reference slices — copy their structure.

## Two-phase workflow (parallel-agent friendly)

- **Phase A (core agent):** extends the deep NTES module
  (`src/core/ntes/web.rs`) with a client method + HTML parser, adds models to
  `src/models.rs`, wires plumbing (`src/web.rs`, `src/state.rs`,
  `src/slices/mod.rs`), and creates slice stubs. Runs:
  `cargo fmt --all && cargo test --lib && cargo clippy --all-targets -- -D warnings`.
- **Phase B (one agent per slice):** fills in the slice only, in parallel.
  Runs `cargo fmt --all` only (no concurrent `cargo build/test/clippy` — the
  orchestrator runs the full suite centrally afterwards).

## File ownership

Phase B agents own EXACTLY these four files:

1. `src/slices/<name>/mod.rs` — router + query validation
2. `src/slices/<name>/service.rs` — logic (cache + source-latency + fallback)
3. `tests/<name>.rs` — integration suite against mocks
4. `static/tabs/<name>.js` — SPA tab

NEVER touch: `src/core/ntes/web.rs`, `src/models.rs`, `src/slices/mod.rs`,
`src/web.rs`, `src/state.rs`, `tests/common/mod.rs`, `static/api.js`,
`static/app.js`, `static/index.html`, `static/ui.js`, `Cargo.toml`, or other
slices' files.

## Read these references FIRST

- `src/slices/trains_between/mod.rs` — router + validation pattern
- `src/slices/trains_between/service.rs` — cache + source-latency + fallback
- `tests/trains_between.rs` — integration test pattern
- `static/tabs/trains_between.js` — tab UI pattern
- `src/slices/station_codes.rs` — `normalize_code` / `require_station` helpers
- `src/core/error.rs` — `AppError` constructors (`bad_request`,
  `source_unavailable`, `internal`)

## mod.rs pattern (endpoint + validation)

```rust
pub mod service;

#[derive(Deserialize, Default)]
struct XQuery { train: Option<String> }

pub fn router() -> Router<AppState> {
    Router::new().route("/rail-api/ntes/<name>", get(handler))
}

async fn handler(State(state): State<AppState>, Query(q): Query<XQuery>) -> Result<Json<XResponse>, AppError> {
    // validate: required params -> AppError::bad_request; normalize codes
    // (uppercase 4-char station codes) via normalize_code/require_station;
    // train numbers must be 5 ASCII digits (not all-zero).
    Ok(Json(service::Service::get_x(&state, &param).await?))
}
```

## service.rs pattern

`pub struct Service; impl Service { pub async fn get_x(state: &AppState, ...) -> Result<XResponse, AppError> }`.

- **Cache first**: `state.cache.get(&cache_key)` / `set(&cache_key, value)` on
  the final DTO, so cache hits work regardless of which source produced it.
- **Source latency**: time the NTES call with `std::time::Instant`, then
  `state.metrics.record_source_latency("ntes", elapsed)`.
- **data_source**: always set `data_source: Some("ntes")` (or the fallback
  source's name) — the app reports the real answering source, never fabricated.
- **Fallback**: on NTES failure, fall back to IRCTC/Railyatri and record the
  original failure string in the error. `today_ist()` (UTC+05:30) is the
  booking/live date.
- **tracing**: `tracing::info!`/`tracing::warn!` with structured fields
  (`%train`, `source = "NTES"`, `latency_ms = ...`).

## Integration test pattern (tests/<name>.rs)

Use the shared harness `tests/common/mod.rs`:

- `TestApp::spawn().await` boots the real app with mocked upstreams;
  `app.get(path) -> (StatusCode, Value)`, `app.mocks.get("ntes")`.
- `ntes_web(html)` wires `/mntes/`, `/mntes/GetCSRFToken`, `/mntes/q`.
- `MockServer::route_html_seq(path_prefix, Vec<String>)` serves one HTML per
  matching request in arrival order — REQUIRED when a flow POSTs to the same
  path more than once (e.g. route + spot both post to `/mntes/TrnMap`).
- Assert happy path, validation failures (400), and source-unavailable
  fallback. Check `app.mock("ntes").calls()` to assert exact upstream bodies.

## Validation rules seen in the codebase

- Station codes: 4-char alphanumeric, trimmed/uppercased; must be a known
  station in `state.datasets.stations` (a code token inside an official train
  name is accepted). Reject `src == dst` for between-station forms.
- Train numbers: `len == 5`, all ASCII digits, not "00000".
- Dates: `DD-MMM-YYYY` for NTES forms (e.g. "17-Aug-2026"); today-IST default.
- PNR: exactly 10 digits. Station code in arrival boards: 4 chars.

## Quality gates

After the slice lands (orchestrator runs centrally):
`cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`. See `rust-workflow` skill for the env vars.
