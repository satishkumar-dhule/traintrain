# Builder stage
FROM rust:1-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release --locked

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /usr/sbin/nologin railway

WORKDIR /app
COPY --from=builder /app/target/release/railway-rs /app/railway-rs
COPY static/ /app/static/
COPY data/ /app/data/

USER railway
EXPOSE 3000
ENV RAILWAY_PORT=3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -f http://127.0.0.1:3000/healthz || exit 1
ENTRYPOINT ["/app/railway-rs"]
