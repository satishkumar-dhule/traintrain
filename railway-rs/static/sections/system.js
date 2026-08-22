/* sections/system.js - System page. Views: observability, settings, debug.
   Replaces the old More section. Live-data-only. */

(() => {
window.Sections = window.Sections || {};

const VIEW_LABELS = {
  observability: 'Observability',
  settings: 'Settings',
  debug: 'Debug',
};

window.Sections.system = {
  mount(container, ctx, route) {
    const view = route.view || 'observability';
    const views = ['observability', 'settings', 'debug'];
    const pills = ctx.ui.pillBar(views, VIEW_LABELS, view, (v) => {
      ctx.navigate(Routes.href({ section: 'system', view: v }));
    });
    const content = ctx.ui.el('div');
    ctx.ui.render(container, pills, content);
    switch (view) {
      case 'settings': viewSettings(content, ctx); break;
      case 'debug': viewDebug(content, ctx); break;
      default: viewObservability(content, ctx); break;
    }
  },
};

/* ===================================================================
   Observability view – full port of tabs/observability.js
   =================================================================== */

const OBS_REFRESH_MS = 5000;
const OBS_MAX_POINTS = 120;
const OBS_LOG_LIMIT = 150;
const OBS_PALETTE = ['#2563eb', '#d97706', '#059669', '#dc2626', '#7c3aed', '#0ea5e9', '#db2777'];

let obsTimer = null;
let obsCharts = {};
let obsGaugeMax = {};
let obsLogFilter = 'all';
let obsUiRef = null;
let obsLastLogs = [];

function obsStop() {
  if (obsTimer) { clearInterval(obsTimer); obsTimer = null; }
  Object.values(obsCharts).forEach((c) => { if (c) c.destroy(); });
  obsCharts = {};
}

function obsEnsureChartLib() {
  return new Promise((resolve) => {
    if (window.Chart) return resolve(true);
    if (document.getElementById('chartjs-vendor')) {
      const tries = setInterval(() => {
        if (window.Chart) { clearInterval(tries); resolve(true); }
      }, 100);
      setTimeout(() => { clearInterval(tries); resolve(!!window.Chart); }, 3000);
      return;
    }
    const s = document.createElement('script');
    s.id = 'chartjs-vendor';
    s.src = '/vendor/chart.umd.min.js';
    s.onload = () => resolve(!!window.Chart);
    s.onerror = () => resolve(false);
    document.head.appendChild(s);
  });
}

function obsRegisterCenterText() {
  if (window.Chart && !window.Chart.registry.plugins.get('gaugeCenter')) {
    window.Chart.registry.add({
      id: 'gaugeCenter',
      afterDraw(chart) {
        const meta = chart.getDatasetMeta(0);
        if (!meta.data || !meta.data.length) return;
        const { ctx } = chart;
        const { x, y } = meta.data[0];
        const cfg = (chart.config.options.plugins || {}).gaugeCenter || {};
        ctx.save();
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = cfg.color || '#0f172a';
        ctx.font = `bold ${cfg.fontSize || 20}px system-ui, sans-serif`;
        ctx.fillText(cfg.value != null ? cfg.value : '', x, y - 8);
        if (cfg.sub) {
          ctx.fillStyle = '#94a3b8';
          ctx.font = `10px system-ui, sans-serif`;
          ctx.fillText(cfg.sub, x, y + 14);
        }
        ctx.restore();
      },
    });
  }
}

function obsFmtUptime(secs) {
  const total = Math.max(0, Number(secs) || 0);
  const d = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (d) return `${d}d ${h}h ${m}m`;
  if (h) return `${h}h ${m}m ${s}s`;
  if (m) return `${m}m ${s}s`;
  return `${s}s`;
}

function obsFmtNum(v) {
  return (Number(v) || 0).toLocaleString('en-IN');
}

function obsFmtBytes(v) {
  const n = Number(v) || 0;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(1)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(1)} MB`;
  if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${n} B`;
}

function obsEsc(v) {
  return String(v == null || v === '' ? '—' : v)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function obsArr(v) {
  return Array.isArray(v) ? v : [];
}

function obsBadge(text, kind) {
  return `<span class="badge badge-${kind}">${obsEsc(text)}</span>`;
}

function obsSpecs() {
  return [
    { id: 'rps', label: 'Req / sec', max: 50, color: '#2563eb' },
    { id: 'latency', label: 'Avg latency', max: 2000, suffix: 'ms', color: '#d97706' },
    { id: 'cpu', label: 'CPU', max: 1, suffix: '%', pct: true, color: '#7c3aed' },
    { id: 'mem', label: 'Memory', max: 1024, suffix: 'MB', color: '#059669' },
    { id: 'conn', label: 'Active conns', max: 100, color: '#0ea5e9' },
    { id: 'cache', label: 'Cache hit rate', max: 100, suffix: '%', color: '#0891b2' },
  ];
}

function obsGaugeData(spec, value) {
  const max = Math.max(spec.max, value || 0);
  const v = Math.min(Math.max(value || 0, 0), max);
  return {
    labels: [spec.label, ''],
    datasets: [{
      data: [v, Math.max(max - v, 0)],
      backgroundColor: [spec.color, '#e2e8f0'],
      borderWidth: 0,
      circumference: 180,
      rotation: -90,
      cutout: '78%',
    }],
  };
}

function obsGaugeOptions(spec) {
  return {
    responsive: true,
    maintainAspectRatio: true,
    aspectRatio: 2.2,
    plugins: {
      legend: { display: false },
      tooltip: { enabled: false },
      gaugeCenter: { value: '0', sub: spec.suffix || '', color: '#0f172a' },
    },
  };
}

function obsLineOptions(title, yLabel, gridColor) {
  return {
    responsive: true,
    maintainAspectRatio: false,
    animation: false,
    plugins: {
      legend: { display: false },
      tooltip: { mode: 'index', intersect: false },
    },
    scales: {
      x: { ticks: { maxTicksLimit: 6, font: { size: 10 }, color: '#94a3b8' }, grid: { display: false } },
      y: { title: { display: true, text: yLabel, font: { size: 10 }, color: '#94a3b8' }, ticks: { font: { size: 10 }, color: '#94a3b8' }, grid: { color: gridColor, drawTicks: false } },
    },
  };
}

function obsRenderError(body, msg) {
  let box = body.querySelector('.obs-error-box');
  if (!box) {
    box = document.createElement('div');
    box.className = 'error-box obs-error-box';
    body.prepend(box);
  }
  box.textContent = msg;
}

/* KPI build + update */

function obsBuildKpis(ui) {
  const grid = ui.el('div', { class: 'obs-kpi-grid' });
  obsSpecs().forEach((spec) => {
    obsGaugeMax[spec.id] = spec.max;
    const canvas = ui.el('canvas', { class: 'obs-gauge-canvas', id: `obs-gauge-${spec.id}`, width: 180, height: 90 });
    grid.append(ui.el('div', { class: 'obs-kpi-card' },
      ui.el('p', { class: 'obs-kpi-label', text: spec.label }),
      canvas,
    ));
  });
  return ui.card('Live metrics', grid, obsBuildStats(ui));
}

function obsInitCharts() {
  obsSpecs().forEach((spec) => {
    const el = document.getElementById(`obs-gauge-${spec.id}`);
    if (!el) return;
    const data = obsGaugeData(spec, 0);
    obsCharts[`gauge-${spec.id}`] = new window.Chart(el.getContext('2d'), {
      type: 'doughnut',
      data,
      options: obsGaugeOptions(spec),
    });
  });

  const lineDefs = [
    { id: 'rps', title: 'Request rate', yLabel: 'req/s', color: '#2563eb', fill: true },
    { id: 'latency', title: 'Request latency', yLabel: 'ms', color: '#d97706', fill: false },
    { id: 'mem', title: 'Memory', yLabel: 'MB', color: '#059669', fill: true },
  ];
  lineDefs.forEach((def) => {
    const el = document.getElementById(`obs-chart-${def.id}`);
    if (!el) return;
    obsCharts[`chart-${def.id}`] = new window.Chart(el.getContext('2d'), {
      type: 'line',
      data: { labels: [], datasets: [{ label: def.title, data: [], borderColor: def.color, backgroundColor: def.color + '22', fill: def.fill, tension: 0.35, pointRadius: 0, borderWidth: 2 }] },
      options: obsLineOptions(def.title, def.yLabel, 'rgba(148,163,184,0.6)'),
    });
  });

  const srcEl = document.getElementById('obs-chart-sources');
  if (srcEl) {
    obsCharts['chart-sources'] = new window.Chart(srcEl.getContext('2d'), {
      type: 'line',
      data: { labels: [], datasets: [] },
      options: obsLineOptions('Source latency', 'ms', 'rgba(148,163,184,0.6)'),
    });
  }

  const statusEl = document.getElementById('obs-chart-status');
  if (statusEl) {
    obsCharts['chart-status'] = new window.Chart(statusEl.getContext('2d'), {
      type: 'doughnut',
      data: {
        labels: ['2xx', '3xx', '4xx', '5xx'],
        datasets: [{ data: [0, 0, 0, 0], backgroundColor: ['#059669', '#0ea5e9', '#d97706', '#dc2626'], borderWidth: 0 }],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { position: 'bottom', labels: { boxWidth: 10, font: { size: 11 } } } },
      },
    });
  }
}

function obsUpdateKpis(m) {
  const values = {
    rps: Number(m.req_per_sec) || 0,
    latency: Number(m.latency_ms) || 0,
    cpu: Number(m.cpu_usage) || 0,
    mem: Math.round((Number(m.mem_usage) || 0) / 1024 / 1024),
    conn: Number(m.active_connections) || 0,
    cache: Number(m.cache && m.cache.hit_rate) || 0,
  };
  obsSpecs().forEach((spec) => {
    const raw = values[spec.id];
    if (spec.id === 'rps' && raw > obsGaugeMax.rps) obsGaugeMax.rps = Math.ceil(raw * 1.5);
    if (spec.id === 'latency' && raw > obsGaugeMax.latency) obsGaugeMax.latency = Math.ceil(raw * 1.5);
    if (spec.id === 'mem' && raw > obsGaugeMax.mem) obsGaugeMax.mem = Math.ceil(raw * 1.5);
    if (spec.id === 'conn' && raw > obsGaugeMax.conn) obsGaugeMax.conn = Math.ceil(raw * 1.5);

    const max = obsGaugeMax[spec.id] || spec.max;
    const value = Math.min(Math.max(raw, 0), max);
    const center = spec.id === 'cpu' ? `${(raw * 100).toFixed(0)}%` : obsFmtNum(raw);
    const chart = obsCharts[`gauge-${spec.id}`];
    if (chart) {
      chart.data.datasets[0].data = [value, Math.max(max - value, 0)];
      chart.config.options.plugins.gaugeCenter.value = center;
      chart.config.options.plugins.gaugeCenter.sub = spec.suffix || '';
      chart.update('none');
    }
  });
}

/* Charts build + update */

function obsBuildCharts(ui) {
  const lineDefs = [
    { id: 'rps', title: 'Request rate (req/s)', canvasId: 'obs-chart-rps' },
    { id: 'latency', title: 'Request latency (ms)', canvasId: 'obs-chart-latency' },
    { id: 'mem', title: 'Memory (MB)', canvasId: 'obs-chart-mem' },
  ];
  const grid = ui.el('div', { class: 'obs-charts-grid' });
  lineDefs.forEach((def) => {
    grid.append(
      ui.el('div', { class: 'obs-chart-card' },
        ui.el('p', { class: 'obs-chart-title', text: def.title }),
        ui.el('div', { class: 'obs-chart-box' }, ui.el('canvas', { id: def.canvasId })),
      ),
    );
  });
  grid.append(
    ui.el('div', { class: 'obs-chart-card' },
      ui.el('p', { class: 'obs-chart-title', text: 'Source latency (ms)' }),
      ui.el('div', { class: 'obs-chart-box' }, ui.el('canvas', { id: 'obs-chart-sources' })),
    ),
  );
  return ui.card('Graphs', grid);
}

function obsUpdateCharts(series) {
  const times = obsArr(series.times);
  const labels = times.slice(-OBS_MAX_POINTS).map((t) => {
    const d = new Date(Number(t) * 1000);
    return d.toTimeString().slice(0, 8);
  });

  const seriesDefs = [
    { chart: 'chart-rps', key: 'rps' },
    { chart: 'chart-latency', key: 'latency_ms' },
    { chart: 'chart-mem', key: 'mem_mb' },
  ];
  seriesDefs.forEach(({ chart, key }) => {
    const c = obsCharts[chart];
    if (!c) return;
    c.data.labels = labels;
    c.data.datasets[0].data = obsArr(series[key]).slice(-OBS_MAX_POINTS);
    c.update('none');
  });

  const srcChart = obsCharts['chart-sources'];
  if (srcChart) {
    const sourceNames = obsArr(series.sources).map((s) => s.name);
    srcChart.data.labels = labels;
    srcChart.data.datasets = sourceNames.map((name, i) => {
      const old = srcChart.data.datasets.find((d) => d.label === name);
      const seriesSrc = obsArr(series.sources).find((s) => s.name === name);
      return {
        label: name,
        data: obsArr(seriesSrc && seriesSrc.latency_ms).slice(-OBS_MAX_POINTS),
        borderColor: old ? old.borderColor : OBS_PALETTE[i % OBS_PALETTE.length],
        backgroundColor: 'transparent',
        borderWidth: 2,
        tension: 0.35,
        pointRadius: 0,
        fill: false,
      };
    });
    if (srcChart.options.plugins && srcChart.options.plugins.legend) {
      srcChart.options.plugins.legend.display = true;
    }
    srcChart.update('none');
  }
}

/* Stats build + update */

function obsBuildStats(ui) {
  const grid = ui.el('div', { class: 'obs-stats-grid mt-8' });
  const items = [
    ['Uptime', 'obs-stat-uptime'],
    ['Total requests', 'obs-stat-reqs'],
    ['Bytes served', 'obs-stat-bytes'],
    ['Cache entries', 'obs-stat-cache'],
  ];
  items.forEach(([label, id]) => {
    grid.append(ui.el('div', { class: 'obs-stat-card' },
      ui.el('p', { class: 'obs-kpi-label', text: label }),
      ui.el('p', { class: 'obs-stat-value', id, text: '—' }),
    ));
  });
  return grid;
}

function obsUpdateStats(m) {
  const set = (id, v) => {
    const el = document.getElementById(id);
    if (el) el.textContent = v;
  };
  set('obs-stat-uptime', obsFmtUptime(m.uptime_secs));
  set('obs-stat-reqs', obsFmtNum(m.requests_total));
  set('obs-stat-bytes', obsFmtBytes(m.bytes_out));
  set('obs-stat-cache', obsFmtNum(m.cache && m.cache.entries));

  const statusChart = obsCharts['chart-status'];
  if (statusChart) {
    const codes = obsArr(m.status_codes);
    const groups = [0, 0, 0, 0];
    codes.forEach((s) => {
      const c = Number(s.code) || 0;
      const idx = c >= 500 ? 3 : c >= 400 ? 2 : c >= 300 ? 1 : c >= 200 ? 0 : -1;
      if (idx >= 0) groups[idx] += Number(s.count) || 0;
    });
    statusChart.data.datasets[0].data = groups;
    statusChart.update('none');
  }
}

/* Tables build + update */

function obsBuildTables(ui) {
  const container = ui.el('div', { class: 'obs-tables-grid' });

  container.append(
    ui.card('Status',
      ui.el('div', { class: 'obs-doughnut-box' }, ui.el('canvas', { id: 'obs-chart-status', width: 200, height: 140 })),
      ui.el('div', { id: 'obs-status-table' }, ui.table(['Code', 'Count'], [['—', '—']])),
    ),
  );

  container.append(
    ui.card('Top Paths',
      ui.el('div', { id: 'obs-paths-table' }, ui.table(['Path', 'Reqs', 'Share'], [['—', '—', '—']])),
    ),
  );

  container.append(
    ui.card('Sources',
      ui.el('div', { id: 'obs-sources-table' }, ui.table(['Source', 'Latency', 'Samples'], [['—', '—', '—']])),
    ),
  );

  container.append(
    ui.card('Cache',
      ui.el('div', { id: 'obs-cache-table' }, ui.table(['Metric', 'Value'], [['—', '—']])),
    ),
  );

  return container;
}

function obsUpdateTables(m) {
  const total = Number(m.requests_total) || 0;
  const ui = obsUiRef;

  const codes = obsArr(m.status_codes);
  const statusRows = codes.map((s) => {
    const kind = s.code >= 500 ? 'red' : s.code >= 400 ? 'amber' : s.code >= 300 ? 'blue' : 'green';
    return [obsEsc(s.code), obsFmtNum(s.count), obsBadge(String(s.code >= 200 && s.code < 300 ? 'ok' : s.code >= 500 ? 'error' : s.code >= 400 ? 'warn' : 'info'), kind)];
  });
  const statusTbl = document.getElementById('obs-status-table');
  if (statusTbl) statusTbl.replaceChildren(ui.table(['Code', 'Count', 'Class'], statusRows.length ? statusRows : [['—', '—', '—']]));

  const paths = obsArr(m.top_paths);
  const pathRows = paths.map((p) => {
    const path = Array.isArray(p) ? p[0] : p.path;
    const count = Number(Array.isArray(p) ? p[1] : p.count) || 0;
    const share = total > 0 ? `${((count / total) * 100).toFixed(1)}%` : '—';
    return [obsEsc(path), obsFmtNum(count), share];
  });
  const pathsTbl = document.getElementById('obs-paths-table');
  if (pathsTbl) pathsTbl.replaceChildren(ui.collapsibleTable(['Path', 'Requests', 'Share'], pathRows.length ? pathRows : [['—', '—', '—']], 10));

  const origins = obsArr(m.origins);
  const sourceRows = origins.map((o) => [
    obsEsc(o.name),
    `${obsFmtNum(o.latency)} ms`,
    obsBadge(o.status, o.status === 'live' ? 'green' : 'amber'),
  ]);
  const sourcesTbl = document.getElementById('obs-sources-table');
  if (sourcesTbl) sourcesTbl.replaceChildren(ui.table(['Source', 'Latency', 'Status'], sourceRows.length ? sourceRows : [['—', '—', '—']]));

  const c = m.cache || {};
  const cacheRows = [
    ['Hits', obsFmtNum(c.hits)],
    ['Misses', obsFmtNum(c.misses)],
    ['Hit rate', c.hit_rate != null ? `${Number(c.hit_rate).toFixed(1)}%` : '—'],
    ['Entries', obsFmtNum(c.entries)],
  ];
  const cacheTbl = document.getElementById('obs-cache-table');
  if (cacheTbl) cacheTbl.replaceChildren(ui.table(['Metric', 'Value'], cacheRows));
}

/* Logs build + update */

function obsBuildLogs(ui) {
  const panel = ui.card('Logs');
  const controls = ui.el('div', { class: 'row gap-8 obs-log-controls' });
  const renderFilter = () => {
    controls.replaceChildren(ui.seg(
      [['all', 'All'], ['warn', 'Warn+'], ['error', 'Errors']],
      obsLogFilter,
      (v) => { obsLogFilter = v; renderFilter(); obsUpdateLogs(obsLastLogs); },
    ));
  };
  renderFilter();
  panel.append(
    controls,
    ui.el('div', { class: 'obs-log-panel', id: 'obs-log-list', style: 'max-height:260px;overflow:auto;' }),
  );
  return panel;
}

function obsUpdateLogs(logs) {
  const list = document.getElementById('obs-log-list');
  if (!list) return;
  const rows = obsArr(logs).filter((l) => {
    const level = String(l.level || '').toLowerCase();
    if (obsLogFilter === 'error') return level === 'error';
    if (obsLogFilter === 'warn') return level === 'warn' || level === 'error';
    return true;
  });
  list.replaceChildren();
  if (!rows.length) {
    list.append(obsUiRef.el('p', { class: 'text-xs muted', text: obsLogFilter === 'all' ? 'No log records.' : 'No matching records.' }));
    return;
  }
  rows.slice(0, 60).forEach((l) => {
    const level = String(l.level || '').toLowerCase();
    const kind = level === 'error' ? 'red' : level === 'warn' ? 'amber' : 'slate';
    const ts = new Date(Number(l.ts)).toTimeString().slice(0, 8);
    const fields = l.fields && typeof l.fields === 'object' && Object.keys(l.fields).length
      ? ` · ${Object.entries(l.fields).map(([k, v]) => `${k}=${obsEsc(String(v))}`).join(' ')}`
      : '';
    const line = obsUiRef.el('div', { class: `obs-log-line obs-log-${level || 'info'}` },
      obsUiRef.el('span', { class: 'obs-log-ts mono', text: ts }),
      obsUiRef.el('span', { class: 'obs-log-badge', html: `<span class="badge badge-${kind}">${obsEsc(level || 'info')}</span>` }),
      obsUiRef.el('span', { class: 'obs-log-target mono', text: l.target || '' }),
      obsUiRef.el('span', { class: 'obs-log-msg', text: l.message || '' }),
      obsUiRef.el('span', { class: 'obs-log-fields mono', html: fields }),
    );
    list.append(line);
  });
}

/* Main observability mount */

function viewObservability(container, ctx) {
  obsStop();
  const ui = ctx.ui;
  obsUiRef = ui;

  const refresh = ui.refreshRow({
    autoMs: OBS_REFRESH_MS,
    onRefresh: () => load(),
    onAuto: (on) => obsSetAuto(on),
  });
  const autoToggle = refresh.row.querySelector('.auto-toggle');
  if (autoToggle) {
    autoToggle.setAttribute('aria-pressed', 'true');
    autoToggle.classList.add('on');
  }
  const topRow = ui.el('div', { class: 'row justify-between items-center' }, refresh.row, ui.liveDot());
  ui.render(container, topRow);

  const body = ui.el('div', { class: 'obs-wrap' });
  container.append(body);

  body.append(obsBuildKpis(ui));
  body.append(obsBuildCharts(ui));
  body.append(obsBuildTables(ui));
  body.append(obsBuildLogs(ui));

  obsRegisterCenterText();
  obsEnsureChartLib().then((ok) => {
    if (!ok) {
      body.prepend(ui.el('p', { class: 'text-xs muted mt-8', text: 'Charts unavailable — tables and gauges still show live values.' }));
      return;
    }
    obsInitCharts();
  });

  function obsSetAuto(on) {
    if (obsTimer) { clearInterval(obsTimer); obsTimer = null; }
    if (on) obsTimer = setInterval(load, OBS_REFRESH_MS);
  }

  function load() {
    Promise.all([ctx.api.observability(), ctx.api.logs(OBS_LOG_LIMIT)])
      .then(([m, lr]) => {
        if (!container.contains(topRow)) { obsStop(); return; }
        if (!m || m.ok === false) {
          obsRenderError(body, `Observability: ${(m && m.error) || 'request failed'}`);
          return;
        }
        const logs = (lr && lr.ok !== false && obsArr(lr.logs)) || obsArr(m.logs);
        obsLastLogs = obsArr(logs);
        obsUpdateKpis(m);
        obsUpdateCharts(m.series || {});
        obsUpdateStats(m);
        obsUpdateTables(m);
        obsUpdateLogs(obsLastLogs);
        refresh.setUpdated(new Date().toISOString());
        body.classList.remove('obs-error');
      })
      .catch((err) => {
        if (!container.contains(topRow)) { obsStop(); return; }
        obsRenderError(body, `Request failed: ${err && err.message ? err.message : String(err)}`);
      });
  }

  load();
  obsTimer = setInterval(load, OBS_REFRESH_MS);
}

/* ===================================================================
   Settings view – ported from more.js viewSystem
   =================================================================== */

function viewSettings(container, ctx) {
  const ui = ctx.ui;
  const header = ui.card('Settings', ui.spinner());

  ctx.api.sourceStatus()
    .then((status) => {
      if (!status || status.ok === false) {
        const msg = status && status.error ? status.error : 'Failed to load source status.';
        ui.render(container, header, ui.errorBox(msg));
        return;
      }
      ui.render(container, header, ...renderSettings(status, ui));
    })
    .catch((err) => {
      const msg = err && err.message ? err.message : String(err);
      ui.render(container, header, ui.errorBox(`Failed: ${msg}`));
    });
}

function renderSettings(s, ui) {
  const liveBadge = s.live_enabled ? ui.badge('Enabled', 'green') : ui.badge('Disabled', 'red');

  return [
    ui.card('System',
      settingsInfoRow(ui, 'Mode', ui.badge(s.mode || 'live', 'blue')),
      settingsInfoRow(ui, 'Live', liveBadge),
      settingsInfoRow(ui, 'Cache', `${s.cache_ttl_seconds}s`),
      ui.el('div', { class: 'mt-8' }, ui.table(['Source', 'Status'], (s.sources || []).map((src) => [
        src.name,
        `<span class="badge badge-${src.reachable ? 'green' : 'red'}">${src.reachable ? 'Up' : 'Down'}</span>`,
      ]))),
    ),
  ];
}

function settingsInfoRow(ui, label, value) {
  const row = ui.el('div', { class: 'row justify-between', style: 'padding:2px 0;' });
  row.append(ui.el('span', { class: 'text-xs muted', text: label }));
  row.append(value.nodeType ? value : ui.el('span', { class: 'text-xs bold', text: value }));
  return row;
}

/* ===================================================================
   Debug view – ported from more.js viewDebug
   =================================================================== */

const DBG_MAX_ROWS = 500;

function viewDebug(container, ctx) {
  const ui = ctx.ui || window.UI;
  RailLog.lifecycle('debug view mounted', debugSystemInfo());
  debugBuildSummary(ui, container);
}

function debugCounts(entries) {
  const byLevel = { info: 0, warn: 0, error: 0 };
  const byType = {};
  entries.forEach((e) => {
    byLevel[e.l] = (byLevel[e.l] || 0) + 1;
    byType[e.ty] = (byType[e.ty] || 0) + 1;
  });
  return { byLevel, byType };
}

function debugSystemInfo() {
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

function debugBuildSummary(ui, root) {
  const info = debugSystemInfo();
  const infoLine = ui.el('p', { class: 'text-xs muted mono', text:
    `${info.viewport} · ${info.size}` });

  const stats = ui.el('div', { class: 'row gap-8', id: 'debug-stats' });

  const panel = ui.card('Debug Log',
    ui.el('div', { class: 'row gap-8' },
      ui.el('button', { class: 'btn btn-sm', text: 'Refresh', onclick: debugRefresh }),
      ui.el('button', { class: 'btn btn-sm secondary', text: 'Copy', onclick: debugCopyLog }),
      ui.el('button', { class: 'btn btn-sm secondary', text: 'Download', onclick: debugDownloadLog }),
      ui.el('button', { class: 'btn btn-sm secondary', text: 'Send', onclick: () => debugSendLog(ui) }),
      ui.el('button', { class: 'btn btn-sm ghost', text: 'Clear', onclick: () => debugClearLog(ui) }),
    ),
    stats,
    infoLine,
    ui.el('textarea', {
      id: 'debug-text', class: 'input mono debug-text', readonly: true,
      spellcheck: 'false',
      style: 'width:100%;min-height:160px;height:30vh;resize:vertical;font-size:11px;white-space:pre;overflow:auto;',
    }),
  );
  root.append(panel);
  debugUpdate(ui);
}

function debugUpdate(ui) {
  const entries = (window.RailLog && RailLog.entries()) || [];
  const { byLevel, byType } = debugCounts(entries);
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

function debugRaw() {
  return (window.RailLog && RailLog.raw()) || '(no log entries yet)';
}

function debugToast(msg, kind) {
  if (window.UI && window.UI.toast) window.UI.toast(msg, kind || 'info');
}

function debugRefresh() {
  debugUpdate(window.UI);
}

function debugCopyLog() {
  const text = debugRaw();
  const done = () => debugToast('Log copied', 'success');
  const fail = () => debugToast('Clipboard blocked — use Download instead', 'error');
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

function debugDownloadLog() {
  const blob = new Blob([debugRaw() + '\n'], { type: 'text/plain;charset=utf-8' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `rail-debug-${new Date().toISOString().replace(/[:.]/g, '-')}.txt`;
  document.body.appendChild(a);
  a.click();
  setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 200);
  debugToast('Download started', 'success');
}

function debugClearLog(ui) {
  ui.dialog({
    title: 'Clear debug log?',
    actions: [
      { label: 'Cancel', value: false, primary: false },
      { label: 'Clear', value: true },
    ],
  }).then((ok) => {
    if (!ok) return;
    RailLog.clear();
    debugUpdate(ui);
    debugToast('Log cleared', 'success');
  });
}

async function debugSendLog(ui) {
  const text = debugRaw();
  if (!text || text === '(no log entries yet)') {
    debugToast('Nothing to send', 'info');
    return;
  }
  try {
    const res = await window.Api.request('/rail-api/debug', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ report: text }),
    });
    if (res && res.ok !== false) {
      debugToast(`Sent (${res.lines || 0} lines)`, 'success');
      RailLog.lifecycle('debug report sent to server', { lines: res.lines || 0 });
    } else {
      debugToast(`Send failed: ${res && res.error ? res.error : 'unknown error'}`, 'error');
    }
  } catch (err) {
    const m = err && err.message ? err.message : String(err);
    debugToast(`Send failed: ${m}`, 'error');
  }
}

})();
