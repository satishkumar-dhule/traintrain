# Skill: rerun-app

Restart the railway-rs app on `:3000`. Use for "rerun", "restart", "rereun",
"rerum", or any variant requesting the app to be restarted.

## Env required

```bash
export PATH="/home/runner/workspace/.local/share/.cargo/bin:$PATH"
export CARGO_HOME=/home/runner/workspace/.local/share/.cargo
```

## Quick check & restart

```bash
# 1. Kill existing
pkill -x railway-rs 2>/dev/null
sleep 1

# 2. Check binary exists
cd /home/runner/workspace/railway-rs
ls ./target/release/railway-rs 2>/dev/null || NEED_BUILD=1

# 3. If binary missing → check toolchain → restore if needed → build
if [ "$NEED_BUILD" = "1" ]; then
  rustc --version 2>/dev/null || {
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain 1.86.0 --profile minimal
    export PATH="/home/runner/workspace/.local/share/.cargo/bin:$PATH"
    rustup default 1.86.0
  }
  CARGO_HOME=/home/runner/workspace/.local/share/.cargo /nix/store/brzjqpcbk04hzmhsqlmp7vng4jdis2yc-rust-mixed/bin/cargo build --release
fi

# 4. Start
RAILWAY_PORT=3000 setsid nohup ./target/release/railway-rs > /tmp/railway-rs.log 2>&1 < /dev/null & disown
sleep 2
curl -s localhost:3000/healthz
```

## Key details

- **Never use 1.86 for builds** — candle-core requires rustc ≥1.87. Always use
  the nix toolchain at `/nix/store/brzjqpcbk04hzmhsqlmp7vng4jdis2yc-rust-mixed/bin/cargo` for builds.
- Rustup 1.86.0 is installed for clippy/fmt only.
- The sandbox wipes `~/.cargo` but not `~/workspace/`, so toolchain may need
  restoring while source code is fine.

## Logs

- Log file: `/tmp/railway-rs.log`
- Kill: `pkill -x railway-rs` only (not `pkill -f`)

## Response

Keep output to 1 line: `On :3000.` (or `On :3000. Rebuilt.` if a rebuild was needed).
