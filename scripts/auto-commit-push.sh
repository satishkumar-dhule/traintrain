#!/usr/bin/env bash
set -u

REPO_ROOT="/home/runner/workspace"
APP_DIR="$REPO_ROOT/railway-rs"
INTERVAL="${1:-300}"
BRANCH="$(git -C "$APP_DIR" rev-parse --abbrev-ref HEAD)"

export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_HOME="$HOME/.cargo"

log() { printf '[auto %s] %s\n' "$(date -u '+%F %T')" "$*"; }

tree_dirty() {
	[ -n "$(git -C "$REPO_ROOT" status --porcelain -- ':!.opencode' ':!logs' ':!.space-janitor')" ]
}

tree_settled() {
	local a b
	a="$(git -C "$REPO_ROOT" status --porcelain)"
	sleep 15
	b="$(git -C "$REPO_ROOT" status --porcelain)"
	[ "$a" = "$b" ]
}

gates_pass() {
	cd "$APP_DIR" || return 1
	cargo fmt --all --check >/dev/null 2>&1 || { log "gate fmt failed"; return 1; }
	if ! cargo clippy --all-targets -- -D warnings >/tmp/opencode/gate-clippy.log 2>&1; then
		log "gate clippy failed: $(tail -2 /tmp/opencode/gate-clippy.log | tr '\n' ' ')"
		return 1
	fi
	if ! cargo test >/tmp/opencode/gate-cargo.log 2>&1; then
		log "gate cargo-test failed: $(grep -m1 -E 'FAILED|error\[|panicked' /tmp/opencode/gate-cargo.log)"
		return 1
	fi
	if ! node --test tests/js/*.test.mjs >/tmp/opencode/gate-js.log 2>&1; then
		log "gate js-tests failed"
		return 1
	fi
	return 0
}

frontend_rebuild_if_stale() {
	local ref stale
	ref="$(ls -t "$APP_DIR"/static/assets/* 2>/dev/null | head -1)"
	[ -z "$ref" ] && ref="$APP_DIR/static/index.html"
	stale="$(find "$APP_DIR/frontend/src" "$APP_DIR/frontend/index.html" -type f -newer "$ref" 2>/dev/null | head -1)"
	if [ -n "$stale" ]; then
		log "bundle stale ($stale), rebuilding"
		(cd "$APP_DIR/frontend" && npm run build --silent >/tmp/opencode/build.log 2>&1) || { log "frontend build failed"; return 1; }
	fi
}

clean_stale_assets() {
	cd "$APP_DIR/static/assets" || return 0
	local entries refs new merged f r keep
	entries="$(grep -ho 'assets/[A-Za-z0-9_.-]*' ../*.html | sed 's|assets/||' | sort -u)"
	refs="$entries"
	while :; do
		new=""
		for r in $refs; do
			case "$r" in
				*.js|*.css) [ -f "$r" ] && new="$new $(grep -hoE '[A-Za-z0-9_.-]+\.(js|css|woff2?|png|jpe?g|svg|webp)' "$r" | sort -u)" ;;
			esac
		done
		merged="$(printf '%s\n%s\n' "$refs" "$new" | tr ' ' '\n' | sort -u)"
		[ "$merged" = "$(printf '%s\n' "$refs" | tr ' ' '\n' | sort -u)" ] && break
		refs="$merged"
	done
	for f in *; do
		keep=0
		for r in $refs; do
			[ "$f" = "$r" ] && keep=1 && break
		done
		if [ "$keep" -eq 0 ] && [ -n "$(find "$f" -mmin +30 -print -quit 2>/dev/null)" ]; then
			rm -f -- "$f"
		fi
	done
}

push_if_ahead() {
	git -C "$REPO_ROOT" fetch -q origin 2>/dev/null
	if [ -n "$(git -C "$REPO_ROOT" rev-list "origin/$BRANCH..$BRANCH" 2>/dev/null)" ]; then
		gh auth setup-git >/dev/null 2>&1
		if git -C "$REPO_ROOT" push -q origin "$BRANCH"; then
			log "pushed"
		else
			log "push failed, will retry next cycle"
		fi
	fi
}

log "loop started (interval ${INTERVAL}s, branch $BRANCH)"
while true; do
	if tree_dirty && tree_settled; then
		frontend_rebuild_if_stale || continue
		clean_stale_assets
		if gates_pass; then
			git -C "$REPO_ROOT" add -A -- ':!.opencode' ':!logs' ':!.space-janitor'
			if git -C "$APP_DIR" diff --cached --quiet; then
				log "gates green, nothing to commit"
			else
				git -C "$APP_DIR" commit -q -m "auto: green snapshot $(date -u '+%F %T')"
				log "committed $(git -C "$APP_DIR" rev-parse --short HEAD)"
			fi
		fi
	fi
	push_if_ahead
	sleep "$INTERVAL"
done
