---
name: restore-rust-toolchain
description: Reinstall the Rust toolchain for the railway-rs project after the sandbox home directory is wiped. Use when cargo/rustc/rustfmt/clippy are "command not found", when ~/.cargo or ~/.rustup are missing, or when rustc crashes at startup with "cannot allocate memory in static TLS block".
---

# Restore Rust Toolchain (railway-rs)

The Replit-style sandbox periodically wipes the home directory. Everything under
`/home/runner` that is rust-related vanishes: `~/.cargo`, `~/.rustup`, and the
old `~/.local/rustbin`. The **workspace survives** (`/home/runner/workspace`,
including `railway-rs/` source, `target/`, and this skill), so all work is
recoverable — only the compiler needs reinstalling.

## CRITICAL: pin to 1.86.0

Latest stable rustc (1.97.x) **crashes in this sandbox at load time**:

```
rustc: error while loading shared libraries:
.../librustc_driver-...so: cannot allocate memory in static TLS block
```

This is a loader/sandbox restriction (LD_PRELOAD, `GLIBC_TUNABLES`, and other
known workarounds all fail). **rustc 1.86.0 works fine** and matches the
project's original toolchain. Always default to 1.86.0.

## Env needed for every cargo command

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME="$HOME/.cargo"
```

Without these, `cargo` is "command not found" even after installation.

## Full restore (idempotent, ~2-4 min)

```bash
# 1. Install rustup pinned to 1.86.0 (skips if cargo is already present)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain 1.86.0 --profile minimal

export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME="$HOME/.cargo"

# 2. Add clippy + rustfmt to 1.86.0 and make it the default
rustup component add clippy rustfmt --toolchain 1.86.0
rustup default 1.86.0

# 3. Verify - rustc MUST be 1.86.0, and all four must print versions
rustc --version && cargo --version && rustfmt --version && cargo clippy --version
```

If rustup is already installed but the toolchain was lost: skip step 1, just run
steps 2-3.

## Rebuild the project

The `~/.cargo/registry` cache is also wiped, so the first build re-downloads
all dependencies (~1 min).

```bash
cd /home/runner/workspace/railway-rs
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME="$HOME/.cargo"
cargo build            # debug (~60s)
cargo test             # all suites (hermetic, no network)
cargo build --release  # deployable binary (~75s)
```

Quality gates (must all pass after any change):

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Run the app on :3000

```bash
cd /home/runner/workspace/railway-rs
pkill -x railway-rs    # pkill -f self-matches the shell and kills it too
sleep 1
RAILWAY_PORT=3000 setsid nohup ./target/release/railway-rs > /tmp/railway-rs.log 2>&1 < /dev/null & disown
```

Verify: `curl -s localhost:3000/healthz` → `{"status":"ok","service":"railway-rs","runtime":"rust/axum"}`.
Log: `/tmp/railway-rs.log`. Kill with `pkill -x railway-rs` only.

## Notes

- If only cargo exists but rustc/rustfmt/clippy were lost, `rustup default 1.86.0`
  and `rustup component add clippy rustfmt --toolchain 1.86.0` restore them.
- The gov-first architecture (NTES primary, Railyatri fallback) is deployed in
  the running binary; from the sandbox NTES returns empty responses so the app
  honestly falls back to Railyatri and reports `data_source: "Railyatri"`.
- No rust toolchain is in `/nix/store` anymore (old nix profile was GC'd);
  do not rely on nix paths for rustfmt/clippy.
