#!/usr/bin/env bash
# Kaizen daemon — autonomous continuous improvement loop.
# Always innovates: every cycle discovers a new 1% win (deterministic + LLM research)
# and either commits it or surfaces the next manual innovation.
#
# Architecture: single-instance via flock, hook.sh starts it on login,
# install.sh materializes the ~/.bashrc hook. Zero extra deps beyond bash/curl/node/cargo.
#
# Usage: daemon.sh [--loop|--once] [--dry-run]
set -uo pipefail

BASE="/home/runner/workspace/.kaizen"
CONFIG="$BASE/config.env"
LOG_FILE="$BASE/logs/daemon.log"
STATUS_FILE="$BASE/status.json"
LOCK_FILE="$BASE/lock"
PID_FILE="$BASE/pid"
PAUSE_FILE="$BASE/PAUSE"
HEARTBEAT="$BASE/HEARTBEAT"

ROOT="/home/runner/workspace"
KAIZEN_RUN="$ROOT/.agents/skills/kaizen/scripts/run.sh"
KAIZEN_LEDGER="$ROOT/.agents/skills/kaizen/ledger.json"
MIRROR="$ROOT/railway-rs/static/data/kaizen.json"

MODE="loop"
DRY_RUN=0
for arg in "$@"; do
  case "$arg" in
    --loop) MODE="loop" ;;
    --once) MODE="once" ;;
    --dry-run) DRY_RUN=1 ;;
    *) echo "usage: $0 [--loop|--once] [--dry-run]" >&2; exit 2 ;;
  esac
done

# Defaults (config.env may override)
CYCLE_SECS=3600
FORCE_RESEARCH=1
[ -r "$CONFIG" ] && . "$CONFIG"

RUNNING=1
trap 'RUNNING=0' TERM INT

# Single instance via flock
exec 9>>"$LOCK_FILE"
if ! flock -n 9; then
  echo "$(date -Is) kaizen daemon: another instance holds the lock; exiting" >&2
  exit 0
fi
echo $$ >"$PID_FILE"

mkdir -p "$BASE/logs"
log() { echo "$(date -Is) $*" >>"$LOG_FILE"; }
rotate_log_if_big() {
  local max=$((4 * 1024 * 1024))
  [ -f "$LOG_FILE" ] || return 0
  [ "$(stat -c%s "$LOG_FILE" 2>/dev/null || echo 0)" -lt "$max" ] && return 0
  local stamp; stamp="$(date +%Y%m%d%H%M%S)"
  gzip -c "$LOG_FILE" >"$LOG_FILE.$stamp.gz" && : >"$LOG_FILE"
  ls -1t "$LOG_FILE".*.gz 2>/dev/null | tail -n +4 | xargs -r rm -f 2>/dev/null || true
}

write_status() {
  # args: cycle, duration_s, last_rc, last_pick, last_delta
  local cycle="$1" dur="$2" rc="$3" pick="$4" delta="$5"
  local now; now="$(date -Is)"
  local runs; runs=$(node -e "try{const j=require('$KAIZEN_LEDGER');console.log((j.runs||[]).length)}catch(e){console.log(0)}" 2>/dev/null || echo 0)
  local last_commit; last_commit=$(node -e "try{const j=require('$KAIZEN_LEDGER');const r=(j.runs||[]).slice(-1)[0];console.log(r?r.commit||'':'' )}catch(e){console.log('')}" 2>/dev/null || echo "")
  local innovations; innovations=$(node -e "try{const j=require('$KAIZEN_LEDGER');const v=new Set((j.runs||[]).map(r=>r.pick));console.log(v.size)}catch(e){console.log(0)}" 2>/dev/null || echo 0)
  cat >"$STATUS_FILE.tmp" <<JSON
{
  "ts": "$now",
  "cycle": $cycle,
  "duration_s": $dur,
  "last_rc": $rc,
  "last_pick": "$pick",
  "last_delta_pct": "${delta:-0}",
  "last_commit": "$last_commit",
  "runs_total": $runs,
  "innovations_unique": $innovations,
  "next_cycle_secs": $CYCLE_SECS,
  "dry_run": $DRY_RUN
}
JSON
  mv "$STATUS_FILE.tmp" "$STATUS_FILE"
  # also mirror a public copy for the /kaizen page (best-effort)
  mkdir -p "$(dirname "$ROOT/railway-rs/static/data/kaizen-status.json")" 2>/dev/null || true
  cp "$STATUS_FILE" "$ROOT/railway-rs/static/data/kaizen-status.json" 2>/dev/null || true
}

run_cycle() {
  local t0=$SECONDS cycle_no=$1
  rotate_log_if_big
  if [ -f "$PAUSE_FILE" ]; then
    log "cycle $cycle_no skipped: PAUSE present"
    printf "%s PAUSED\n" "$(date +%H:%M:%S)" >>"$HEARTBEAT" 2>/dev/null || true
    write_status "$cycle_no" 0 2 "paused" 0
    return 0
  fi
  log "=== kaizen cycle $cycle_no start (mode=$MODE dry_run=$DRY_RUN) ==="
  if [ -f "$HEARTBEAT" ] && [ "$(wc -l <"$HEARTBEAT" 2>/dev/null || echo 0)" -gt 500 ]; then
    tail -n 100 "$HEARTBEAT" >"$HEARTBEAT.tmp" 2>/dev/null && mv "$HEARTBEAT.tmp" "$HEARTBEAT" 2>/dev/null || true
  fi
  printf "%s cycle %-3s starting...\n" "$(date +%H:%M:%S)" "$cycle_no" >>"$HEARTBEAT" 2>/dev/null || true

  local rc=0 pick="" delta=""
  local out_file; out_file=$(mktemp /tmp/kaizen-cycle.XXXXXX)

  if [ "$DRY_RUN" -eq 1 ]; then
    log "dry-run: would run $KAIZEN_RUN --research"
    echo "dry-run" >"$out_file"
    rc=2
  else
    # Always innovates: --research is auto-enabled when provider exists (see run.sh);
    # explicit --research ensures the daemon never degrades to deterministic-only.
    set +e
    bash "$KAIZEN_RUN" --research >"$out_file" 2>&1
    rc=$?
    set -e
    # scrape pick/delta for status — for rc=0 prefer the real committed delta
    pick=$(grep -m1 "^pick:" "$out_file" 2>/dev/null | sed -E 's/^pick: ([^ ]+).*/\1/' || echo "")
    delta=$(grep -m1 "^pick:" "$out_file" 2>/dev/null | sed -E 's/.*Δ ([^%]+)%.*/\1/' || echo "")
    if [ "$rc" -eq 0 ]; then
      c_pick=$(grep -m1 "^committed:" "$out_file" 2>/dev/null | awk '{print $4}' || echo "")
      c_delta=$(grep -m1 "^committed:" "$out_file" 2>/dev/null | awk '{print $5}' | tr -d '%' || echo "")
      if [ -n "$c_pick" ]; then pick="$c_pick"; fi
      if [ -n "$c_delta" ]; then delta="$c_delta"; fi
    fi
    # keep last 80 lines in log
    tail -n 80 "$out_file" >>"$LOG_FILE" 2>/dev/null || true
    # heartbeat line
    if [ "$rc" -eq 0 ]; then
      printf "cycle %-3s | ✓ %-20s | Δ %s%% | committed\n" "$cycle_no" "$pick" "$delta" >>"$HEARTBEAT" 2>/dev/null || true
      log "cycle $cycle_no committed: $pick Δ $delta%"
    elif [ "$rc" -eq 2 ]; then
      printf "cycle %-3s | ○ %-20s | no auto-fix (manual innovation surfaced)\n" "$cycle_no" "$pick" >>"$HEARTBEAT" 2>/dev/null || true
      log "cycle $cycle_no no auto-fix candidate (manual innovation): $pick"
      # Always innovates: even when no auto-fix, the run surfaced a *new* manual
      # innovation (LLM-validated or fallback). The next agent invocation or the
      # next daemon cycle with a refreshed digest will propose a different angle,
      # so the loop never stalls. No extra action needed here — the ledger
      # already deprioritizes recent picks and research de-dupes via ideas-bank.
    else
      printf "cycle %-3s | ✗ rc=%s\n" "$cycle_no" "$rc" >>"$HEARTBEAT" 2>/dev/null || true
      log "cycle $cycle_no infra failure rc=$rc"
    fi
  fi

  rm -f "$out_file"
  local dur=$((SECONDS - t0))
  write_status "$cycle_no" "$dur" "$rc" "$pick" "$delta"
  log "cycle $cycle_no done rc=$rc dur=${dur}s"
}

# Publish initial status if missing
if [ ! -f "$STATUS_FILE" ]; then
  write_status 0 0 0 "" 0 2>/dev/null || true
fi

CYCLE=0
case "$MODE" in
  once)
    run_cycle 1
    ;;
  loop)
    log "daemon started pid=$$ cycle=${CYCLE_SECS}s"
    while [ "$RUNNING" -eq 1 ]; do
      CYCLE=$((CYCLE + 1))
      run_cycle "$CYCLE"
      [ "$RUNNING" -eq 1 ] || break
      sleep "$CYCLE_SECS" &
      SLEEP_PID=$!
      wait "$SLEEP_PID" 2>/dev/null || true
    done
    log "daemon stopped"
    ;;
esac
exit 0
