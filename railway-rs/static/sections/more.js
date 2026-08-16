/* sections/more.js - More section: hub + views heritage, parcel, stations,
   system, observability, debug. The observability view delegates to the
   retained observability tab; the rest are ported here. Live-data-only. */

(() => {
window.Sections = window.Sections || {};

const MORE_ITEMS = [
  ['heritage', '🚞', 'Heritage'],
  ['parcel', '📦', 'Parcel SPL'],
  ['stations', '🗺️', 'Stations'],
  ['system', '⚙️', 'System'],
  ['observability', '📊', 'Observability'],
  ['debug', '🐞', 'Debug'],
];

window.Sections.more = {
  mount(container, ctx, route) {
    const view = route.view;
    if (!view) return hub(container, ctx);
    switch (view) {
      case 'heritage': viewHeritage(container, ctx); break;
      case 'parcel': viewParcel(container, ctx); break;
      case 'stations': viewStations(container, ctx); break;
      case 'system': viewSystem(container, ctx); break;
      case 'observability': window.Tabs.observability.mount(container, ctx); break;
      case 'debug': viewDebug(container, ctx); break;
    }
  },
};

function hub(container, ctx) {
  const ui = ctx.ui;
  const card = ui.card('More',
    ui.el('p', { class: 'text-sm muted', text: 'Libraries and system tools.' }),
  );
  const grid = ui.el('div', { class: 'grid grid-2 mt-12' });
  MORE_ITEMS.forEach(([id, icon, label]) => {
    grid.append(ui.el('button', {
      class: 'btn secondary',
      text: icon + ' ' + label,
      onclick: () => ctx.navigate('#/more/' + id),
    }));
  });
  card.append(grid);
  ui.render(container, card);
}

/* ---------- heritage ---------- */

const SELECTIONS = [
  [0, 'All Heritage Trains'],
  [1, 'Kalka Shimla Railway'],
  [2, 'Matheran Hill Railway'],
  [3, 'Kangra Valley Railway'],
  [4, 'Nilgiri Mountain Railway'],
  [5, 'Darjeeling Himalayan Railway'],
];

function viewHeritage(container, ctx) {
  const ui = ctx.ui;
  const header = ui.card('Heritage Trains',
    ui.el('p', { class: 'text-sm muted', text: 'Heritage trains of Indian Railways (NTES)' }),
  );

  const select = ui.el('select', { class: 'input' });
  SELECTIONS.forEach(([value, label]) => {
    select.append(ui.el('option', { value: String(value), text: label }));
  });

  const submit = ui.el('button', { class: 'btn', text: 'Get Trains' });
  const results = ui.el('div', { class: 'mt-12' });

  const submitForm = () => {
    ui.fetchFlow(results, () => ctx.api.heritage(select.value), { button: submit, failText: 'Failed to load heritage trains' })
      .then((res) => { if (res) ui.render(results, ...renderHeritage(res, ui)); });
  };

  submit.addEventListener('click', submitForm);

  ui.render(container, header, ui.queryCard([['Heritage Line', select]], submit), results);
  submitForm();
}

function renderHeritage(res, ui) {
  const summary = ui.card('Summary',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge('Total: ' + (res.total ?? 0), 'blue'),
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['Train', 'Runs', 'From', 'To', 'Duration'],
          trains.map((t) => [
            `${t.number} ${t.name}`,
            `${t.runs} | ${t.train_type}`,
            `${t.source_station} (${t.source_code}) ${t.source_time}`,
            `${t.dest_station} (${t.dest_code}) ${t.dest_time}`,
            t.duration,
          ]))
      : ui.notice('No heritage trains found.'),
  );

  return [summary, list];
}

/* ---------- parcel ---------- */

function viewParcel(container, ctx) {
  const ui = ctx.ui;
  const header = ui.card('Parcel Special Trains',
    ui.el('p', { class: 'text-sm muted', text: 'Currently running time-tabled parcel special trains (NTES)' }),
  );

  const refresh = ui.el('button', { class: 'btn', text: 'Refresh' });
  header.append(ui.el('div', { class: 'row mt-12' }, refresh));

  const results = ui.el('div', { class: 'mt-12' });

  const fetchParcel = () => {
    ui.fetchFlow(results, () => ctx.api.parcel(), { button: refresh, failText: 'Failed to load parcel special trains' })
      .then((res) => { if (res) ui.render(results, ...renderParcel(res, ui)); });
  };

  refresh.addEventListener('click', fetchParcel);

  ui.render(container, header, results);
  fetchParcel();
}

function renderParcel(res, ui) {
  const source = ui.card('',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Parcel Special Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['No.', 'Train', 'Route', 'Days', 'Validity', 'From', 'To', 'Travel'],
          trains.map((t, i) => [
            String(i + 1),
            `${t.number || ''} ${t.name || ''}`,
            t.route || '',
            t.days_of_run || '',
            `${t.validity_from || ''} → ${t.validity_to || ''}`,
            `${t.source_code || ''} ${t.source_time || ''}`,
            `${t.dest_code || ''} ${t.dest_time || ''}`,
            t.travel_time || '',
          ]))
      : ui.notice('No parcel special trains found.'),
  );

  return [source, list];
}

/* ---------- stations ---------- */

function viewStations(container, ctx) {
  const ui = ctx.ui;
  const header = ui.card('Stations',
    ui.el('p', { class: 'text-sm muted', text: 'Live search over stations and trains by code, name or number.' }),
  );

  const input = ui.el('input', {
    class: 'input',
    autocomplete: 'off',
    placeholder: 'e.g. NDLS, MUMBAI RAJDHANI, 12951',
  });
  const btn = ui.el('button', { class: 'btn', text: 'Search' });

  const detailBody = ui.el('div', {},
    ui.emptyState('Click a station result to select it.'),
  );
  const detail = ui.card('Selected Station', detailBody);

  const results = ui.el('div', { class: 'col mt-12' });

  function search() {
    const q = input.value.trim();
    if (!q) {
      ui.render(results, detail, ui.notice('Enter a station name, code or train number.'));
      return;
    }
    const setLoading = ui.withLoading(btn, 'Searching…');
    setLoading(true);
    ui.render(results, ui.spinner());

    Promise.all([ctx.api.stations(q), ctx.api.searchTrains(q)])
      .then(([stations, trains]) => {
        setLoading(false);
        ui.render(results, detail,
          renderStations(stations, ui, selectStation),
          renderTrains(trains, ui),
        );
      })
      .catch((err) => {
        setLoading(false);
        const msg = err && err.message ? err.message : String(err);
        ui.render(results, detail, ui.errorBox(`Search failed: ${msg}`));
      });
  }

  function selectStation(s) {
    ui.render(detailBody,
      fieldRow('Code', s.code, ui),
      fieldRow('Name', s.name, ui),
      s.city ? fieldRow('City', s.city, ui) : null,
      s.zone ? fieldRow('Zone', s.zone, ui) : null,
    );
  }

  input.addEventListener('input', ui.debounce(search, 250));
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') search(); });
  btn.addEventListener('click', search);

  ui.render(container, header,
    ui.queryCard([['Query', input]], btn),
    results);
}

function renderStations(res, ui, onPick) {
  if (!Array.isArray(res)) {
    const msg = res && res.ok === false && res.error ? res.error : 'Failed to load stations.';
    return ui.card('Stations', ui.errorBox(msg));
  }
  if (!res.length) return ui.card('Stations', ui.notice('No stations found.'));
  const rows = res.map((s) => ui.el('div', {
    class: 'row mt-8',
    style: 'cursor:pointer;',
    onclick: () => onPick(s),
  },
    ui.el('span', { class: 'bold', text: s.name }),
    ui.badge(s.code, 'blue'),
    s.city ? ui.el('span', { class: 'text-sm muted', text: s.city }) : null,
    s.zone ? ui.el('span', { class: 'text-sm muted', text: s.zone }) : null,
  ));
  return ui.card('Stations', ...rows);
}

function renderTrains(res, ui) {
  if (!Array.isArray(res)) {
    const msg = res && res.ok === false && res.error ? res.error : 'Failed to load trains.';
    return ui.card('Trains', ui.errorBox(msg));
  }
  if (!res.length) return ui.card('Trains', ui.notice('No trains found.'));
  const rows = res.map((t) => [
    ui.esc(t.number),
    ui.esc(t.name),
    t.type ? ui.esc(t.type) : '—',
  ]);
  return ui.card('Trains', ui.table(['No.', 'Train', 'Type'], rows));
}

function fieldRow(label, value, ui) {
  return ui.el('div', { class: 'row mt-8' },
    ui.el('span', { class: 'text-sm muted', text: label }),
    ui.el('span', { class: 'bold', text: String(value) }),
  );
}

/* ---------- system ---------- */

function viewSystem(container, ctx) {
  const ui = ctx.ui;
  const header = ui.card('System Settings', ui.spinner());

  ctx.api.sourceStatus()
    .then((status) => {
      if (!status || status.ok === false) {
        const msg = status && status.error ? status.error : 'Failed to load source status.';
        ui.render(container, header, ui.errorBox(msg));
        return;
      }
      ui.render(container, header, ...renderSystem(status, ui));
    })
    .catch((err) => {
      const msg = err && err.message ? err.message : String(err);
      ui.render(container, header, ui.errorBox(`Failed to load source status: ${msg}`));
    });
}

function renderSystem(s, ui) {
  const liveBadge = s.live_enabled ? ui.badge('Enabled', 'green') : ui.badge('Disabled', 'red');

  const dataMode = ui.card('Data Mode',
    infoRow(ui, 'Data mode', ui.badge(s.mode || 'live', 'blue')),
    infoRow(ui, 'Live data', liveBadge),
    infoRow(ui, 'Cache TTL', `${s.cache_ttl_seconds}s`),
    infoRow(ui, 'Primary source', s.primary_source),
  );

  const liveSources = ui.card('Live Sources',
    ui.table(['Source', 'Reachable'], (s.sources || []).map((src) => [
      src.name,
      `<span class="badge badge-${src.reachable ? 'green' : 'red'}">${src.reachable ? 'Up' : 'Down'}</span>`,
    ])),
  );

  const notice = ui.card('Data Notice', ui.notice(s.notice));

  const links = (s.verification_links || []).map((href) =>
    ui.el('a', { class: 'text-sm', href, target: '_blank', rel: 'noopener', text: href }),
  );
  const verify = ui.card('Verification Links',
    ui.el('div', { class: 'col mt-8' }, links.length ? links : ui.emptyState('No verification links.')),
  );

  const footer = ui.card('',
    ui.notice('This app is live-data-only. Nothing on this page is configurable from the UI; all options are managed by the server.'),
  );

  return [dataMode, liveSources, notice, verify, footer];
}

function infoRow(ui, label, value) {
  const row = ui.el('div', { class: 'row justify-between mt-8' });
  row.append(ui.el('span', { class: 'label', text: label }));
  row.append(value.nodeType ? value : ui.el('span', { text: value }));
  return row;
}

/* ---------- debug ---------- */

const MAX_ROWS = 500;

function viewDebug(container, ctx) {
  const ui = ctx.ui || window.UI;
  RailLog.lifecycle('debug view mounted', systemInfo());
  buildSummary(ui, container);
}

function counts(entries) {
  const byLevel = { info: 0, warn: 0, error: 0 };
  const byType = {};
  entries.forEach((e) => {
    byLevel[e.l] = (byLevel[e.l] || 0) + 1;
    byType[e.ty] = (byType[e.ty] || 0) + 1;
  });
  return { byLevel, byType };
}

function systemInfo() {
  const sizes = window.localStorage && window.localStorage.length != null
    ? `localStorage items: ${window.localStorage.length}`
    : 'localStorage: unavailable';
  return {
    href: location.href,
    ua: navigator.userAgent,
    viewport: `${window.innerWidth}x${window.innerHeight}`,
    size: sizes,
    log_version: 'v2',
  };
}

function buildSummary(ui, root) {
  const info = systemInfo();
  const infoLine = ui.el('p', { class: 'text-xs muted mono', text:
    `${info.href} · viewport ${info.viewport} · ${info.size} · log ${info.log_version}` });

  const stats = ui.el('div', { class: 'row gap-8 wrap', id: 'debug-stats' });

  const panel = ui.card('Debug Log (this browser)',
    ui.el('p', { class: 'text-xs muted', text: 'Every API request, form action, validation result and runtime error is captured here. Copy the log and paste it to the developer, or send it to the server log.' }),
    ui.el('div', { class: 'row gap-8 mt-8' },
      ui.el('button', { class: 'btn', text: 'Refresh', onclick: refresh }),
      ui.el('button', { class: 'btn secondary', text: 'Copy log', onclick: copyLog }),
      ui.el('button', { class: 'btn secondary', text: 'Download', onclick: downloadLog }),
      ui.el('button', { class: 'btn secondary', text: 'Send to server', onclick: () => sendLog(ui) }),
      ui.el('button', { class: 'btn ghost', text: 'Clear', onclick: () => clearLog(ui) }),
    ),
    stats,
    infoLine,
    ui.el('textarea', {
      id: 'debug-text', class: 'input mono debug-text', readonly: true,
      spellcheck: 'false',
      style: 'width:100%;min-height:320px;height:55vh;resize:vertical;font-size:12px;white-space:pre;overflow:auto;',
    }),
    ui.el('div', { id: 'debug-actions', class: 'text-xs muted mt-8' }),
  );
  root.append(panel);
  update(ui);
}

function update(ui) {
  const entries = (window.RailLog && RailLog.entries()) || [];
  const { byLevel, byType } = counts(entries);
  const stats = document.getElementById('debug-stats');
  if (stats) {
    stats.replaceChildren(
      ui.badge(`${byLevel.error || 0} errors`, 'red'),
      ui.badge(`${byLevel.warn || 0} warnings`, 'amber'),
      ui.badge(`${byLevel.info || 0} info`, 'slate'),
      ui.badge(`${entries.length} total`, 'blue'),
      ...Object.entries(byType)
        .filter(([t]) => t !== 'log')
        .map(([t, n]) => ui.badge(`${n} ${t}`, 'slate')),
    );
  }
  const box = document.getElementById('debug-text');
  if (box) box.value = RailLog.raw() || '(no log entries yet)';
}

function raw() {
  return (window.RailLog && RailLog.raw()) || '(no log entries yet)';
}

function flash(msg) {
  const el = document.getElementById('debug-actions');
  if (!el) return;
  el.textContent = `${new Date().toISOString()} ${msg}`;
}

function refresh() {
  update(window.UI);
  flash('refreshed');
}

function copyLog() {
  const text = raw();
  const done = () => flash('copied to clipboard');
  const fail = () => flash('clipboard blocked — use Download instead');
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(done, fail);
    return;
  }
  const ta = document.createElement('textarea');
  ta.value = text;
  ta.style.position = 'fixed';
  ta.style.opacity = '0';
  document.body.appendChild(ta);
  ta.select();
  try { document.execCommand('copy'); done(); } catch (e) { fail(); }
  ta.remove();
}

function downloadLog() {
  const blob = new Blob([raw() + '\n'], { type: 'text/plain;charset=utf-8' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `rail-debug-${new Date().toISOString().replace(/[:.]/g, '-')}.txt`;
  document.body.appendChild(a);
  a.click();
  setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 200);
  flash('download started');
}

function clearLog(ui) {
  if (!window.confirm('Clear the collected debug log for this browser?')) return;
  RailLog.clear();
  update(ui);
  flash('log cleared');
}

async function sendLog(ui) {
  const text = raw();
  if (!text || text === '(no log entries yet)') {
    flash('nothing to send');
    return;
  }
  flash('sending…');
  try {
    const res = await window.Api.request('/rail-api/debug', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ report: text }),
    });
    if (res && res.ok !== false) {
      flash(`sent to server log (${res.lines || 0} lines) — tell the developer to check /tmp/railway-rs.log`);
      RailLog.lifecycle('debug report sent to server', { lines: res.lines || 0 });
    } else {
      flash(`send failed: ${res && res.error ? res.error : 'unknown error'}`);
    }
  } catch (err) {
    const m = err && err.message ? err.message : String(err);
    flash(`send failed: ${m}`);
  }
}
})();
