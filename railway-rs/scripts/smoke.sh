#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)
PROJECT_ROOT="$SCRIPT_DIR/.."

if command -v cargo >/dev/null 2>&1; then
    CARGO_BIN=cargo
else
    CARGO_BIN=/home/runner/.local/rustbin/cargo
    if [ ! -x "$CARGO_BIN" ]; then
        echo "error: cargo not found on PATH nor at /home/runner/.local/rustbin/cargo" >&2
        exit 1
    fi
fi
export CARGO_HOME="${CARGO_HOME:-/home/runner/.cargo}"

"$CARGO_BIN" build --manifest-path "$PROJECT_ROOT/Cargo.toml"

PORT="${PORT:-3457}"
LOG_FILE=$(mktemp)

cleanup() {
    if [ -n "${SERVER_PID:-}" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$LOG_FILE"
}
trap cleanup EXIT

(
    cd "$PROJECT_ROOT" || exit 1
    exec env RAILWAY_PORT="$PORT" ./target/debug/railway-rs
) >>"$LOG_FILE" 2>&1 &
SERVER_PID=$!

BASE="http://127.0.0.1:$PORT"
UP=0
for _ in $(seq 1 30); do
    if [ "$(curl -s -o /dev/null -w '%{http_code}' "$BASE/healthz" 2>/dev/null || true)" = "200" ]; then
        UP=1
        break
    fi
    sleep 1
done

if [ "$UP" -ne 1 ]; then
    echo "error: server did not become ready within 30s; last 30 lines of log:" >&2
    tail -n 30 "$LOG_FILE" >&2
    exit 1
fi

FAILED=0

probe_status() {
    local name="$1" url="$2" code
    code=$(curl -s -o /dev/null -w '%{http_code}' "$url" || true)
    if [[ "$code" =~ ^2[0-9][0-9]$ ]]; then
        echo "PASS $name -> $code"
    else
        echo "FAIL $name -> $code"
        FAILED=1
    fi
}

is_json() {
    local f="$1"
    if command -v python3 >/dev/null 2>&1; then
        python3 -c 'import json,sys; json.load(sys.stdin)' <"$f" 2>/dev/null
    elif command -v jq >/dev/null 2>&1; then
        jq -e . "$f" >/dev/null 2>&1
    else
        grep -q '"error"' "$f"
    fi
}

probe_status "/healthz" "$BASE/healthz"
probe_status "/rail-api/source-status" "$BASE/rail-api/source-status"
probe_status "/rail-api/observability" "$BASE/rail-api/observability"
probe_status "/rail-api/stations?q=NDLS" "$BASE/rail-api/stations?q=NDLS"
probe_status "/" "$BASE/"

LIVE_URL="$BASE/rail-api/live-status?train=12951"
LIVE_BODY=$(mktemp)
LIVE_CODE=$(curl -s -o "$LIVE_BODY" -w '%{http_code}' "$LIVE_URL" || true)
LIVE_OK=0
case "$LIVE_CODE" in
    200|502|404) LIVE_OK=1 ;;
esac
if [ "$LIVE_OK" -eq 1 ] && [ -s "$LIVE_BODY" ] && is_json "$LIVE_BODY"; then
    echo "PASS $LIVE_URL -> $LIVE_CODE"
else
    echo "FAIL $LIVE_URL -> $LIVE_CODE"
    FAILED=1
fi
rm -f "$LIVE_BODY"

if [ "$FAILED" -eq 0 ]; then
    echo "SMOKE OK"
    exit 0
fi
exit 1
