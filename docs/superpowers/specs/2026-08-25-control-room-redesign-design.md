# Control Room — Radical Aesthetics Upgrade (Design Spec)

Date: 2026-08-25
Status: REBASED — see Addendum below. Original Control Room direction superseded.
Scope: `railway-rs/frontend` only. No Rust API changes.

## ADDENDUM (2026-08-25, post-commit 709f4ec)

A parallel session shipped its own radical identity, **"Signal & Steel"**, which
the user chose as the foundation (user decision: option 1). Superseded vs kept:

- **Identity**: Signal & Steel wins — coach-indigo primary, saffron accent,
  signal lamps `--signal-go/hold/stop` (+ `-ink` AA text variants), cool-paper
  light / night-indigo dark on the 255–272 blue-steel axis.
- **Type**: Archivo Variable (signage/UI, `signage` utility) + JetBrains Mono
  (`data-num`, tabular numerals) — replaces IBM Plex. Fonts already bundled.
- **Tokens**: `go/hold/stop` naming replaces `run/warn/halt`. Radius stays
  0.625rem. `lamp-pulse` replaces `signal-pulse`.
- **Already shipped** (do not rebuild): token layer + utilities in `app.css`;
  `tokens.test.mjs`, extended `contrast.test.mjs`; `uniformity.test.mjs`
  (incl. U3 raw-palette ban); components PageShell, PageHeader, Breadcrumbs,
  TabBar, TrackRule, AsyncState, EmptyState, FilterChipGroup, EntityBadge,
  SignalDot, StatPill, badges kit on signal tones; DRY fan-out of pages.
- **Remaining scope of this plan** (the actual work):
  1. Shell restyle to full identity: Layout rail (darker-than-bg sidebar,
     micro-label groups, active signal-bar), mobile tab bar active glow dot.
  2. Home hero: departure-board search panel (signage face) + live StatPill row.
  3. Extract still-duplicated composables: `Timeline` (Train stops),
     `RouteStrip` (between-stations progress), `KeyValueGrid` (PNR/station
     facts); adopt across pages; delete hand-rolled equivalents.
  4. Page-by-page aesthetic adoption sweep (all 12 pages): signage titles,
     data-num on every numeral, SectionHeader rhythm, glow discipline
     (live-only), zero raw palette literals (U3 enforced).
  5. Screenshot QA matrix (12 pages × 2 themes) via ui-layout-harness.
- Execution waves unchanged in shape (Wave 1 lanes A/B/C → Wave 2 D–H → Wave 3
  integration), with lane A scoped to the three missing composables above.

## 1. Context

The Train Bro UI is a Svelte 5 + Tailwind 4 app with shadcn-svelte primitives
(`src/lib/components/ui/`), a 12-badge kit (`components/badges/`), and 12 pages
(`src/lib/pages/`). The current look is the stock shadcn neutral-zinc palette:
no brand identity, no custom fonts, one radius token, duplicated page markup
(Train.svelte 1,103 lines; System.svelte 732). A component-library pass already
exists (PageHeader, Breadcrumbs, StatPill, DataTable, EmptyState) and JS quality
gates are in place: `node --test` suite under `railway-rs/tests/js` (including
`contrast.test.mjs`), prebuild import checker, cargo fmt/clippy/test.

User mandate: radical aesthetic change across ALL surfaces, full shell redesign,
DRY/KISS/TDD discipline, reusable components, executed via deep parallel
delegation.

## 2. Direction: "Control Room"

Dark-first rail operations center. Status IS the color system: signal green /
amber / red / cyan-blue carry all meaning. IBM Plex Sans + Mono typography,
tabular numerals, squarer geometry, hairline borders, glow reserved for live
elements only. Light mode survives as a tuned "Blueprint" companion theme.

## 3. Goals / Non-goals

**Goals**
1. New token layer in `app.css`: Control Room palette (dark flagship + Blueprint light), signal semantics, type ramp, radius, motion.
2. All existing components restyled once at the source (`ui/` primitives via tailwind-variants; badge kit remapped to signal tokens) — zero consumer API changes.
3. Extract duplicated markup into ~7 new composables; delete hand-rolled equivalents from pages.
4. Redesigned app shell (desktop rail, mobile tab bar/sheet/header).
5. Every page adopted to the kit; no raw color literals in pages (lint-enforced).
6. Tests-first for every gate; all gates green at merge.

**Non-goals**
- No new features, routes, or backend changes.
- No multi-skin theme engine (single identity, two modes).
- No chart library swap; Leaflet map stays (controls re-skinned only).
- No rewrite of router/api/state modules.

## 4. Design language (tokens)

### 4.1 Color (oklch)

Mechanism unchanged from today (`theme.svelte.js` toggles `.light`/`.dark` on
`<html>`): `:root` carries the Blueprint light values, the `.dark` block the
Control Room flagship. Values below are the **dark** set:

| Token | Value | Use |
|---|---|---|
| `--background` | `oklch(0.155 0.012 240)` | page |
| `--card` | `oklch(0.19 0.014 240)` | panels/cards |
| `--popover` | `oklch(0.23 0.016 240)` | menus/dialogs |
| `--foreground` | `oklch(0.96 0.005 240)` | primary text |
| `--muted` | `oklch(0.24 0.012 240)` | muted fills |
| `--muted-foreground` | `oklch(0.68 0.015 240)` | secondary text |
| `--border` | `oklch(1 0 0 / 11%)` | hairlines |
| `--signal-run` | `oklch(0.78 0.16 162)` | on-time / running / success |
| `--signal-warn` | `oklch(0.8 0.14 85)` | delayed / attention |
| `--signal-halt` | `oklch(0.66 0.19 25)` | cancelled / error / destructive |
| `--signal-info` | `oklch(0.72 0.12 230)` | scheduled / links / primary action |
| `--primary` | alias of `--signal-info` | buttons/active nav |
| `--ring` | `oklch(0.72 0.12 230 / 60%)` | focus rings |
| `--glow` | `0 0 12px var(--signal-run / 35%)` | live elements ONLY |

Blueprint light (`.light`): background `oklch(0.975 0.006 240)`, foreground ink
`oklch(0.19 0.02 250)`, card white; signal hues darkened ~15% lightness (e.g.
run → `oklch(0.6 0.14 162)`, info → `oklch(0.55 0.13 230)`) to hold WCAG AA.
Chart tokens `--chart-1..5` remapped onto the signal family. Sidebar family
(`--sidebar*`) retained but re-valued darker than page bg in dark mode
(`oklch(0.125 0.012 240)`).

Every existing badge/status component remaps onto the four signals — consistency
is enforced by tokenization, not review.

### 4.2 Typography

- Deps: `@fontsource-variable/ibm-plex-sans`, `@fontsource/ibm-plex-mono` (400/500/600).
- Body/UI: IBM Plex Sans Variable. Numerals & codes (times, delays, train numbers,
  platforms, stats): IBM Plex Mono + `font-variant-numeric: tabular-nums`.
- Utilities added to `app.css`: `.font-data` (mono + tabular), `.micro-label`
  (11px uppercase tracking +0.08em mono, the ops-console signature).
- Page titles: `text-xl md:text-2xl font-semibold tracking-tight`.

### 4.3 Geometry & motion

- `--radius: 0.375rem` (down from 0.625rem); derived sm/md/lg/xl unchanged formulas.
- Spacing stays Tailwind scale (4px grid).
- Motion tokens: `--motion-fast: 120ms`, `--motion-panel: 200ms`, both ease-out;
  `@media (prefers-reduced-motion: reduce)` disables pulse/transitions (extends
  existing pattern).
- Live pulse: `@keyframes signal-pulse` (opacity 1→0.55→1, 2s infinite) applied
  only by `StatusDot`/live badges.

## 5. Component architecture

### 5.1 Keep & remap (no API changes)
All 12 badges (`badges/index.js` exports), PageHeader, Breadcrumbs, DataTable,
EmptyState, EntityChip, DateStrip, DisplaySettings, SourceTrustChip, StatPill
(absorbed by StatTile where adopted), `ui/*` primitives (button, badge, card,
input, select, dialog, tabs, table, command, breadcrumb, alert, separator,
skeleton, label, textarea, input-group).

### 5.2 New composables (extract only ≥2-page duplicates)
| Component | Owns | First consumers |
|---|---|---|
| `PageShell.svelte` | idle-center behavior, container width, header/content rhythm | all pages |
| `SectionHeader.svelte` | micro-label + hairline rule + actions slot | Train, Availability, System, Pnr |
| `StatTile.svelte` | mono value + label + delta chip | Home, Train, System |
| `StatusDot.svelte` | pulsing live indicator (only glow animation site) | badges, Train, Station |
| `KeyValueGrid.svelte` | label/value rows | Pnr, Station, Extras |
| `RouteStrip.svelte` | between-stations progress bar | Train, JourneysTable |
| `Timeline.svelte` | stop list with signal-colored states | Train, Availability |

Placement: `src/lib/components/`. Exported via `src/lib/components/index.js`;
added to prebuild KIT list so usage-without-import fails the build.

### 5.3 Enforcement (prebuild lint, test-first)
Extend `frontend/scripts/check-component-imports.mjs` (or sibling script run in
prebuild): pages under `src/lib/pages/**.svelte` must not contain raw oklch/hex
color literals nor `<div class="…bg-…" >` card-like hand-rolled blocks — kit
imports required. Failing build = red test.

## 6. Shell redesign (`Layout.svelte`)

Desktop: sidebar bg darker than page (`--sidebar` above), nav groups as
micro-labels, active item = 2px left signal-bar + faint glow, content unchanged.
Mobile: information architecture unchanged (bottom tab bar + More sheet +
scroll-aware header); chrome restyled — active tab icon gains signal-glow dot,
sheet keeps existing motion classes. Home hero: departure-board style search
panel (mono, oversized) + live stat tiles row.

Page template contract: `PageShell > PageHeader (title + status chips +
breadcrumbs) > SectionHeader blocks`. Idle-center preserved exactly.

## 7. Testing strategy (TDD order)

1. **Wave 0 tests first**: new `tests/js/tokens.test.mjs` asserts presence of
   `--signal-*`, font-family, radius, glow vars in `frontend/src/app.css`;
   extend `contrast.test.mjs` to assert AA pairs in BOTH themes. Red → implement
   tokens → green.
2. **Per-lane gates**: `node --test` green; prebuild lint rules added test-first;
   `npm run check:imports`; cargo gates untouched.
3. **Visual verification**: ui-layout-harness real-browser screenshots after each
   wave — matrix of 12 pages × 2 themes, artifacts under `/tmp/opencode`.
4. Final: bundle rebuild, full gate sweep, screenshot QA matrix reviewed.

## 8. Execution plan (deep fan-out, n=2 delegation)

```
WAVE 0 (lead, sequential): fonts deps, app.css rewrite, ui/ primitive restyle,
  badge signal remap, token+contrast tests, lint rule v1
WAVE 1 (3 parallel subagents):
  A: composables set 1 — PageShell, SectionHeader, StatTile, StatusDot (+index.js, KIT list)
  B: composables set 2 — KeyValueGrid, RouteStrip, Timeline + DataTable polish
  C: shell — Layout.svelte rail/tabbar/sheet restyle + Home hero scaffold
WAVE 2 (5 parallel subagents, kit-only, lint-enforced):
  D: Home + Plan          E: Train + JourneysTable     F: Station + Availability
  G: Pnr + Exceptions + Extras                          H: Assistant + System + About
WAVE 3 (lead): integration, duplicate-markup deletion sweep, bundle rebuild,
  screenshot QA matrix, all gates green
```

Each subagent receives: file ownership list (no overlap), token cheat-sheet,
kit API surface, "tests green before return" contract, and instruction to follow
existing code conventions (Svelte 5 runes, `$props()`, lucide icon imports).

## 9. Risks

- **Parallel merge conflicts**: mitigated by strict file ownership per lane; WAVE 1 lanes touch disjoint directories except Layout/Home boundary (C owns Layout.svelte + Home scaffold; D adopts Home in WAVE 2 after C merges).
- **Contrast regressions in Blueprint light**: enforced by extended contrast test, not eyeballing.
- **Bundle size**: two font packages ≈ 150–220KB woff2 self-hosted; accepted by user decision.
- **Svelte 5 runes pitfalls in subagent code**: lanes get convention sheet; lead reviews WAVE 3.
