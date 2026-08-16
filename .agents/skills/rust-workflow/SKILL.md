---
name: rust-workflow
description: Build, test, lint and run the railway-rs Rust app (axum). Use for "restart app", "run tests", "cargo build", "start the rust app", checking quality gates (fmt/clippy/test), or when running any cargo command in this project. Covers the required toolchain env vars, the three quality gates, and the :3000 run/restart/healthcheck procedure.
---

# railway-rs Rust workflow

The app lives in `/home/runner/workspace/railway-rs` (axum backend + vanilla-JS
SPA). All cargo work happens there. There is no `npm` step for the Rust side;
the SPA is plain static files served by the binary.

## Env required before EVERY cargo command

The sandbox home is frequently wiped. Always set these (the restore skill
`restore-rust-toolchain` reinstalls the toolchain when cargo is missing):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME="$HOME/.cargo"
```

Without them `cargo` is "command not found" even when installed. rustc is
pinned to **1.86.0** (newer rustc crashes in this sandbox).

## Quality gates (all must pass after any change)

```bash
cd /home/runner/workspace/railway-rs
export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$HOME/.cargo"
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- `cargo test` runs the full hermetic integration suite (mocked upstreams, no
  network) plus unit tests.
- When working in a parallel-agent orchestration, agents must NOT run the full
  suite concurrently (shared `target/` contends). The orchestrator runs
  `cargo fmt --all`, `cargo test --lib`, `cargo clippy --all-targets -- -D warnings`
  centrally after slices land. Slices run `cargo fmt --all` only.

## Build

```bash
cd /home/runner/workspace/railway-rs
export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$HOME/.cargo"
cargo build            # debug
cargo build --release  # deployable binary
```

## Run on :3000 and restart

```bash
cd /home/runner/workspace/railway-rs
pkill -x railway-rs        # pkill -f self-matches the shell and kills it too
sleep 1
RAILWAY_PORT=3000 setsid nohup ./target/release/railway-rs > /tmp/railway-rs.log 2>&1 < /dev/null & disown
```

- Healthcheck: `curl -s localhost:3000/healthz` →
  `{"status":"ok","service":"railway-rs","runtime":"rust/axum"}`.
- Logs: `/tmp/railway-rs.log`. Kill with `pkill -x railway-rs` only.
- If only a debug binary exists use `./target/debug/railway-rs`.
- Smoke test a real endpoint, e.g.
  `curl -s 'localhost:3000/rail-api/ntes/trains-between?src=NDLS&dst=DLI'`.
  NTES is not reachable from the sandbox, so expect an honest
  `data_source: "Railyatri"` fallback or a source-unavailable error — the
  app reports the real source, never fabricated numbers.

## Conventions

- Live data only. Never fabricate or mock values in the running app.
- Every slice DTO carries `data_source: Option<String>` naming the actual
  upstream that answered (NTES primary, IRCTC/Railyatri fallback).
