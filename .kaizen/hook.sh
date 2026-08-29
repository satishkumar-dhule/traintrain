#!/usr/bin/env bash
# hook.sh — sourced from ~/.bashrc / ~/.profile by install.sh.
# Starts the Kaizen autonomous daemon on login if not already running.
# The daemon continuously innovates: each cycle runs `run.sh --research`
# (or deterministic fallback) and records the improvement.

KAIZEN_BASE="/home/runner/workspace/.kaizen"

kaizen_maybe_start() {
  [ -d "$KAIZEN_BASE" ] || return 0
  [ -f "$KAIZEN_BASE/PAUSE" ] && return 0
  if [ -r "$KAIZEN_BASE/pid" ]; then
    local pid
    pid="$(cat "$KAIZEN_BASE/pid" 2>/dev/null)"
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && return 0
  fi
  mkdir -p "$KAIZEN_BASE/logs" 2>/dev/null
  if command -v nohup >/dev/null 2>&1 && command -v flock >/dev/null 2>&1; then
    nohup "$KAIZEN_BASE/daemon.sh" --loop >>"$KAIZEN_BASE/logs/daemon.out" 2>&1 </dev/null &
    disown 2>/dev/null || true
  fi
}
kaizen_maybe_start
unset -f kaizen_maybe_start
