# Real-browser UI test suite

The app's always-on UI suite. It drives the **actual SPA served by the axum
binary** in a real headless Chromium and pins the behavioral contract every
change must keep. Runs as part of `make check-js` (which CI executes), so no
regression reaches `main` without passing through a browser.

```bash
npm run test:ui      # just the UI suite
make check-ui        # same thing
make check-js        # parse + unit suites + UI suite (what CI runs)
```

## What is pinned (the contract)

| #   | Contract                                                                    | Where                    |
| --- | --------------------------------------------------------------------------- | ------------------------ |
| 1   | `/healthz` answers `{status:"ok", service:"railway-rs"}`                     | `healthz reports ok`     |
| 2   | Every nav route renders exactly one `<h1>` plus real content or live controls | `route …` loop          |
| 3   | Nothing escapes the viewport horizontally at desktop **and** phone widths    | `assertNoHorizontalOverflow` |
| 4   | Vertical scrolling works wherever content exceeds the viewport               | `assertVerticalScrollWorks` |
| 5   | Form controls and icon-only buttons carry accessible names                   | `accessible names`       |
| 6   | Track flow always resolves to data **or** an honest error — never blank/stuck | `train flow`            |
| 7   | Theme choice applies `html.light/dark` and survives reload (`rc-theme`)      | `theme choice`           |
| 8   | Zero uncaught JS exceptions; no app-level `console.error` (network noise allowed) | per-route assertions |

The overflow model matches the CSS contract of the app: `html/body` clip
x-overflow, so an *offender* is an element that escapes the viewport without a
scrollable/clip ancestor — i.e. genuinely unreachable content on real devices.
Never widen clip scopes to silence it; fix the offender.

## Architecture

```
tests/ui/
├── ui.test.mjs        the spec (read this first)
└── _lib/              reusable framework — specs never touch playwright directly
    ├── env.mjs        harness paths, BASE_URL, viewport matrix, route list
    ├── server.mjs     ensureApp(): reuse running :3000 or start binary from target/
    ├── browser.mjs    sharedBrowser()/openPage(): isolated context per test,
    │                  console + pageerror capture, auto-wait for <main>
    ├── layout.mjs     diagnose() + assertion library (overflow, scroll,
    │                  a11y names, console hygiene)
    └── index.mjs      single import point
```

Browser runtime: `playwright-core` + `chromium-headless-shell` live under
`/tmp/opencode/ui-harness` (private Debian lib tree, zero host installs).
First run bootstraps via `../.agents/skills/ui-layout-harness/scripts/setup.mjs`
(idempotent). If provisioning is impossible the suite **skips loudly**;
set `UI_STRICT=1` to fail instead (recommended on CI runners that should
never silently lose UI coverage).

## Configuration (env)

| Variable         | Default                          | Purpose                       |
| ---------------- | -------------------------------- | ----------------------------- |
| `UI_PORT`        | `3000`                           | Port the app is probed on     |
| `UI_BASE_URL`    | `http://localhost:$UI_PORT`      | Target an already-running app |
| `UI_VIEWPORTS`   | `1440x900:desktop,390x844:mobile`| Comma-separated matrix        |
| `UI_ROUTES`      | all 12 nav routes                | Routes toured by contract #2  |
| `UI_HARNESS_HOME`| `/tmp/opencode/ui-harness`       | Browser runtime location      |
| `UI_STRICT`      | unset                            | `1` = fail instead of skip when browser missing |

Server lifecycle: `ensureApp()` probes `/healthz` and reuses a healthy
server; only if absent does it spawn `target/release` (or debug) binary as a
detached process (log: `/tmp/railway-ui-test-server.log`). The suite kills
only servers it started.

## Extending TDD-style

1. **RED** — add the failing spec first in `ui.test.mjs` using the `_lib`
   helpers (`openPage`, assertions). Run `npm run test:ui` and watch it fail
   for the right reason.
2. **Probe** — write a throwaway script under `/tmp/opencode` importing
   `_lib/browser.mjs` to dump DOM/text when you need evidence to decide:
   app bug → fix Svelte source; wrong expectation → calibrate the spec with
   a comment explaining why.
3. **GREEN** — fix the app (`frontend/src`), rebuild so :3000 serves it:
   `(cd frontend && npm run build)`, rerun until green.
4. **Gate** — `make check-js`; commit spec + fix together.

Rules of thumb:

- Assertions about *product intent* go in specs; tolerance/policy (e.g. which
  console messages are ignorable network noise) lives in `_lib/layout.mjs`
  with a reason comment.
- One test per concern keeps failures localized; tour viewports inside the
  test rather than duplicating it per viewport.
- Never fabricate data expectations from upstream (NTES etc.) — assert
  honesty (data **or** explicit error), matching the app's live-data rule.

## Troubleshooting

- `no headless_shell` → run the setup script (see above).
- Suite hangs → a page kept a fetch alive; ensure your probe scripts call
  `closeBrowser()` (the suite itself does in `test.after`).
- Overflow offender listed → open the printed tag/class in DevTools at that
  viewport; the class string maps 1:1 to Tailwind classes in source.
