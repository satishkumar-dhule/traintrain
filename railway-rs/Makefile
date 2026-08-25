.PHONY: build build-ui run run-release test check lint fmt fmt-check check-js check-ui clean smoke

build:
	cargo build --release

build-ui:
	(cd frontend && npm ci && npm run build)

run:
	RAILWAY_PORT=$(PORT) cargo run

run-release:
	RAILWAY_PORT=$(PORT) cargo run --release

test:
	cargo test

check:
	cargo check

lint:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

# Frontend gates: every served JS file must parse, the routes + dom-smoke
# suites must pass (Node >= 18 for --test), and the real-browser UI suite
# (tests/ui/) must pass against the app on :3000.
check-js:
	@echo "node --check on all static JS..."
	@set -e; for f in $$(find static -name '*.js' -not -path 'static/dist/*' | sort); do node --check "$$f"; done
	@echo "node --test tests/js/..."
	@node --test tests/js/
	@$(MAKE) --no-print-directory check-ui

# Real-browser UI suite: playwright-core + a private chromium-headless-shell
# under $UI_HARNESS_HOME (default /tmp/opencode/ui-harness, zero host
# installs). Bootstraps the harness on first use; specs skip with a loud
# message when no browser can be provisioned unless UI_STRICT=1. The suite
# reuses an already-running server on :3000 or starts one from target/.
check-ui:
	@if [ ! -d "$${UI_HARNESS_HOME:-/tmp/opencode/ui-harness}/browsers" ]; then \
	  echo "bootstrapping ui harness (first run only)..."; \
	  node ../.agents/skills/ui-layout-harness/scripts/setup.mjs \
	    || echo "harness bootstrap failed - UI specs will skip"; \
	fi
	@node --test tests/ui/

clean:
	cargo clean

smoke:
	@if test -f scripts/smoke.sh; then scripts/smoke.sh; else echo "scripts/smoke.sh not found; skipping"; fi
