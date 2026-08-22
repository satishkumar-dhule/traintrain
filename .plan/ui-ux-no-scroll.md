# UI/UX Revamp: Eliminate Scrolling

## Context

Every view in the app scrolls on mobile (iPhone SE: 504px usable height). The shell chrome (header ~114px, pill bar 77-127px, input card ~160px) consumes 49-63% of the viewport before any content renders. Worst offenders: Observability at ~3,666px (7.3x viewport), Train Spot at ~1,227px (2.4x), Home at ~776px (1.5x).

**Goal**: Make the page body never scroll. The `.main` area becomes the sole scroll container, and each view is redesigned to minimize vertical space.

---

## Phase 1: App Shell — Fixed Viewport Layout

Convert from body-scroll to app-shell pattern. The `.shell` becomes a fixed-height flex column; `.main` becomes the internal scroll container.

### File: `static/styles.css`

```css
/* BEFORE */
.shell { min-height: 100vh; }
.layout { display: flex; min-height: 100vh; }
.main { flex: 1; width: 100%; max-width: 900px; margin: 0 auto; padding: 16px 12px 90px; }

/* AFTER */
.shell { height: 100vh; display: flex; flex-direction: column; overflow: hidden; }
.layout { display: flex; flex: 1; overflow: hidden; }
.main { flex: 1; width: 100%; max-width: 900px; margin: 0 auto; padding: 12px 12px 12px; overflow-y: auto; overscroll-behavior: contain; }
```

Desktop override:
```css
@media (min-width: 768px) {
  .main { padding: 16px 20px 24px; }
}
```

Mobile header: reduce height by tightening padding and font.
```css
.mobile-header { padding: 8px 12px; gap: 8px; }
.brand { font-size: 14px; }
```

### File: `static/index.html`

No structural changes needed — CSS-only.

---

## Phase 2: Home Dashboard — Grid Layout

Use a 2-column grid on desktop, compact cards on mobile.

### File: `static/sections/home.js`

**Changes**:
1. Remove the title "RailCompanion" and subtitle (redundant with header branding) — replace with a compact greeting
2. Quick actions: render as a horizontal row of icon-buttons instead of tall tiles
3. PNR card: compact (inline input + button, no description text)
4. Recent + Status: render side-by-side in a `.grid.grid-2` wrapper
5. All cards use `.card-sm` (new compact card class)

**New compact card class in styles.css**:
```css
.card-sm { padding: 12px; margin-bottom: 8px; }
```

**Compact tile**:
```css
.tile { min-height: 56px; padding: 10px 12px; flex-direction: row; align-items: center; gap: 8px; }
```

---

## Phase 3: Track Section — Compact Form + Scrollable Results

### File: `static/styles.css`

**Horizontal pill bar** (no wrapping):
```css
.pill-bar { overflow-x: auto; flex-wrap: nowrap; -webkit-overflow-scrolling: touch; scrollbar-width: none; }
.pill-bar::-webkit-scrollbar { display: none; }
.pill { white-space: nowrap; flex-shrink: 0; min-height: 36px; padding: 8px 14px; }
```

### File: `static/sections/track.js`

**Changes**:
1. Inline the train input with the Search button (row layout, not stacked)
2. Description text: hide or show as a tooltip/summary only
3. Results area: scrollable within its own container (already handled by Phase 1 `.main` scroll)

---

## Phase 4: Station + Plan Sections — Compact Forms

### File: `static/sections/station.js`

**Changes**:
1. Merge header card into query card (remove separate "Live Station" description card)
2. Use compact query form: station code + hours inline in one row
3. Results: collapsible tables already help — ensure maxVisible stays at 10

### File: `static/sections/plan.js`

**Changes**:
1. Merge header card into query card
2. Train/availability/chart forms: compact inline layout
3. From + To stations: side-by-side in a row (not stacked)

---

## Phase 5: Observability — Maximum Space Savings

This is the heaviest view (~3,666px). Major restructuring needed.

### File: `static/sections/system.js`

**Changes**:
1. **Gauge cards**: reduce canvas size — change `aspectRatio: 2` to `aspectRatio: 2.5`, reduce `obs-chart-box` height from 200px to 140px
2. **Charts**: reduce height from 200px to 140px
3. **Doughnut**: reduce from 190px to 140px
4. **Stats grid**: already compact, keep as-is
5. **Tables**: use smaller font (`--fs-xs`), tighter row padding
6. **Logs panel**: reduce `max-height` from 420px to 300px
7. **Collapsible sections**: wrap charts, tables, and logs in `<details><summary>` elements so they're collapsed by default

### File: `static/styles.css`

```css
.obs-chart-box { height: 140px; } /* was 200px */
.obs-doughnut-box { height: 140px; } /* was 190px */
.obs-log-panel { max-height: 300px; } /* was 420px */
.obs-kpi-card { padding: 8px; }
.obs-kpi-label { margin-bottom: 4px; }
.obs-chart-card { padding: 8px; }
.obs-tables-grid .tbl td { padding: 5px 6px; }
```

---

## Phase 6: System Settings + Debug — Compact

### File: `static/sections/system.js`

**Settings**: Already reasonably compact. Merge some cards (Data Mode + Live Sources into one card).

**Debug**: Reduce textarea height from `55vh` to `40vh`. Remove redundant button (combine "Copy log" + "Download" into one row).

---

## Phase 7: Table Density (Global)

### File: `static/styles.css`

```css
.tbl td { padding: 6px 8px; } /* was 8px */
.tbl th { padding: 6px 8px; font-size: 11px; } /* was 12px */
.tbl { font-size: 12px; } /* was 13px */
```

---

## Files to Modify

| File | Changes |
|---|---|
| `static/styles.css` | App shell fix, compact cards, pill-bar nowrap, table density, obs height reductions |
| `static/sections/home.js` | Grid layout, compact cards, remove redundant title |
| `static/sections/track.js` | Inline form, compact description |
| `static/sections/station.js` | Merged header+form, compact layout |
| `static/sections/plan.js` | Merged header+form, compact layout |
| `static/sections/system.js` | Obs chart reductions, collapsible sections, compact settings/debug |
| `static/index.html` | No changes (CSS-only shell fix) |

---

## Verification

1. Open on iPhone SE viewport (375x667): `#home`, `#train/12559`, `#station/NDLS`, `#plan/NDLS/BSB`, `#system/observability`
2. Verify page body never scrolls (no `document.body.scrollTop` or `document.documentElement.scrollTop` changing)
3. Verify `.main` area scrolls internally for long content (train stations, obs dashboard)
4. Verify all pill bars are horizontally scrollable, not wrapping
5. Run `cargo test` to ensure no regressions in backend tests
6. Test dark mode — all new styles must use `light-dark()` tokens
