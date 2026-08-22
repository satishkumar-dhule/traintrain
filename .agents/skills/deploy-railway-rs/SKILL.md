---
name: deploy-railway-rs
description: Deploy the railway-rs app to hosting. Use for "deploy", "render", "koyeb", "free hosting", "docker image", "cloudflare tunnel", "PORT binding fails on PaaS". Covers the Render Blueprint, the RAILWAY_PORT→PORT fallback, the multi-stage Dockerfile, systemd, and free no-credit-card options vetted in past sessions.
---

# Deploying railway-rs

Axum binary serves the SPA + JSON API on one port. All config via env vars,
every one optional (`railway-rs/.env.example`). Required files next to the
binary: `static/` and `data/`.

## Port binding (the #1 PaaS failure)

`RAILWAY_PORT` wins, then PaaS-standard `PORT`, then default 3000 — see
`port_from_env` in `railway-rs/src/config.rs`. Never hardcode 3000 in a
platform config; set nothing and let `PORT` be injected, or override with
`RAILWAY_PORT`. The Dockerfile bakes `ENV PORT=3000` as last resort.

## Targets already wired in the repo

**Render (Blueprint, committed):** root `render.yaml` — service `train-bro`,
`runtime: docker`, `plan: free`, region singapore,
`dockerfilePath: ./railway-rs/Dockerfile` (note: the Dockerfile lives inside
`railway-rs/`, not at repo root, despite what README says),
`healthCheckPath: /healthz`, `autoDeploy: true`, env `RAILWAY_LOG_FORMAT=json`,
`RAILWAY_CACHE_TTL=120`. CLI is preinstalled at `~/.local/bin/render`
(login done once); deploys/redeploys go through `render deploys` /
blueprint sync. Verify with `curl -fsS https://<url>/healthz`.

**Docker anywhere:** multi-stage `railway-rs/Dockerfile` —
`rust:1-slim-bookworm` builder runs `cargo build --release --locked`;
`debian:bookworm-slim` runtime as non-root user `railway`; copies binary +
`static/` + `data/`; `HEALTHCHECK curl -f http://127.0.0.1:${PORT:-3000}/healthz`.
`docker build -t railway-rs railway-rs/ && docker run -p 3000:3000 railway-rs`.

**systemd:** `railway-rs/deploy/railway-rs.service` (hardened: non-root
`railway` user, `ProtectSystem=strict`, `NoNewPrivileges`) reading
`/etc/railway-rs/railway-rs.env`; full walkthrough in
`railway-rs/deploy/README.md`.

## Options vetted in past sessions (no configs exist yet)

- **Koyeb**: accepted alternative, works with the Dockerfile as-is; injects
  `PORT` so zero changes needed.
- **Cloudflare Workers/Functions**: NOT drop-in — Rust axum needs a rewrite
  against workerd/wasm APIs; do not attempt without explicit ask.
- **Android phone + `cloudflared` tunnel**: viable hobby path — run the
  release binary (or Docker) on-device, expose via cloudflare tunnel; no repo
  config exists, keep it manual.
- **Free-tier rule**: past decisions rejected anything requiring a credit
  card. Render free / Koyeb free comply; check before suggesting others.
- **Non-India IPs**: NTES is unreachable from most foreign hosts (Render
  included). Expected behavior is honest fallback — responses carry
  `data_source` naming the real upstream (e.g. Railyatri). Never treat that
  as a deploy bug; see `ntes-live-verification`.
