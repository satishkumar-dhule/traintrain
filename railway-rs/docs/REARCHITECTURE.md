# railway-rs SPA Rearchitecture & Legacy Removal — Plan

## 1. Context & Goal

Rearchitect the working Rust SPA (`railway-rs/static`, vanilla JS, no build step) from 16 flat nav tabs into **5 condensed sections + a More menu** with **hash deep links**, then **completely remove the legacy React app** at the repo root. Nothing loses value: every existing view stays reachable; the backend (18 API slices) is untouched.

Confirmed decisions:
- Target: **Rust SPA** (`railway-rs/static/*`) — not the legacy React app.
- IA: **5 destinations + More** (Home, Track, Station, Plan, PNR).
- Design: re-skin in **plain CSS** per pasted spec (Trust Blue `#1D4ED8`, orange `#F97316`, status colors, `rounded-2xl`/`rounded-full`, ≥48px touch targets, condensed mobile-first).
- `.replit`: **repointed** to the Rust app (not deleted).

## 2. Current State (verified)

- Backend: axum, 18 vertical slices, all live-data (`/rail-api/*`). CI is Rust-only; Dockerfile self-contained; **zero references to root files**.
- SPA: `index.html` + `boot.js` (RailLog) + `ui.js` (DOM helpers, `AutoComplete`) + `api.js` (18 endpoint helpers) + `app.js` (NAV registry, mount, shell search, captcha overlay, `prefill`/`railwayTabs` hacks) + `static/tabs/*.js` (16 tabs).
- No routing, no deep links, ~15 duplicated input+button+spinner+error pipelines.
- Legacy root app: `index.html`, `server.ts`, `src/` (React), `package.json`/lock/`bun.lock`, `tsconfig.json`, `vite.config.ts`, `aggregator.test.ts`, 50+ `.cjs`/`.js` maintenance scripts, root `data/`, root `.env.example`, `metadata.json`, `assets/.aistudio/` — none referenced by railway-rs.

## 3. Target IA & Deep Links

| Section | Route | Views (was) |
|---|---|---|
| Home | `#/` | Quick-action tiles, recent lookups (localStorage), source chip; links to `#/more/stations` |
| Track | `#/train/{num}[/view]` | `spot` (live_status), `schedule`, `map` (train_on_map), `delay` (average_delay), `exceptions` (exceptional), `journey` (journey_basis) |
| Station | `#/station/{code}[/view]` | `live` (live_station), `tt` (station_timetable) |
| Plan | `#/plan/{src}/{dst}[/view]` | `trains` (trains_between), `availability` (new), `chart` (new) |
| PNR | `#/pnr/{pnr}` | pnr |
| More (`⋯`) | `#/more/{view}` | heritage, parcel, stations, `system` (settings), observability, debug |

Hash routing works with the existing `index.html` fallback — zero server change. Shareable, refresh-safe, back/forward works.

## 4. Engineering Principles

- **Deep module (TDD-first):** `static/routes.js` — pure route table (`parse`, `href`, validation), zero DOM, `module.exports` guard. Tests in `railway-rs/tests/js/routes.test.mjs` (Node's built-in `node --test`, no new deps; kept out of `static/` so it's not publicly served).
- **Vertical slice preserved:** tabs become views; `mount(root, ctx, params)` reads route params → validates → fetches → renders; valid params auto-submit, invalid/missing fall back to input form. Replaces `prefill`/`railwayTabs` hacks (verified: no tab listens to `railway:select`).
- **DRY:** `ui.js` primitives — `trainInput`/`stationInput` (wrap `AutoComplete`), `queryCard`, `fetchFlow` (spinner → render | errorBox), `delay`/`days`/`statusCell`. Large self-contained tabs (`train_on_map.js`, `observability.js`) move as-is.
- **KISS:** incremental phases, each leaves the app runnable and gates green. No frameworks, no build step, no backend changes.
- **Radical end-state:** 16 tab files merged into 6 section files; 6 `<script>` tags.

## 5. Phases (12 todos)

0. **Phase 0 — routes.js + tests (TDD, green)**
1. **Phase 1 — Router shell** in `app.js`: 5-section nav + More menu, hash router, compat mapping keeps 16 tabs working.
2. **Phase 2 — Sections** (verify each): Track → Station → Plan (+ `availability`/`chart` per `tests/availability.rs` & `tests/chart.rs` shapes) → PNR + More → Home.
3. **Phase 3 — DRY + merge:** `ui.js` primitives, strip duplicated pipelines, merge to 6 section files, slim `index.html`, remove hacks.
4. **Phase 4 — Design re-skin:** hexes → `:root` CSS variables (values only, class names stable; `obs-*` untouched), `rounded-2xl`/`rounded-full`, ≥48px targets, condensed mobile-first.
5. **Phase 5 — Legacy removal:** `git rm` root `index.html`, `server.ts`, `src/`, `tsconfig.json`, `vite.config.ts`, `package.json`/`package-lock.json`, `bun.lock`, `aggregator.test.ts`, all root `*.cjs`/`*.js` scripts, root `data/`, root `.env.example`, `metadata.json`, `assets/.aistudio/`; repoint `.replit` (drop `nodejs-20`, `[run] command = "cargo run"`, `workdir = "railway-rs"`, `[packager] language = "rust"`, keep `:3000`).
6. **Phase 6 — Hardening:** `Makefile check-js`, CI Node job (`node --check` + `node --test`), README tidy, full gates, boot :3000 + deep-link checklist, zero dangling legacy refs.

## 6. Verification Gates

- `node --check` on every JS file in `static/`
- `node --test tests/js/` (routes suite)
- `cargo fmt --all --check` && `cargo clippy --all-targets -- -D warnings` && `cargo test`
- Curl each view's `/rail-api/*` endpoint for shape
- Boot on :3000 → `/healthz` ok; deep-link checklist (open each route, refresh, back/forward, nav active state)

## 7. Explicitly Out of Scope

- PWA/manifest/offline shell (not in current app)
- Backend endpoint changes (all slices exist)
- `train_on_map.js`/`observability.js` DRY rewrites (kept as-is)
