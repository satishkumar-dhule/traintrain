.PHONY: build run test check lint fmt fmt-check clean smoke

build:
	cargo build --release

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

clean:
	cargo clean

smoke:
	@if test -f scripts/smoke.sh; then scripts/smoke.sh; else echo "scripts/smoke.sh not found; skipping"; fi
