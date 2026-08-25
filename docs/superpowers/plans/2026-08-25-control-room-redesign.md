# Control Room Redesign — Implementation Plan

Spec: `docs/superpowers/specs/2026-08-25-control-room-redesign-design.md`
Working dir for all frontend work: `railway-rs/frontend`
Date: 2026-08-25

## Conventions sheet (pasted into every subagent prompt)

- Svelte 5 runes only: `$state`, `$props()`, `$derived`, `$effect`, `{@render children()}`.
- Icons: `import X from 'lucide-svelte/icons/x'`.
- Styling: Tailwind utility classes referencing semantic tokens ONLY
  (`bg-background text-muted-foreground border-border bg-card` … plus the four
  signal colors exposed as `text-[--signal-run]` style vars or mapped Tailwind
  colors added in Wave 0). NEVER raw oklch/hex in `.svelte` under `src/lib/pages/`.
- Numerals/codes always get class `font-data`. Section labels use `micro-label`.
- No comments unless explaining a non-obvious invariant (match repo style).
- Verification before returning from any lane:
  - `(cd railway-rs && node --test tests/js/)` green
  - `(cd railway-rs/frontend && npm run check:imports && npm run build)` green
  - Never touch files outside your ownership list.
- Do not run `git commit`; leave the tree dirty for the lead to review/commit.

## Phase 0 — Foundation (lead, sequential; everything depends on it)

### 0.1 Fonts
`cd railway-rs/frontend && npm i @fontsource-variable/ibm-plex-sans @fontsource/ibm-plex-mono`

### 0.2 Tests first (RED)
- New `railway-rs/tests/js/tokens.test.mjs`: reads `frontend/src/app.css`,
  asserts presence of: `--signal-run|warn|halt|info`, `--glow`, `--radius: 0.375rem`,
  IBM Plex family declarations, `.font-data`, `.micro-label`, `signal-pulse`
  keyframes, reduced-motion guard for the pulse.
- Extend `tests/js/contrast.test.mjs`: parse both token blocks, assert WCAG AA
  (≥4.5:1 body pairs, ≥3:1 large/UI pairs) for foreground/background,
  muted-foreground/card, each signal on card, BOTH themes.
Run: red confirmed.

### 0.3 Tokens (GREEN)
Rewrite `frontend/src/app.css`: Blueprint light in `:root`, Control Room dark in
`.dark` (values per spec §4.1), `@theme inline` gains
`--color-run/warn/halt/info` so utilities like `text-info` exist; fonts imported
at top; add `.font-data`, `.micro-label`, `--glow`, `@keyframes signal-pulse`
(+ reduced-motion off); radius 0.375rem; chart/sidebar families re-valued;
keep ALL existing utility CSS blocks (md-*, hit-y, idle-center, leaflet,
touch ergonomics) untouched.

### 0.4 Primitives restyle (single-source pass)
`src/lib/components/ui/*`: geometry/radius updates land automatically via
tokens; adjust variants where hardcoded (button sizes, badge tones) to map
semantic→signal (`destructive`→halt etc.). No prop/API changes.

### 0.5 Badge kit remap
`src/lib/components/badges/*`: tone classes switch to signal tokens; delay/pnr/
availability/log-level semantics per spec §4.1. Visual output changes, APIs don't.

### 0.6 Prebuild lint v1 (test-first)
Extend `frontend/scripts/check-component-imports.mjs`: pages
(`src/lib/pages/**`) containing raw oklch/hex literals fail the prebuild. Add
its unit coverage under `tests/js/` first (RED), then implement (GREEN).
Verify Phase 0: `node --test tests/js/` + `npm run build` + commit.

## Wave 1 — three parallel lanes (disjoint file ownership)

### Lane A — composables set 1
Owns: `components/PageShell.svelte`, `SectionHeader.svelte`, `StatTile.svelte`,
`StatusDot.svelte`, `components/index.js` (new), KIT list in prebuild script.
Contracts (fixed so Waves 2 lanes can rely on them):
- `PageShell`: props `{ title?, chips?, breadcrumbs? }` + `{ children }`; wraps
  idle-center + PageHeader composition.
- `SectionHeader`: props `{ label }` + actions snippet slot; micro-label + hairline.
- `StatTile`: props `{ value, label, delta?, tone? }`; mono value.
- `StatusDot`: props `{ tone = 'run', pulse = true }`.

### Lane B — composables set 2
Owns: `components/KeyValueGrid.svelte`, `RouteStrip.svelte`, `Timeline.svelte`,
`components/DataTable.svelte` polish.
Contracts: `KeyValueGrid { rows: [ [label, value] ] }`;
`RouteStrip { from, to, progress (0..1), status }`;
`Timeline { stops: [{ name, code, time, status?, platform? }] }`.

### Lane C — shell + Home scaffold
Owns: `lib/Layout.svelte`, `pages/Home.svelte` (hero scaffold only: search panel
+ stat-tile row consuming StatTile API above).
Sidebar darkens (`bg-sidebar`), groups → micro-labels, active item left
signal-bar + glow; tab bar active icon glow dot; sheet/header restyled. Home
keeps all existing behavior/search wiring — chrome only.

## Wave 2 — five parallel page-adoption lanes (kit-only, lint-enforced)

Adoption checklist per page: wrap in PageShell; SectionHeader blocks; `.font-data`
on every numeral; badges via kit; delete hand-rolled duplicates; zero raw colors
(prebuild enforces); `node --test` + build green.

- Lane D owns `pages/Home.svelte` (post-C adoption), `pages/Plan.svelte`
- Lane E owns `pages/Train.svelte`, `pages/JourneysTable.svelte` (adopt RouteStrip/Timeline)
- Lane F owns `pages/Station.svelte`, `pages/Availability.svelte`
- Lane G owns `pages/Pnr.svelte` (KeyValueGrid), `pages/Exceptions.svelte`, `pages/Extras.svelte`
- Lane H owns `pages/Assistant.svelte`, `pages/System.svelte` (StatTiles), `pages/About.svelte`

## Wave 3 — integration (lead)

1. Review diff across lanes; resolve Layout/Home seams.
2. Deletion sweep: grep for orphaned markup/classes; remove dead helpers.
3. `make build-ui` bundle rebuild; `node --check static/dist` sanity.
4. Gates: `make check-js` (includes real-browser ui suite), cargo `fmt/clippy/test` untouched-but-run.
5. Screenshot QA matrix: ui-layout-harness, 12 pages × 2 themes, artifacts `/tmp/opencode`; fix regressions found.
6. Commit(s) by wave.

## Risk controls
- Ownership lists are hard boundaries; violations = lane rework.
- Contracts fixed in Wave 1 before Wave 2 launches.
- Contrast enforced by tests, not eyes.
