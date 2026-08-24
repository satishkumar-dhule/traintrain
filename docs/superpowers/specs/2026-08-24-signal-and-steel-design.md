# Signal & Steel — radical aesthetic upgrade (design spec)

Date: 2026-08-24 · Status: approved-by-directive (user mandated execution + fan-out)

## Brief

Radical, detailed aesthetics upgrade across **all** pages. Method constraints set by the user:
deep fan-out / deep delegation, DRY, KISS, TDD, reusable components.

## Diagnosis

The current UI is default shadcn/zinc: neutral grey tokens (hue 285), system fonts,
generic cards. Nothing is *of* Indian Railways. The fix is not decoration — it is an
identity drawn from the subject's own world.

## Direction: "Signal & Steel"

Indian Railways operations vernacular: coach indigo, signal lamps, platform signage type,
counterfoil data.

### Palette (oklch, both themes)

| Token | Light | Dark | Meaning |
|---|---|---|---|
| background | `oklch(0.975 0.005 255)` porcelain steel | `oklch(0.165 0.02 265)` night steel | page |
| card | `oklch(0.995 0.003 255)` | `oklch(0.21 0.024 265)` | surfaces |
| primary | `oklch(0.40 0.13 272)` coach indigo (white-on ok, AA ≥6) | `oklch(0.66 0.14 272)` (dark ink text) | identity/actions |
| accent/saffron | `oklch(0.74 0.18 65)` saffron, dark ink on it | same family brightened | highlight/brand warmth |
| --signal-go | `oklch(0.60 0.16 150)` | brightened | confirmed/on-time |
| --signal-hold | `oklch(0.72 0.17 70)` | brightened | RAC/waiting/amber delay |
| --signal-stop | `oklch(0.55 0.22 27)` | brightened | cancelled/severe |

All greys shift from neutral hue 285 to the blue-steel axis (258–265) so neutrals belong
to the palette. Charts re-keyed to indigo/saffron/teal/magenta/cyan.

### Typography

- **Archivo Variable** (`@fontsource-variable/archivo`, incl. `wdth` axis): display +
  UI face. Page titles and hero numerals use uppercase, tightened tracking, weight
  700–800 — platform-signage treatment.
- **JetBrains Mono Variable** (`@fontsource-variable/jetbrains-mono`): all data — train
  numbers, PNR, station codes, times, coordinates. Wired to `--font-mono` so existing
  `font-mono` utilities upgrade automatically.
- Two families total. KISS.

### Signature elements (boldness spent here, nowhere else)

1. **Track-rule**: a divider styled as rail ties (repeating-linear-gradient) under page
   headers — one `.track-rule` utility in app.css, used by PageHeader only.
2. **Signal lamps**: new `SignalDot.svelte` primitive (8px lamp in a signal color, gentle
   pulse for live states, disabled under reduced-motion) used by status badges/live rows.
3. **Counterfoil data**: station codes / train numbers always render through the existing
   central badge primitives (restyled once, centrally — DRY), never inline-styled per page.

## Architecture of the change (DRY/KISS)

- Radical look ≈ 80% token-level: pages already consume `bg-background`, `text-muted-*`,
  `border-border`, etc., so the palette flip restyles everything at once.
- Shared primitives (badges/, ui/, PageHeader, StatPill…) are restyled centrally; pages
  consume; no per-page hex/oklch literals ever.
- New reusable surface limited to what ≥2 pages need: `SignalDot`, plus token utilities.

## TDD gates

- New `tests/js/tokens.test.mjs`: pins the palette literals in `src/app.css` (both
  themes), asserts fontsource imports exist, asserts `--font-display/--font-mono` wired
  into `@theme`, asserts signal slots defined in `:root` AND `.dark`.
- Existing gates stay green: `npm run test:js`, frontend `build`, `check:imports`.
- Visual verification post-integration via browser screenshots (light+dark, mobile+desktop).

## Fan-out plan (delegation)

Foundation (serial, me): fonts, app.css, SignalDot, badge restyles, tokens test.
Then 6 parallel agents, disjoint file ownership (zero conflicts):

1. Home + About · 2. Train · 3. Station + Availability · 4. Pnr + Plan + JourneysTable ·
5. Assistant + System · 6. Extras + Exceptions

Agent rules: consume tokens/primitives only; no logic/API changes; keep a11y floor
(focus-visible, ≥44px touch targets, reduced-motion); run frontend build gate; report
diff summary. Integration (me) afterwards: Layout chrome (sidebar/header/tab bar/sheet),
PowerSearch/dialogs, bundle rebuild, full gates, screenshot sweep.
