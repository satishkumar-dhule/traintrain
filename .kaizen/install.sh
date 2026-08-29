#!/usr/bin/env bash
# install.sh — idempotent installer for the Kaizen autonomous daemon.
# Mirrors .space-janitor/install.sh: safe to re-run after home wipe.
set -euo pipefail

BASE="/home/runner/workspace/.kaizen"
HOOK="$BASE/hook.sh"

MARK_BEGIN="# >>> kaizen >>>"
MARK_END="# <<< kaizen <<<"
BLOCK="$MARK_BEGIN
[ -r $HOOK ] && . $HOOK
$MARK_END"

ensure_hook_in() {
  local rcfile="$1"
  [ -f "$rcfile" ] || : >"$rcfile"
  if [ -L "$rcfile" ]; then
    local target; target="$(readlink -f "$rcfile")"
    tmp="$(mktemp "${rcfile}.new.XXXXXX")"
    cat "$target" >"$tmp"
    printf '\n%s\n' "$BLOCK" >>"$tmp"
    mv "$tmp" "$rcfile"
    echo "materialized symlinked $rcfile (was -> $target) + hook"
    return 0
  fi
  if grep -qF "$MARK_BEGIN" "$rcfile"; then
    echo "hook already present in $rcfile"
  else
    printf "\n%s\n" "$BLOCK" >>"$rcfile"
    echo "hook appended to $rcfile"
  fi
}

mkdir -p "$BASE/logs"
chmod +x "$HOOK" "$BASE/daemon.sh" 2>/dev/null || true

ensure_hook_in "$HOME/.bashrc"
# also hook .profile if present (like space-janitor)
if [ -f "$HOME/.profile" ] || [ "${KAIZEN_PROFILE:-1}" = "1" ]; then
  ensure_hook_in "$HOME/.profile"
fi

if [ -f "$BASE/PAUSE" ]; then
  echo "PAUSE file present; daemon NOT started (remove $BASE/PAUSE to enable)."
elif [ -r "$BASE/pid" ] && kill -0 "$(cat "$BASE/pid")" 2>/dev/null; then
  echo "daemon already running (pid $(cat "$BASE/pid"))"
else
  nohup "$BASE/daemon.sh" --loop >>"$BASE/logs/daemon.out" 2>&1 </dev/null &
  disown 2>/dev/null || true
  echo "daemon started"
fi
