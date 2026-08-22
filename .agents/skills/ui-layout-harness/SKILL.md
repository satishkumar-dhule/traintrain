---
name: ui-layout-harness
description: Run real-browser UI checks (screenshots, horizontal-overflow diagnostics, geometry probes) against localhost:3000 without installing anything on the host and without Docker. Use for "ui test", "screenshot the page", "check layout/overflow", "why is X cut off", "verify the redesign looks right", or any visual/layout verification of the Svelte frontend. Artifacts live under /tmp (disposable); the host stays untouched.
---

# UI layout harness (real browser, zero host installs)

Drives Chromium headless-shell via playwright-core to load pages served by the
running app (`rust-workflow` starts it on :3000), capture full-page
screenshots at several viewports, and report **horizontal-overflow offenders**
(elements sticking out of the viewport without a scrollable/clip ancestor).

Why not Docker/apt here: this sandbox has `REPLIT_DISABLE_DOCKER=1`, fake
`sudo` (uid unchanged), and apk landlock fails under the sandbox kernel. The
harness therefore assembles a private Debian library tree in /tmp and launches
the browser with `LD_LIBRARY_PATH` pointing at it. Nothing is installed on the
host; everything lives in `/tmp/opencode/ui-harness` and is safe to delete.

## 1. Bootstrap (idempotent, network needed once)

```bash
node .agents/skills/ui-layout-harness/scripts/setup.mjs
```

Downloads into `/tmp/opencode/ui-harness`: playwright-core, the
chromium-headless-shell binary (~110 MB), and the missing Debian bookworm
shared libraries (glib/nss/X11/fonts…) extracted into `debroot/`. Host glibc
is kept (Debian's libc copies are stripped to avoid GLIBC conflicts with nix
binaries).

## 2. Check pages

```bash
# Any route, default viewports, idle only:
node .agents/skills/ui-layout-harness/scripts/ui-check.mjs --path /availability

# With interactions (repeatable --step) and chosen viewports:
node .../ui-check.mjs --path /availability \
  --vp 1440x900,390x844 \
  --step 'fill:#av-from=NDLS' --step 'fill:#av-to=CNB' \
  --step 'click:text=Search' --step 'wait:5000' \
  --step 'shot:results' \
  --step 'click:text=Matrix' --step 'wait:600' --step 'shot:matrix'

# Full-page screenshot only:
node .../ui-check.mjs --path / --vp 1280x800 --full
```

Step syntax: `fill:<css>=<value>` · `click:text=<text>` · `wait:<ms>` ·
`shot:<name>` · `press:Enter`. Screenshots land in
`/tmp/opencode/ui-harness/shots/<name>-<viewport>.png`; read them back with the
Read tool (it renders images).

## 3. Read the output

Per viewport it prints one block:

```
== desktop-1440 ==
viewport 1440 | docScrollW 1440 | hOverflow false
  v-scroll: scrolled to 400 of 1200
```

- `docScrollW > winW` or `OFFENDER` lines ⇒ content escapes the visible range.
  Each offender lists tag/classes/edges so you can find the culprit class in
  source. An offender is reported only when NO ancestor scrolls/clips it, i.e.
  genuinely unreachable (matches `html,body{overflow-x:clip}` behavior).
- `v-scroll: scrolled to 0 of N` with N>0 would mean vertical scroll is broken.

## Conventions

- Never fix overflow by widening `overflow-x: clip` scopes — find the offender.
- After editing Svelte sources, rebuild so :3000 serves them:
  `(cd railway-rs/frontend && npm run build)` (hashes change; index.html is
  rewritten by Vite). Then rerun the check.
- Fonts come from `debroot` (Liberation); pixel-perfect text metrics may differ
  slightly from dev machines, layout math does not.
