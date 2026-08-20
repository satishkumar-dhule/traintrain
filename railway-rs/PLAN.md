# Railway-RS UI/UX Radical Rearchitecture

## Design Principles

1. **Senior-first**: Min 14px body text (15px target). WCAG AAA (7:1) contrast. Plain language. Fewer steps.
2. **Keyboard is primary**: Enter submits every form. Escape closes every modal. Tab navigates logically. No mouse required.
3. **Human-readable dates**: Never `2026-08-20`. Show "Today", "Tomorrow", "Wednesday". Timestamps → "2 min ago".
4. **Adaptive theme**: Automatic light/dark via `prefers-color-scheme`. Eye-comfortable on all screens.
5. **Color means something**: Colorblind-safe. Color never sole signal — always paired with icon/text/shape.
6. **Zero redundancy**: Every element earns its place. Progressive disclosure — summary first, detail on demand.
7. **Connected graph**: Every entity reachable from any other in ≤2 clicks.

---

## 1. Page Architecture (5 Pages)

| # | Page | Route | Pill Tabs |
|---|------|-------|-----------|
| 1 | **Dashboard** | `#/` | *(none — PNR inline, recent, quick actions)* |
| 2 | **Train** | `#/train/{num}[/{view}]` | Spot · Schedule · Map · Delay · Exceptions · Journey |
| 3 | **Station** | `#/station/{code}[/{view}]` | Live · Timetable · Heritage · Parcel |
| 4 | **Plan** | `#/plan/{src}/{dst}[/{view}]` | Trains · Availability · Chart |
| 5 | **System** | `#/system/[{view}]` | Observability · Settings · Debug |

### Relocations

| Current Location | New Home | Rationale |
|-----------------|----------|-----------|
| PNR section (`sections/pnr.js`) | Dashboard (inline card) | Quick-check action, not a destination |
| Heritage (`more.js`) | Station, "Heritage" pill | Station-origin data |
| Parcel (`more.js`) | Station, "Parcel" pill | Station-based data |
| Stations search | Global autocomplete only | Already in shell search |
| System settings | System, "Settings" pill | Groups with observability |
| Debug (`more.js`) | System, "Debug" pill | System tool |
| Observability | System, "Observability" pill | System dashboard |

### Files Deleted

- `static/sections/more.js`
- `static/sections/pnr.js`
- `static/tabs/observability.js`
- `static/tabs/train_on_map.js`

---

## 2. Adaptive Theme (`styles.css`)

Replace 13 hardcoded tokens with `light-dark()`:

```css
:root {
  color-scheme: light dark;
  --text:       light-dark(#0f172a, #f1f5f9);
  --text-soft:  light-dark(#334155, #cbd5e1);
  --muted:      light-dark(#475569, #94a3b8);
  --surface:    light-dark(#ffffff, #0f172a);
  --surface-muted: light-dark(#f8fafc, #1e293b);
  --surface-soft:  light-dark(#f1f5f9, #334155);
  --border:     light-dark(#e2e8f0, #475569);
  --border-strong: light-dark(#cbd5e1, #64748b);
  --primary:    light-dark(#1d4ed8, #60a5fa);
  --primary-100: light-dark(#dbeafe, #1e3a5f);
  --primary-50:  light-dark(#eff6ff, #172554);
  --success:    light-dark(#047857, #34d399);
  --success-bg: light-dark(#d1fae5, #064e3b);
  --danger:     light-dark(#b91c1c, #f87171);
  --danger-bg:  light-dark(#fee2e2, #7f1d1d);
  --amber:      light-dark(#92400e, #fbbf24);
  --amber-bg:   light-dark(#fef3c7, #78350f);
  --accent:     light-dark(#ea580c, #fb923c);
  --accent-100: light-dark(#ffedd5, #7c2d12);
  --shadow-sm: 0 1px 2px light-dark(rgba(15,23,42,.06), rgba(0,0,0,.3));
  --shadow-md: 0 4px 14px light-dark(rgba(15,23,42,.08), rgba(0,0,0,.4));
  --focus-ring: 0 0 0 3px light-dark(rgba(37,99,235,.25), rgba(96,165,250,.35));
}
```

Support for forced colors and reduced motion:
```css
@media (forced-colors: active) {
  .btn, .pill, .entity-link, .nav-item { border: 2px solid ButtonText; }
  .badge { border: 1px solid ButtonText; }
}
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

**Eliminated**: `--faint` (#94a3b8) — fails 4.5:1 contrast. All its usages replaced with `--muted`.

---

## 3. Typography (Senior-First)

| Token | Size | Use |
|-------|------|-----|
| `--fs-xs` | 12px | Badges, table headers (minimum) |
| `--fs-sm` | 13px | Labels, pills, notices |
| `--fs-base` | 15px | Body, inputs, buttons |
| `--fs-md` | 16px | Card titles |
| `--fs-lg` | 20px | Section titles |
| `--fs-xl` | 24px | Page title (dashboard) |

**10px font eliminated** — was used for nav groups, KPI labels, chart titles. All bumped to 12px+.

---

## 4. Color Coding System

| Color | Meaning | Used for |
|-------|---------|----------|
| Blue | Navigation, links, entities, active states | Pill tabs, entity links, codes, primary buttons |
| Green | On-time, vacant, available, positive | Delay OK, berth vacant, source up |
| Red | Late, occupied, cancelled, error | Delay late, berth taken, cancelled trains |
| Amber | Warning, rescheduled, diverted | Rescheduled/diverted, warnings |
| Slate | Neutral, metadata | Source badges, cache info, inactive |

**Status badges** (consistent everywhere):
- `ON TIME` green + checkmark
- `N min LATE` red + clock
- `DEPARTED` slate
- `EXPECTED` amber
- `CANCELLED` red + X
- `RESCHEDULED` amber + refresh
- `VACANT` green circle / `TAKEN` red circle

---

## 5. Human-Readable Dates

New helpers in `ui.js`:

```javascript
friendlyDate(dateStr)   // "Today", "Tomorrow", "Wednesday", "Wed, 20 Aug"
friendlyTime(isoStr)    // "just now", "2 min ago", "1 hr ago"
```

**Date picker** in Plan gets quick-select buttons: `[Today] [Tomorrow] [Day after] [Pick a date]`

---

## 6. Keyboard-First

| Key | Action | Scope |
|-----|--------|-------|
| Enter | Submit current form | ALL inputs |
| Escape | Close modal/autocomplete/blur | Palette, captcha, autocomplete |
| Tab | Next interactive element | Logical order throughout |
| Shift+Tab | Previous element | Reverse order |
| ArrowUp/Down | Navigate lists | Autocomplete, pill tabs |

**Fixes**: Captcha input gets Enter handler. Journey select gets Enter handler.

---

## 7. Command Palette (Cmd+K)

Smart query parsing — no AI, pure regex:

| Input | Result |
|-------|--------|
| `12559` | Train spot |
| `12559 delay` | Train delay view |
| `NDLS` | Station live |
| `NDLS timetable` | Station tt view |
| `NDLS MUM` | Plan trains |
| `2498761234` | PNR check |
| `observability` | System obs |

---

## 8. Cross-Entity Linking

Every result table gets clickable entity badges. Every result gets a "What next?" contextual action bar.

| From | Clickable | To |
|------|-----------|-----|
| Train station row | Station code badge | `#/station/{code}` |
| Station train row | Train number badge | `#/train/{number}` |
| Station header | "Plan from here" | `#/plan/{station}/` |
| Plan train row | Train number badge | `#/train/{number}` |
| PNR train info | Train badge | `#/train/{number}` |
| PNR stations | Station badges | `#/station/{code}` |

---

## 9. Space Efficiency

- **One-line entity header**: `🚄 12559 — Rajdhani Express [NTES] [cached 2m]`
- **Collapsible tables**: Show 10 rows, "Show all N" toggle
- **Inline metadata**: Source + freshness + cache as small badges on one row
- **No duplicate headers**: Entity name in pill bar, not repeated in sub-cards

---

## 10. Route Table

```javascript
const SECTIONS = {
  home:    { views: [] },
  train:   { views: ['spot','schedule','map','delay','exceptions','journey'],
             defaultView: 'spot', params: ['train'] },
  station: { views: ['live','tt','heritage','parcel'],
             defaultView: 'live', params: ['station'] },
  plan:    { views: ['trains','availability','chart'],
             defaultView: 'trains', params: ['src','dst'] },
  system:  { views: ['observability','settings','debug'],
             defaultView: 'observability' },
};
```

---

## 11. All Deep Links

| URL | Result |
|-----|--------|
| `#/` | Dashboard |
| `#/pnr/2498761234` | Dashboard, PNR auto-checked |
| `#/train/12559` | Train, spot |
| `#/train/12559/schedule` | Train, schedule |
| `#/train/12559/map` | Train, map |
| `#/train/12559/delay` | Train, delay |
| `#/train/12559/exceptions` | Train, exceptions |
| `#/train/12559/journey` | Train, journey |
| `#/station/NDLS` | Station, live |
| `#/station/NDLS/tt` | Station, timetable |
| `#/station/NDLS/heritage` | Station, heritage |
| `#/station/NDLS/parcel` | Station, parcel |
| `#/plan/NDLS/BSB` | Plan, trains |
| `#/plan/NDLS/BSB/availability` | Plan, availability |
| `#/plan/NDLS/BSB/chart` | Plan, chart |
| `#/system` | System, observability |
| `#/system/settings` | System, settings |
| `#/system/debug` | System, debug |

---

## 12. File Changes Summary

### New files
| File | Purpose |
|------|---------|
| `static/palette.js` | Command palette (Cmd+K) + smart parser |
| `static/sections/system.js` | System page (obs+settings+debug) |

### Modified files
| File | Changes |
|------|---------|
| `static/styles.css` | `light-dark()` tokens, min 12px fonts, no `--faint`, pill-bar, entity-link, skeleton, palette, contextual-action, forced-colors, reduced-motion |
| `static/ui.js` | `entityLink()`, `skeleton()`, `friendlyDate()`, `friendlyTime()`, `dateQuickPick()`, `collapsibleTable()`, `contextualActions()` |
| `static/routes.js` | New SECTIONS (5 pages), NAV_ORDER, system routes |
| `static/app.js` | New NAV, palette init, captcha Enter fix, focus management, remove More |
| `static/index.html` | Add palette.js, update search markup |
| `static/sections/home.js` | Complete rewrite (Dashboard + inline PNR) |
| `static/sections/track.js` | Cross-links, entity header, pill bar, friendly dates, collapsible tables |
| `static/sections/station.js` | Add heritage/parcel tabs, cross-links, friendly dates |
| `static/sections/plan.js` | Cross-links, date quick-pick, friendly dates, collapsible tables |

### Deleted files
| File | Reason |
|------|--------|
| `static/sections/more.js` | Views relocated |
| `static/sections/pnr.js` | Inlined into Dashboard |
| `static/tabs/observability.js` | Inlined into System |
| `static/tabs/train_on_map.js` | Inlined into Train/map |

---

# Vertical Slice Tracking

## Phase 1: CSS Foundation

**Goal**: Adaptive theme works in light+dark. All fonts ≥12px. No `--faint`. Reduced motion + forced colors support.

### Slice 1.1: Token rework
- [ ] Replace `:root` color tokens with `light-dark()` function calls
- [ ] Add `color-scheme: light dark` to `:root`
- [ ] Remove `--faint` token entirely
- [ ] Verify every `--faint` usage is replaced with `--muted`
- [ ] Bump `--muted` to `#475569` (light) / `#94a3b8` (dark) for AAA contrast

### Slice 1.2: Typography
- [ ] Add `--fs-xs` through `--fs-xl` tokens
- [ ] Set body `font-size: var(--fs-base)` (15px)
- [ ] Remove all 10px font-size declarations
- [ ] Bump all 11px to 12px minimum
- [ ] Update `.nav-item`, `.input`, `.btn` to use `--fs-base`
- [ ] Update `.card h2, h3` to use `--fs-md`
- [ ] Update `.badge` to use `--fs-xs`
- [ ] Update `.section-pill` to use `--fs-sm`
- [ ] Update `.notice`, `.label` to use `--fs-sm`

### Slice 1.3: Forced colors + reduced motion
- [ ] Add `@media (forced-colors: active)` rules for buttons, badges, pills, links
- [ ] Add `@media (prefers-reduced-motion: reduce)` rules
- [ ] Test with Windows High Contrast mode simulation

### Slice 1.4: Contrast audit
- [ ] Verify `--text` vs `--surface` ≥ 7:1 in both themes
- [ ] Verify `--muted` vs `--surface` ≥ 4.5:1 in both themes
- [ ] Verify `--primary` vs `--surface` ≥ 4.5:1 in both themes
- [ ] Verify all badge colors pass 4.5:1 on their backgrounds
- [ ] Verify chart colors against Okabe-Ito palette

**Verification**: Open `index.html` in browser. Toggle OS light/dark mode. All text readable. No contrast failures.

---

## Phase 2: Route + Nav Restructure

**Goal**: 5-page routing works. No "More" section.

### Slice 2.1: Route table
- [ ] Update `routes.js` SECTIONS to 5 sections (home, train, station, plan, system)
- [ ] Add system views: `['observability', 'settings', 'debug']`
- [ ] Add station views: `['live', 'tt', 'heritage', 'parcel']`
- [ ] Update NAV_ORDER to `['home', 'train', 'station', 'plan', 'system']`
- [ ] Add route regex for `#/system[/{view}]`
- [ ] Remove `more` section from SECTIONS
- [ ] Remove all `#/more/*` route regexes

### Slice 2.2: Navigation
- [ ] Update `app.js` SECTIONS array to 5 items
- [ ] Remove MORE array entirely
- [ ] Update `buildNav()` — remove "More" group from sidebar
- [ ] Update `buildNav()` for mobile — 5 tabs, no "More" tab
- [ ] Update `updateNav()` — remove `data-more` handling
- [ ] Update `buildSectionHeader()` — remove `more` from entity label logic

### Slice 2.3: Route migration
- [ ] Update all internal navigation calls from `#/more/observability` to `#/system/observability`
- [ ] Update all internal navigation calls from `#/more/stations` to removed (use search)
- [ ] Update all internal navigation calls from `#/more/system` to `#/system/settings`
- [ ] Update all internal navigation calls from `#/more/debug` to `#/system/debug`
- [ ] Update all internal navigation calls from `#/more/heritage` to `#/station/{code}/heritage`
- [ ] Update all internal navigation calls from `#/more/parcel` to `#/station/{code}/parcel`
- [ ] Update home.js tile links to new routes

**Verification**: Navigate to every deep link in the table above. Each resolves correctly.

---

## Phase 3: Pill Tab System

**Goal**: Entity pages show horizontal pill tabs. Sticky on scroll.

### Slice 3.1: Pill bar CSS
- [ ] Add `.pill-bar` CSS (sticky, flex, wrap)
- [ ] Add `.pill` CSS (border-radius-pill, transitions, focus-visible)
- [ ] Add `.pill.active` CSS (primary fill + shadow)
- [ ] Add `.entity-breadcrumb` CSS

### Slice 3.2: Pill bar JS helper
- [ ] Add `ui.pillBar(views, labels, activeView, onSwitch)` helper to `ui.js`
- [ ] Returns a DOM element with pill buttons
- [ ] Each pill calls `onSwitch(viewName)` on click
- [ ] Active pill gets `.active` class
- [ ] Pills are keyboard-focusable (natural tab order)

### Slice 3.3: Section header rework
- [ ] Update `buildSectionHeader()` in `app.js` to use `ui.pillBar()`
- [ ] Add entity breadcrumb above pills
- [ ] Entity name/badge shown in one line above pills
- [ ] Sticky positioning (stays visible when scrolling results)

### Slice 3.4: Train page integration
- [ ] Mount pill bar at top of `sections/track.js`
- [ ] Each pill navigates to the correct view via `Routes.href()`
- [ ] Active pill matches `route.view`
- [ ] Pill bar renders before the view content

### Slice 3.5: Station + Plan page integration
- [ ] Same pill bar pattern in `sections/station.js`
- [ ] Same pill bar pattern in `sections/plan.js`
- [ ] Station page gets 4 pills: Live, Timetable, Heritage, Parcel
- [ ] Plan page gets 3 pills: Trains, Availability, Chart

**Verification**: On a train page, click each pill — view switches, URL updates, pill highlights. Scroll down — pill bar stays sticky.

---

## Phase 4: System Page

**Goal**: New `sections/system.js` replaces `sections/more.js` and `tabs/observability.js`.

### Slice 4.1: System page shell
- [ ] Create `static/sections/system.js`
- [ ] Register as `window.Sections.system`
- [ ] Route: `#/system` → observability (default)
- [ ] Pill bar with 3 tabs: Observability, Settings, Debug
- [ ] Switch views based on `route.view`

### Slice 4.2: Observability tab
- [ ] Port all code from `tabs/observability.js` into `sections/system.js`
- [ ] KPI gauges, charts, status tables, log viewer
- [ ] Update CSS class references (ensure `obs-*` classes still work)
- [ ] Auto-refresh behavior preserved
- [ ] Delete `tabs/observability.js`

### Slice 4.3: Settings tab
- [ ] Port `viewSystem()` from `sections/more.js` into `sections/system.js`
- [ ] Data mode, live enabled, cache TTL, primary source
- [ ] Source reachability table
- [ ] Verification links

### Slice 4.4: Debug tab
- [ ] Port `viewDebug()` from `sections/more.js` into `sections/system.js`
- [ ] Client-side log viewer
- [ ] Actions: refresh, copy, download, send to server, clear
- [ ] System info display

### Slice 4.5: Cleanup
- [ ] Delete `sections/more.js`
- [ ] Remove all `window.Tabs` delegation patterns
- [ ] Remove `window.Sections.more` registration
- [ ] Update `index.html` — remove `<script>` tags for deleted files

**Verification**: `#/system/observability` shows full dashboard. `#/system/settings` shows source status. `#/system/debug` shows log viewer. All 3 tabs switch correctly.

---

## Phase 5: Station Expansion + Cross-Links

**Goal**: Station page has 4 tabs. Cross-entity linking everywhere.

### Slice 5.1: Station heritage tab
- [ ] Port `viewHeritage()` from `more.js` into `sections/station.js`
- [ ] Heritage selector dropdown + submit
- [ ] Auto-fetch on mount when tab is active
- [ ] Train results show clickable train number badges

### Slice 5.2: Station parcel tab
- [ ] Port `viewParcel()` from `more.js` into `sections/station.js`
- [ ] Auto-fetch on mount when tab is active
- [ ] Refresh button

### Slice 5.3: Cross-link helper
- [ ] Add `ui.entityLink(type, code, label, navigate)` to `ui.js`
- [ ] Add `.entity-link` CSS (blue pill, hover lift, focus ring)
- [ ] Add `ui.contextualActions(entity, navigate)` to `ui.js`
- [ ] Add `.contextual-actions` CSS

### Slice 5.4: Station cross-links
- [ ] Live board: train number cells become clickable `entityLink('train', number)`
- [ ] Timetable: train number cells become clickable
- [ ] Header: "Plan from here" button navigates to `#/plan/{station}/`
- [ ] Heritage: train number cells become clickable

### Slice 5.5: Train cross-links
- [ ] Spot stations: station code cells become clickable `entityLink('station', code)`
- [ ] Schedule: station code cells become clickable
- [ ] Map: station table code cells become clickable
- [ ] Journey basis: station code cells become clickable

### Slice 5.6: Plan cross-links
- [ ] Trains list: train number cells become clickable
- [ ] Availability: train number cells become clickable
- [ ] Chart: train number badge becomes clickable

### Slice 5.7: Contextual actions
- [ ] Train results get: Delay, Map, Schedule action buttons
- [ ] Station results get: Plan from here, Timetable action buttons
- [ ] PNR results get: Spot train, From station, To station buttons

**Verification**: Click a station code in Train results → navigates to Station page. Click a train in Station results → navigates to Train page. Contextual action buttons appear below every result.

---

## Phase 6: Dashboard Rewrite

**Goal**: Dashboard replaces Home. PNR inline. Smart recent. Date quick-pick.

### Slice 6.1: Dashboard shell
- [ ] Rewrite `sections/home.js` as Dashboard
- [ ] Remove old tile grid (6 tiles → 3 quick-action buttons)
- [ ] Add live mode dot in header
- [ ] Add search bar trigger for command palette

### Slice 6.2: Inline PNR
- [ ] Port PNR fetch + captcha logic from `sections/pnr.js` into `sections/home.js`
- [ ] PNR input card with 10-digit validation
- [ ] Passenger results rendering
- [ ] Captcha retry flow (3 attempts)
- [ ] Auto-submit when `#/pnr/{10digits}` deep linked
- [ ] Delete `sections/pnr.js`

### Slice 6.3: Smart recent
- [ ] Recent items show entity type icon (train/station/plan/pnr)
- [ ] Recent items show relative timestamp ("2 min ago")
- [ ] Recent items store `view` alongside `label` and `hash`
- [ ] Clicking recent returns to exact tab user was on

### Slice 6.4: System status card
- [ ] Compact card showing live mode + primary source
- [ ] Source reachability as inline badges
- [ ] Auto-refreshes on mount

### Slice 6.5: Date quick-pick
- [ ] Add `ui.dateQuickPick(onSelect)` helper
- [ ] Renders [Today] [Tomorrow] [Day after] buttons
- [ ] Used in Plan availability and chart views
- [ ] Replaces raw `<input type="date">` as primary interaction
- [ ] Date picker still available as fallback

**Verification**: Dashboard loads. PNR check works. Recent shows icons + relative times. Date quick-pick in Plan shows Today/Tomorrow buttons.

---

## Phase 7: Command Palette

**Goal**: Cmd+K opens a centered modal with search, recent, quick actions, smart parsing.

### Slice 7.1: Palette shell
- [ ] Create `static/palette.js`
- [ ] Fixed-position modal overlay
- [ ] Input field with autofocus
- [ ] Three sections: Recent, Quick Actions, Results
- [ ] Keyboard: ArrowUp/Down navigate, Enter selects, Esc closes

### Slice 7.2: Smart parser
- [ ] Implement `parseQuery(q)` function
- [ ] PNR detection (10 digits)
- [ ] Plan detection (two station codes with separator)
- [ ] Train + view detection ("12559 delay")
- [ ] Station + view detection ("NDLS timetable")
- [ ] System command detection ("observability", "debug")
- [ ] Fallback to search

### Slice 7.3: Integration
- [ ] Wire Cmd+K / Ctrl+K listener in `app.js`
- [ ] Wire search bar click to open palette on desktop
- [ ] Wire palette selection to `navigate()` calls
- [ ] Palette closes on navigation
- [ ] Focus returns to previous element on close

### Slice 7.4: Palette CSS
- [ ] Modal overlay (backdrop blur, semi-transparent)
- [ ] Centered card (max-width 520px)
- [ ] Section headers (RECENT, ACTIONS, RESULT)
- [ ] Item rows (icon + text, hover highlight)
- [ ] Keyboard highlight state

**Verification**: Press Cmd+K. Type "12559 delay". Result shows "Train 12559 → Delay". Press Enter → navigates to `#/train/12559/delay`. Press Esc → closes.

---

## Phase 8: Polish

**Goal**: Skeletons, friendly dates, collapsible tables, keyboard audit, final QA.

### Slice 8.1: Skeleton loaders
- [ ] Add `ui.skeleton(rows)` helper
- [ ] Add `.skeleton` CSS (shimmer animation)
- [ ] Replace `ui.spinner()` calls in fetch flows with skeletons
- [ ] Crossfade from skeleton to content

### Slice 8.2: Friendly dates
- [ ] Add `ui.friendlyDate(dateStr)` — "Today", "Tomorrow", "Wednesday"
- [ ] Add `ui.friendlyTime(isoStr)` — "2 min ago", "1 hr ago"
- [ ] Replace all raw date displays across all sections
- [ ] Train instance dates → friendly
- [ ] PNR journey date → friendly
- [ ] Plan availability/chart dates → friendly
- [ ] PNR freshness → relative time
- [ ] Observability timestamps → HH:MM format

### Slice 8.3: Collapsible tables
- [ ] Add `ui.collapsibleTable(headers, rows, maxVisible)` helper
- [ ] Show 10 rows by default, "Show all N rows" toggle
- [ ] Apply to: Train schedule, Train delay, Station timetable, Plan trains, Station heritage, Station parcel

### Slice 8.4: Keyboard audit
- [ ] Tab through every interactive element on every page
- [ ] Verify Enter submits every form
- [ ] Verify Escape closes palette, captcha, autocomplete
- [ ] Verify focus-visible ring on every interactive element
- [ ] Fix any focus traps or lost focus

### Slice 8.5: Micro-interactions
- [ ] Pill tab switch: 150ms transition
- [ ] Entity link hover: translateY(-1px) lift
- [ ] Card appear: fade-in 200ms
- [ ] Skeleton → content: crossfade

### Slice 8.6: index.html cleanup
- [ ] Remove `<script>` tags for deleted files (more.js, pnr.js, observability.js, train_on_map.js)
- [ ] Add `<script>` tag for palette.js
- [ ] Verify script load order is correct

### Slice 8.7: Final QA
- [ ] Every deep link in the table resolves correctly
- [ ] Every form submits with Enter
- [ ] Light mode: all text readable
- [ ] Dark mode: all text readable
- [ ] Tab navigation works on every page
- [ ] No duplicate information across cards
- [ ] Every pixel earns its place
- [ ] Run `node --check` on all JS files for syntax
- [ ] Verify no broken `window.Tabs` or `window.Sections` references

**Verification**: Full walkthrough of every page, every tab, every deep link. Keyboard-only navigation. Light and dark mode. Mobile and desktop.
