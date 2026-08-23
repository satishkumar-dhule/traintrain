# Session Visit-Trail Strip (Top of Page)

## Context

As the user navigates the Svelte app, append every visited page to a horizontally
scrollable "trail" strip pinned at the top of the page — a **path-based breadcrumb**
(session visit log), distinct from location-based breadcrumbs (reserved for entity
context by PLAN.md Phase 3 `.entity-breadcrumb`). Naming avoids all existing
collisions: `timeline` (train stop timeline, src/models.rs:147), `journey*`
(Journey Basis slice), `recent`/`RecentSearches` (per-page recent chips),
`tracking*` (no-tracking product promise in About).

**Decisions** (user-confirmed): entries = **data-entry navigations only** (paths
with an entity segment, e.g. `/train/12951` — bare section hops like Home/Live
are NOT recorded); persistence = session only (`sessionStorage`); UI =
shadcn-svelte Breadcrumb primitives; strip must **fit the viewport with no
horizontal scroll** — on overflow the oldest crumbs are trimmed (ellipsis
marker shown), never scrolled.

**Why no tracking library**: the app has a single navigation choke point —
`navigate()` + the `popstate` listener in `frontend/src/lib/router.svelte.js`.
All nav (sidebar links Layout.svelte:54-58, bottom bar, PowerSearch palette)
funnels through it. Instrumenting it *is* the established pattern here;
the Navigation API / `history` lib would be redundant since we own the router.

---

## Phase 1: Breadcrumb primitive

### Install

```bash
cd railway-rs/frontend && npx shadcn-svelte@latest add breadcrumb
```

Generates `src/lib/components/ui/breadcrumb/*` onto the existing
bits-ui / tailwind-variants / tailwind-merge stack (already in package.json).
No other deps. Verify with `npm run check:imports`.

---

## Phase 2: Visit-trail store

### File: `frontend/src/lib/visit-trail.svelte.js` (new)

```js
const KEY = 'rc-visits'          // dashed rc-* convention (theme.svelte.js:13)
const MAX_ITEMS = 10             // hard storage cap; display trims further to fit viewport

export const visitTrail = $state({ entries: [] })
```

- Hydrate once from `sessionStorage[KEY]` inside try/catch; validate it's an
  array of `{path,label}`; in-memory fallback for private mode.
- `describe(path)` → null unless path has ≥2 segments. Only **data-entry
  navigations** are recorded (entity result pages like `/train/12951`,
  `/station/NDLS`); bare section hops (Home, Live, Board…) are ignored.
- `recordVisit(path)`:
  - normalize, then describe; null → no-op;
  - skip if identical to last entry (consecutive-dup dedupe);
  - label = pure sync resolver: first segment → section label from the same
    list as Layout items (Layout.svelte:37-47); second segment appended as a
    mono entity chip: `/train/12951` → `Live · 12951`, `/station/NDLS` →
    `Board · NDLS`. No async fetching, no fabricated names (LIVE DATA ONLY rule).
  - push `{path, label, ts}`, cap to MAX_ITEMS dropping oldest, persist to
    sessionStorage (try/catch, empty catch like recent.js:19-21).
- Initial load runs through the same gate — a deep-link landing on an entity
  page records itself; landing on `/` records nothing.

### File: `frontend/src/lib/router.svelte.js` (edit)

Call `recordVisit()` after both mutation points:
- inside `navigate()` after `route.path = ...` (line 10)
- inside the `popstate` handler (line 14-16)

This covers push navigation AND back/forward traversal.

---

## Phase 3: VisitTrail component + mount

### File: `frontend/src/lib/components/VisitTrail.svelte` (new)

- Sticky strip rendered inside Layout's `lg:pl-60` wrapper directly above
  `<main>` (Layout.svelte:199) — under the mobile header, clear of desktop sidebar.
- Structure: shadcn `Breadcrumb.List` with `flex-nowrap overflow-hidden` —
  **no horizontal scrolling**; the strip must always fit the viewport.
- Fit-to-width trimming: `$effect` resets `hidden=0`, then a rAF pass measures
  each crumb (`[data-crumb]`) plus its separator against the strip's
  `clientWidth`; while over budget it drops the **oldest** crumb from display
  and shows `Breadcrumb.Ellipsis`. A `ResizeObserver` on the strip re-runs the
  fit on viewport changes; newest crumb is never dropped.
- Each visible crumb = link routed through `navigate(entry.path)`; last crumb
  is non-link with `aria-current="page"` (breadcrumb a11y best practice).
  Labels truncate (`max-w-[9rem] sm:max-w-[14rem]`) so a single long crumb can
  never overflow alone.
- Hidden entirely when `entries.length <= 1`.
- Touch targets: `max-lg:h-11` like RecentSearches chips
  (RecentSearches.svelte:16); entity segment styled `font-mono` like
  RecentSearches.svelte:19.

### File: `frontend/src/lib/Layout.svelte` (edit)

Import and render `<VisitTrail />` between the drawer block and `<main>`
inside `<div class="lg:pl-60">`.

---

## Phase 4: Build & verify

```bash
cd railway-rs/frontend && npm run build   # prebuild check:imports runs first
```

Verification checklist (ui-layout-harness or manual):
1. Section hops only (Home → Live → Board) record nothing; trail stays hidden.
2. Entity navigations append: `/train/12951` → `Live · 12951`, then
   `/station/NDLS` appends `Board · NDLS`.
3. Browser back/forward to entity pages appends re-visits (it's a log).
4. Clicking an older crumb navigates and becomes the new head.
5. Reload keeps trail; new tab starts empty (sessionStorage semantics).
6. Consecutive duplicate nav does not add a chip.
7. On narrow viewports the strip NEVER scrolls horizontally: oldest crumbs
   collapse into an ellipsis, newest always visible, `docScrollW == winW`.
8. Trail hidden on fresh landing (≤1 entry).

**Untouched**: legacy vanilla SPA (`static/sections/`, frozen per skill),
server code, PLAN.md Phase 3 entity-breadcrumb scope.
