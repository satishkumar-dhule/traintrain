---
name: spa-tab
description: Add or fix a vanilla-JS tab in railway-rs/static/tabs/ (the SPA frontend). Use for "add a tab", "build the frontend for X feature", "fix UI of the X tab", "observability tab", or any static/tabs/*.js work. Covers the Tabs registry pattern (mount(root, ctx), title, icon), ui.* and api.* helpers, error/empty states, and not fabricating data.
---

# SPA tab (static/tabs/*.js)

The frontend is a vanilla-JS SPA: `index.html` loads scripts, `app.js` builds
navigation from a global `window.Tabs` registry and mounts the active tab.
Each feature gets ONE file `static/tabs/<name>.js` exposing
`window.Tabs.<name>`. No frameworks, no build step.

## Anatomy (copy `static/tabs/pnr.js` — the cleanest example)

```js
/* <name>.js - <Title> tab. Live enquiry against GET /rail-api/... */
(() => {
window.Tabs = window.Tabs || {};
window.Tabs.<name> = {
  title: 'Display Title',
  icon: '…',                      // keep in family with other tabs
  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Title', ui.el('p', { class: 'text-sm muted', text: '...' }));
    const input = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: '...' });
    const btn = ui.el('button', { class: 'btn', text: 'Go' });
    const results = ui.el('div');
    ui.render(root, header, ui.card('', ui.label('...'), input, ui.el('div', { class: 'row mt-8' }, btn), results));
    btn.onclick = submit;
    function submit() {
      const value = input.value.trim();
      // validate BEFORE calling the API; show ui.errorBox on bad input
      ui.render(results, ui.errorBox('...')); return;
      // fetch via ctx.api or ctx.api.request(path), then ui.render(results, ...)
    }
  },
};
})();
```

## Conventions

- **LIVE DATA ONLY.** Call the real backend endpoint. Never fabricate or
  hardcode numbers/dates in the tab. On API error, render the returned error
  message (never a fake value).
- Render into detached `ui.el('div')` targets with `ui.render()`, so unmount
  keeps state; do not overwrite the whole root.
- Use `ui.card`, `ui.el`, `ui.label`, `ui.render`, `ui.errorBox`, `ui.spinner`,
  `ui.table` — see `static/ui.js`. `ui.el` sets `text`/`html`/classes/on* /
  attributes via options keys.
- Validate input client-side (PNR = exactly 10 digits, station code = 4 chars,
  train = 5 digits) and show `ui.errorBox` without hitting the network.
- Backend response DTOs carry `data_source` (e.g. `"NTES"`, `"Railyatri"`).
  Surface it in a muted notice when the UX differs by source.
- `static/api.js` (`window.Api`) has named helpers plus `Api.request(path)`.
  Use `encodeURIComponent` for query values. Unauthorized/captcha (HTTP 428)
  flows are supported in the API layer — reuse them rather than re-implementing.

## Registration / wiring

- Tabs auto-register via `window.Tabs`; `app.js` lists them in `NAV` and hides
  unregistered ones, so no `index.html` change is needed for a new tab (script
  tags there are per-tab `<script src="/tabs/<name>.js">`). If a tab is not
  appearing, check `window.Tabs.<name>` is defined and `NAV` includes the id.
- `ctx.autocomplete.attach` (station/train autocomplete, prewarmed from the
  datasets) is available for inputs that pick a station/train.
- Global search wiring hooks: `window.railwayTabs.{stations,trains}` — expose
  `selectStation`/`selectTrain` so global search can prefill this tab.

## Verify

- Syntax: `node --check static/tabs/<name>.js`.
- Behavioral check runs through the backend, not the browser: use
  `curl -s localhost:3000/rail-api/...` to confirm the JSON shape the tab
  expects (see `rust-workflow` for running the app).
