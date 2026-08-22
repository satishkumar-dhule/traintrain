.PHONY: build build-ui run test check lint fmt fmt-check check-js clean smoke

build:
	cargo build --release

build-ui:
	npm ci --no-fund --no-audit
	npm run build

run:
	RAILWAY_PORT=$(PORT) cargo run

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

# Frontend gates: every served JS file must parse, and the routes + dom-smoke
# suites must pass (Node >= 18 for --test).
check-js:
	@echo "node --check on all static JS..."
	@set -e; for f in $$(find static -name '*.js' -not -path 'static/dist/*' | sort); do node --check "$$f"; done
	@echo "node --test tests/js/..."
	@node --test tests/js/

clean:
	cargo clean

smoke:
	@if test -f scripts/smoke.sh; then scripts/smoke.sh; else echo "scripts/smoke.sh not found; skipping"; fi
