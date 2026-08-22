# UI/UX Modern Revamp — Holistic Plan

## Context

RailCompanion is a fast, honest, privacy-respecting railway data app (Rust/axum backend, vanilla-JS SPA in `static/`, no build step, hash-router deep links, offline autocomplete over real datasets). The prior plan (`.plan/ui-ux-no-scroll.md`) shipped the fixed-viewport shell, scrollable `.main`, nowrap pill bars, compact obs dashboard, and table density — those are the baseline to build on.

Gaps vs. a modern web app (2026 expectations):

- Emoji icons, no brand identity (no favicon / manifest / theme-color)
- No type scale or motion system; generic system-ui font
- No feedback layer: no toasts, no retry on errors, no "updated Xs ago", no auto-refresh on live views
- No entity-first pages: every view is a table in a card; no hero header, no journey visualization
- No hub: `#/` drops into the Train landing; no dashboard or favorites
- System section mixes dev tooling (debug/logs) with user-facing observability
- No micro-interactions: pill switches remount abruptly; no transitions; no scroll preservation
- No explicit light/dark/system toggle (only OS-follow via `light-dark()`); not installable as PWA
- Loading / empty / error states inconsistent across sections

## Principles

1. **Entity-first** — a train/station page leads with a hero (name, number, route, live pulse, actions); tables follow as secondary.
2. **Calm data density** — dense data is fine; clutter is not. One type scale, one spacing scale; status colors only for status.
3. **Every state designed** — loading (skeleton), empty, error (with retry), stale (with refresh), live (pulse + timestamp).
4. **No-build, offline-friendly** — all assets self-hosted, `?v=`-busted, no new framework.
5. **Accessible by default** — keyboard-first, `aria-live`, focus management, reduced-motion, forced-colors, AA contrast.
6. **Tokens over exceptions** — anything repeated goes into `styles.css` tokens + `ui.js` helpers; no new inline styles.

## Open Decisions (recommended defaults)

| # | Decision | Recommended | Effort |
|---|---|---|---|
| D1 | Home dashboard section as new `#/` root | Yes — hub (search, quick actions, favorites, recents, source status). Train landing stays at `#/train` | M |
| D2 | Favorites (star trains/stations in localStorage) | Yes — shown on Home + hero actions; cheap, high perceived value | S |
| D3 | Self-host variable font (Inter) | Yes — woff2 in `static/vendor/fonts/`, system fallback | S |
| D4 | PWA manifest + installability | Yes — manifest, icons, theme-color; defer service worker (clashes with `?v=` busting) | S |
| D5 | Theme toggle (light/dark/system) | Yes — header quick toggle + settings, persisted, no-FOUC boot | S |
| D6 | Move Settings/Debug out of the System tab bar | Yes — into a "Settings" sub-entry; tab bar keeps Observability | S |

---

## Phase A — Design Foundation

### A1. Tokens (`static/styles.css` `:root`)

- **Type scale**: add `--fs-2xs: 11px`, `--fs-3xl: 30px`, `--fs-4xl: 36px`; tokenize fonts as `--font-ui`, `--font-mono`.
- **Spacing scale**: `--sp-1: 4px` … `--sp-8: 32px`; retire magic literals in new code.
- **Motion**: `--dur-1: 120ms`, `--dur-2: 200ms`, `--dur-3: 300ms`; `--ease-out`. Reduced-motion already zeroes these.
- **Elevation**: keep `--shadow-sm/md/lg`, add `--shadow-pop` for menus/sheets.
- **Semantic**: add `--info`/`--info-bg` (cyan); add `--accent-grad` for brand moments (logo, hero avatars).
- **Z-index scale**: `--z-sticky: 10; --z-header: 50; --z-menu: 60; --z-palette: 200; --z-toast: 300; --z-sheet: 400`.

### A2. Icon system (`static/icons.svg` sprite + `ui.js`)

- Inline `<svg><symbol>` sprite, ~28 glyphs: home, train, station, pin, plan, system, search, refresh, copy, share, star, star-fill, sun, moon, clock, calendar, chevron-l/r, close, check, alert, info, delay, settings, log, spark, swap, filter.
- `UI.icon(name, cls)` → `<svg class="ic ..."><use href="#i-{name}"/></svg>`; `.ic { width:1em; height:1em; }`.
- Sweep: replace emoji in `app.js` (nav), `track.js` (ENTITY_ICONS), `palette.js` (parsedIcon), `index.html` (logo).

### A3. Brand & shell metadata (`static/index.html`, `static/boot.js`)

- `favicon.svg` (RC monogram, accent→primary gradient), apple-touch-icon PNG, `manifest.webmanifest`, `<meta name="theme-color">`, `viewport-fit=cover`.
- `boot.js`: read `rc.theme` (light|dark|system) **before paint**, set `data-theme` + `color-scheme` on `<html>`; expose `window.AppTheme`. Toggle in header, sidebar footer, Settings.
- D3: `@font-face` Inter variable + `font-display: swap`.

---

## Phase B — App Shell & Navigation

### B1. Header (`index.html`, `styles.css`, `app.js`)

- Brand (SVG logo + wordmark) · global search (focus on `/` key) · theme toggle · live-mode badge (add pulsing dot) · Cmd+K hint chip (desktop). Sticky, `--z-header`.

### B2. Navigation

- Mobile bottom nav: 5 items — Home, Train, Station, Plan, System — `UI.icon` + label, active indicator bar, pressed state, `env(safe-area-inset-bottom)`.
- Desktop sidebar: brand, nav, **Favorites** group (live re-render), footer = source-status box + theme toggle.
- `ui.pillBar`: add `role="tablist"`, scroll active pill into view.

### B3. Home dashboard (new `static/sections/home.js` — D1)

- routes.js: `SECTIONS.home` + `/^\/$/` → `#/home`; register in `app.js` nav.
- Content: greeting + date; **big search combo** (reuse suggest); **quick actions** tiles (Track Train, PNR, Live Station, Plan Journey); **Favorites**; **Recent**; **source status** chip (live/offline + primary source).
- Fallback if D1 declined: upgrade the Train landing in `track.js` into this hub (same components, no new route).

### B4. Route transitions & scroll (`app.js`)

- Fade/slide-in `.tab-content` (`--dur-2`); **preserve `.main` scrollTop across pill switches within a section** (store per-route scroll, restore on back, reset on section change). Deep-link auto-submit stays as-is.

---

## Phase C — Component System (`ui.js` + `styles.css`)

1. `toast(msg, kind)` — `#toast-host` in `index.html`, kinds info/success/error, auto-dismiss 4s. Used for copy-link, save/unsave, errors, source-down.
2. `errorState(title, hint, retryFn)` — icon + message + Retry; replaces bare `errorBox` on fetch paths (keep `errorBox` for inline validation).
3. `emptyState(icon, title, hint)` — replaces `notice('No ...')` in result lists.
4. `skeletonTable(rows, cols)` / `skeletonCard(lines)` — variants of existing `skeleton`.
5. `heroCard(entity, facts, actions)` — icon avatar, name (`--fs-xl`), code badge, route line, facts row, actions (favorite star, copy link, share via `navigator.share` w/ copy fallback, refresh).
6. `refreshRow(updatedAt, onRefresh, auto)` — "Updated Xs ago · Refresh" + auto-refresh toggle (per-view preference in localStorage).
7. `liveDot(pulse)` — pulsing dot + "LIVE".
8. `statTile(label, value, sub, kind)` — KPI tile (Home status, PNR hero, board header).
9. `seg` segmented control for 2–3 options (live hours, log filter).
10. Favorites in app ctx: `ctx.fav = { list, has, toggle, onchange }`, `localStorage['rc.favs']` (D2).
11. `copyLink(hash)` / `share(hash)` with toast feedback.
12. `journeyProgress(stops, currentIdx)` — horizontal origin→current→destination track for Spot.
13. `dialog(promise)` — focus trap + Esc; replaces the ad-hoc captcha modal in `app.js` (`showCaptcha`).
14. Card polish: `.card` shadow + hover lift on interactive cards; `.card-flush` for tables.

---

## Phase D — Section Redesigns

### D1. Train (`sections/track.js`)

- Entity view: `heroCard` (number, name, source→dest, running-days chips, live dot) + actions (favorite, copy, share, refresh).
- **Spot**: replace "Current Position" card with a **journey timeline** — origin → current stop (pulse highlight) → destination, position line, run date, delay chip; stations table follows; instances picker becomes compact date chips.
- **Schedule**: keep collapsible table; route line + duration in hero.
- **Delay**: per-station delay column with inline CSS bar sparklines, color-coded.
- **Map**: keep Leaflet; optional toolbar (recenter/fullscreen).
- Landing (if D1 declined): hub layout with stat tiles.
- Live views (spot, delay) get `refreshRow` + auto-refresh.

### D2. Station (`sections/station.js`)

- **Live board**: mono numerals for times, big ETA cell with delay chip (`+23 min`, amber), platform chip, **All/Arr/Dep filter seg** (client-side), live dot + refresh row.
- **TT**: hero (station name, total trains), keep collapsible table.
- **Heritage/Parcel**: migrate to new cards, icons, empty states.

### D3. Plan (`sections/plan.js`)

- Trip builder: from/to inputs + **swap button**, date quick-pick, one responsive row.
- **Trains** results: route cards (number/name, mono dep/arr, duration, days chips) instead of bare table.
- **Availability**: class chips colored by status (AVAILABLE/WL/RAC).
- **Chart**: coach tabs (one coach at a time) instead of one giant berth table.

### D4. System (`sections/system.js`)

- **Observability**: refresh row + live dot; icons on KPI cards; log filter seg + clear.
- **Settings** (D6): theme toggle, refresh/auto-refresh defaults, data-mode display; Debug collapsible under "Advanced".

---

## Phase E — Feedback, Motion & Freshness

- Toasts wired into copy/share/save/errors; captcha via `dialog`.
- Route transitions (B4); hover/active/pressed states on all interactive elements; list item micro-lift.
- `aria-live="polite"` on results regions; `aria-label` on icon-only buttons; keyboard: `/` focus search, Esc closes menus, focus trap in dialogs.
- Freshness UX: `refreshRow` on every live view; offline banner (`navigator.onLine`) when the network drops; `data-source` badges retained but de-emphasized.

---

## Phase F — Mobile & PWA Polish

- `viewport-fit=cover` + safe-area insets on header/bottom-nav; bottom nav labels on wide phones.
- PWA: manifest + icons + install prompt via `beforeinstallprompt` (minimal, no SW).
- 375px QA pass; desktop wide-layout check (max-width 1100px for `.main`).

---

## File Inventory

| File | Changes |
|---|---|
| `static/styles.css` | Tokens (type/spacing/motion/z), icon styles, hero/stat/toast/dialog/empty-state/refresh-row/seg, card polish, safe-area |
| `static/icons.svg` | NEW — symbol sprite (~28 glyphs) |
| `static/index.html` | Sprite inline, favicon/manifest/theme-color, `#toast-host`, header structure |
| `static/boot.js` | Theme boot (no FOUC), `AppTheme` API |
| `static/manifest.webmanifest` + `static/favicon.svg` | NEW — PWA + brand |
| `static/ui.js` | `icon`, `toast`, `errorState`, `emptyState`, skeleton variants, `heroCard`, `refreshRow`, `liveDot`, `statTile`, `seg`, `dialog`, `copyLink`, `share`, `journeyProgress` |
| `static/app.js` | 5-item nav, favorites ctx (`ctx.fav`), scroll preservation, transitions, search `/` shortcut, captcha via dialog |
| `static/routes.js` | `home` section (D1) |
| `static/sections/home.js` | NEW — dashboard (D1) |
| `static/sections/track.js` | Hero, journey timeline, delay bars, refresh rows, favorites, icon sweep |
| `static/sections/station.js` | Board-style live view, filter seg, hero, refresh row |
| `static/sections/plan.js` | Trip builder (swap), route cards, class chips, coach tabs |
| `static/sections/system.js` | Obs icons + refresh, Settings consolidation, Advanced/Debug |
| `static/palette.js` | Icons, favorites commands, theme command |

## Implementation Order

A (foundation) → B (shell/nav/home) → C (components) → D (sections) → E (motion/feedback) → F (mobile/PWA). Each phase ends with a shippable state; cargo tests + routes.js unit tests must stay green (`cargo test`, `node` on `routes.js` if present in tests).

## Verification

1. `cargo test` (backend) + `tests/routes` unit tests — no regressions.
2. Manual QA matrix (375×667, 768×1024, 1440×900): Home, `#/train/12559` + all views, `#/station/NDLS`, `#/plan/NDLS/BSB`, `#/system/observability`; check states: loading skeleton, error+retry (kill server), empty (bad code), live pulse.
3. Dark mode + forced-colors + reduced-motion passes on all new components.
4. Lighthouse: a11y ≥ 95, no layout shift on route change, offline asset check (no CDN beyond Leaflet).
