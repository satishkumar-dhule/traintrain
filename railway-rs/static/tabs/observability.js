/* observability.js - state-of-the-art Observability dashboard.
   Data: GET /rail-api/observability (metrics + time-series + logs) and
   GET /rail-api/logs (live structured-log stream), auto-refreshed every 5s.
   Rendering: Chart.js (vendored at /vendor/chart.umd.min.js) for the line
   charts, doughnut gauges and status distribution; plain tables for the
   per-endpoint/source/cache breakdowns. Every number is real server data. */

window.Tabs = window.Tabs || {};

window.Tabs.observability = (() => {
  const REFRESH_MS = 5000;
  const MAX_POINTS = 120;
  const LOG_LIMIT = 150;

  const PALETTE = ['#2563eb', '#d97706', '#059669', '#dc2626', '#7c3aed', '#0ea5e9', '#db2777'];

  let timer = null;
  let charts = {};
  let gaugeMax = {};   // adaptive gauge ceilings keyed by gauge id
  let logFilter = 'all';
  let uiRef = null;    // UI helper captured at mount for use in refresh callbacks

  /* ---------- Chart.js loading (CSP-safe: vendored, same-origin) ---------- */

  function ensureChartLib() {
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

  /* Center-text plugin so the doughnut gauges can label their value. */
  function registerCenterText() {
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

  /* ------------------------------- helpers ------------------------------- */

  function stop() {
    if (timer) { clearInterval(timer); timer = null; }
    Object.values(charts).forEach((c) => { if (c) c.destroy(); });
    charts = {};
  }

  function fmtUptime(secs) {
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

  function fmtNum(v) {
    const n = Number(v) || 0;
    return n.toLocaleString('en-IN');
  }

  function fmtBytes(v) {
    const n = Number(v) || 0;
    if (n >= 1073741824) return `${(n / 1073741824).toFixed(1)} GB`;
    if (n >= 1048576) return `${(n / 1048576).toFixed(1)} MB`;
    if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${n} B`;
  }

  function timeNow() {
    return new Date().toTimeString().slice(0, 8);
  }

  function esc(v) {
    return String(v == null || v === '' ? '—' : v)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function arr(v) {
    return Array.isArray(v) ? v : [];
  }

  /* ------------------------------ mount ------------------------------- */

  function mount(root, ctx) {
    stop();
    const ui = ctx.ui;
    uiRef = ui;

    const header = ui.card('Observability',
      ui.el('div', { class: 'row justify-between items-center' },
        ui.el('p', { class: 'text-sm muted', text: 'Real runtime metrics · refreshed every 5s' }),
        ui.el('span', { class: 'badge badge-green', text: '● LIVE' }),
      ),
      ui.el('div', { class: 'text-sm muted mt-8', id: 'obs-refreshed' }),
    );

    ui.render(root, header);

    const body = ui.el('div', { class: 'obs-wrap' });
    root.append(body);

    // Static skeleton (built once so the Chart.js instances stay alive).
    body.append(buildKpis(ui));
    body.append(buildCharts(ui));
    body.append(buildStats(ui));
    body.append(buildTables(ui));
    body.append(buildLogs(ui));

    registerCenterText();
    ensureChartLib().then((ok) => {
      if (!ok) {
        const note = ui.el('p', { class: 'text-sm muted mt-8', text: 'Chart.js could not be loaded — tables and gauges below still reflect live data.' });
        body.prepend(note);
        return;
      }
      initCharts();
    });

    function refresh() {
      if (!root.contains(header)) { stop(); return; }
      root.querySelector('#obs-refreshed').textContent = `Last refreshed ${timeNow()} · ${new Date().toLocaleDateString()}`;
      Promise.all([ctx.api.observability(), ctx.api.logs(LOG_LIMIT)])
        .then(([m, lr]) => {
          if (!root.contains(header)) { stop(); return; }
          if (!m || m.ok === false) {
            renderError(body, `Observability: ${(m && m.error) || 'request failed'}`);
            return;
          }
          const logs = (lr && !lr.ok === false && arr(lr.logs)) || arr(m.logs);
          updateKpis(m);
          updateCharts(m.series || {});
          updateStats(m);
          updateTables(m);
          updateLogs(logs);
          body.classList.remove('obs-error');
        })
        .catch((err) => {
          if (!root.contains(header)) { stop(); return; }
          renderError(body, `Request failed: ${err && err.message ? err.message : String(err)}`);
        });
    }

    refresh();
    timer = setInterval(refresh, REFRESH_MS);
  }

  function renderError(body, msg) {
    let box = body.querySelector('.obs-error-box');
    if (!box) {
      box = document.createElement('div');
      box.className = 'error-box obs-error-box';
      body.prepend(box);
    }
    box.textContent = msg;
  }

  /* ------------------------- KPI gauge cards ------------------------- */

  function buildKpis(ui) {
    const grid = ui.el('div', { class: 'obs-kpi-grid' });
    specs().forEach((spec) => {
      gaugeMax[spec.id] = spec.max;
      const canvas = ui.el('canvas', { class: 'obs-gauge-canvas', id: `obs-gauge-${spec.id}`, width: 220, height: 110 });
      const card = ui.el('div', { class: 'obs-kpi-card' },
        ui.el('p', { class: 'obs-kpi-label', text: spec.label }),
        canvas,
        ui.el('p', { class: 'obs-kpi-sub', id: `obs-kpi-sub-${spec.id}`, text: '—' }),
      );
      grid.append(card);
    });
    return ui.card('Live Gauges', grid);
  }

  function initCharts() {
    specs().forEach((spec) => {
      const el = document.getElementById(`obs-gauge-${spec.id}`);
      if (!el) return;
      const data = gaugeData(spec, 0);
      charts[`gauge-${spec.id}`] = new window.Chart(el.getContext('2d'), {
        type: 'doughnut',
        data,
        options: gaugeOptions(spec),
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
      charts[`chart-${def.id}`] = new window.Chart(el.getContext('2d'), {
        type: 'line',
        data: { labels: [], datasets: [{ label: def.title, data: [], borderColor: def.color, backgroundColor: def.color + '22', fill: def.fill, tension: 0.35, pointRadius: 0, borderWidth: 2 }] },
        options: lineOptions(def.title, def.yLabel, 'rgba(148,163,184,0.6)'),
      });
    });

    const srcEl = document.getElementById('obs-chart-sources');
    if (srcEl) {
      charts['chart-sources'] = new window.Chart(srcEl.getContext('2d'), {
        type: 'line',
        data: { labels: [], datasets: [] },
        options: lineOptions('Source latency', 'ms', 'rgba(148,163,184,0.6)'),
      });
    }

    const statusEl = document.getElementById('obs-chart-status');
    if (statusEl) {
      charts['chart-status'] = new window.Chart(statusEl.getContext('2d'), {
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

  function specs() {
    return [
      { id: 'rps', label: 'Req / sec', max: 50, color: '#2563eb' },
      { id: 'latency', label: 'Avg latency', max: 2000, suffix: 'ms', color: '#d97706' },
      { id: 'cpu', label: 'CPU', max: 1, suffix: '%', pct: true, color: '#7c3aed' },
      { id: 'mem', label: 'Memory', max: 1024, suffix: 'MB', color: '#059669' },
      { id: 'conn', label: 'Active conns', max: 100, color: '#0ea5e9' },
      { id: 'cache', label: 'Cache hit rate', max: 100, suffix: '%', color: '#0891b2' },
    ];
  }

  function gaugeData(spec, value) {
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

  function gaugeOptions(spec) {
    return {
      responsive: true,
      // Height is derived from the width (2:1). With maintainAspectRatio:false
      // the chart fills its parent, whose height is driven by the canvas, so
      // each resize grows the canvas again -> the gauge grows vertically
      // without bound. aspectRatio converges instead.
      maintainAspectRatio: true,
      aspectRatio: 2,
      plugins: {
        legend: { display: false },
        tooltip: { enabled: false },
        gaugeCenter: { value: '0', sub: spec.suffix || '', color: '#0f172a' },
      },
    };
  }

  function updateKpis(m) {
    const values = {
      rps: Number(m.req_per_sec) || 0,
      latency: Number(m.latency_ms) || 0,
      cpu: Number(m.cpu_usage) || 0,
      mem: Math.round((Number(m.mem_usage) || 0) / 1024 / 1024),
      conn: Number(m.active_connections) || 0,
      cache: Number(m.cache && m.cache.hit_rate) || 0,
    };
    specs().forEach((spec) => {
      const raw = values[spec.id];
      const el = document.getElementById(`obs-gauge-${spec.id}`);
      const sub = document.getElementById(`obs-kpi-sub-${spec.id}`);
      if (sub) {
        if (spec.id === 'rps') sub.textContent = `of ${fmtNum(gaugeMax.rps)} · lifetime avg`;
        else if (spec.id === 'latency') sub.textContent = `${fmtNum(raw)}${spec.suffix || ''} EMA`;
        else if (spec.id === 'cpu') sub.textContent = `${(raw * 100).toFixed(1)}% of one core`;
        else if (spec.id === 'mem') sub.textContent = fmtBytes(Number(m.mem_usage) || 0) + ' RSS';
        else if (spec.id === 'conn') sub.textContent = 'in flight';
        else if (spec.id === 'cache') sub.textContent = `${fmtNum(m.cache && m.cache.hits || 0)} hits · ${fmtNum(m.cache && m.cache.misses || 0)} misses`;
      }

      // adaptive ceiling: keep the gauge meaningful as traffic grows
      if (spec.id === 'rps' && raw > gaugeMax.rps) gaugeMax.rps = Math.ceil(raw * 1.5);
      if (spec.id === 'latency' && raw > gaugeMax.latency) gaugeMax.latency = Math.ceil(raw * 1.5);
      if (spec.id === 'mem' && raw > gaugeMax.mem) gaugeMax.mem = Math.ceil(raw * 1.5);
      if (spec.id === 'conn' && raw > gaugeMax.conn) gaugeMax.conn = Math.ceil(raw * 1.5);

      const max = gaugeMax[spec.id] || spec.max;
      const value = Math.min(Math.max(raw, 0), max);   // clamp so the arc never overflows
      const center = spec.id === 'cpu' ? `${(raw * 100).toFixed(0)}%` : fmtNum(raw);
      const chart = charts[`gauge-${spec.id}`];
      if (chart) {
        chart.data.datasets[0].data = [value, Math.max(max - value, 0)];
        chart.config.options.plugins.gaugeCenter.value = center;
        chart.config.options.plugins.gaugeCenter.sub = spec.suffix || '';
        chart.update('none');
      }
    });
  }

  /* ------------------------------ charts ------------------------------- */

  function buildCharts(ui) {
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
    return ui.card('Real-time Graphs', grid);
  }

  function lineOptions(title, yLabel, gridColor) {
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

  function updateCharts(series) {
    const times = arr(series.times);
    const labels = times.slice(-MAX_POINTS).map((t) => {
      const d = new Date(Number(t) * 1000);
      return d.toTimeString().slice(0, 8);
    });

    const seriesDefs = [
      { chart: 'chart-rps', key: 'rps' },
      { chart: 'chart-latency', key: 'latency_ms' },
      { chart: 'chart-mem', key: 'mem_mb' },
    ];
    seriesDefs.forEach(({ chart, key }) => {
      const c = charts[chart];
      if (!c) return;
      c.data.labels = labels;
      c.data.datasets[0].data = arr(series[key]).slice(-MAX_POINTS);
      c.update('none');
    });

    const srcChart = charts['chart-sources'];
    if (srcChart) {
      const sourceNames = arr(series.sources).map((s) => s.name);
      srcChart.data.labels = labels;
      srcChart.data.datasets = sourceNames.map((name, i) => {
        const old = srcChart.data.datasets.find((d) => d.label === name);
        const seriesSrc = arr(series.sources).find((s) => s.name === name);
        return {
          label: name,
          data: arr(seriesSrc && seriesSrc.latency_ms).slice(-MAX_POINTS),
          borderColor: old ? old.borderColor : PALETTE[i % PALETTE.length],
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

  /* ------------------------------- stats ------------------------------- */

  function buildStats(ui) {
    const grid = ui.el('div', { class: 'obs-stats-grid' });
    const items = [
      ['Uptime', 'uptime', 'obs-stat-uptime', 'uptime_secs'],
      ['Total requests', 'requests_total', 'obs-stat-reqs', null],
      ['Bytes served', 'bytes_out', 'obs-stat-bytes', null],
      ['Cache entries', 'cache_entries', 'obs-stat-cache', null],
    ];
    items.forEach(([label, , id]) => {
      grid.append(ui.el('div', { class: 'obs-stat-card' },
        ui.el('p', { class: 'obs-kpi-label', text: label }),
        ui.el('p', { class: 'obs-stat-value', id, text: '—' }),
      ));
    });
    return ui.card('Runtime Stats', grid);
  }

  function updateStats(m) {
    const set = (id, v) => {
      const el = document.getElementById(id);
      if (el) el.textContent = v;
    };
    set('obs-stat-uptime', fmtUptime(m.uptime_secs));
    set('obs-stat-reqs', fmtNum(m.requests_total));
    set('obs-stat-bytes', fmtBytes(m.bytes_out));
    set('obs-stat-cache', fmtNum(m.cache && m.cache.entries));

    const statusChart = charts['chart-status'];
    if (statusChart) {
      const codes = arr(m.status_codes);
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

  /* ------------------------------ tables ------------------------------- */

  function buildTables(ui) {
    const container = ui.el('div', { class: 'obs-tables-grid' });

    container.append(
      ui.card('Status Distribution',
        ui.el('div', { class: 'obs-doughnut-box' }, ui.el('canvas', { id: 'obs-chart-status', width: 240, height: 190 })),
        ui.el('div', { id: 'obs-status-table' }, ui.table(['Code', 'Count'], [['—', '—']])),
      ),
    );

    container.append(
      ui.card('Top Paths',
        ui.el('div', { id: 'obs-paths-table' }, ui.table(['Path', 'Requests', 'Share'], [['—', '—', '—']])),
      ),
    );

    container.append(
      ui.card('Data Sources',
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

  function updateTables(m) {
    const total = Number(m.requests_total) || 0;

    // status codes
    const codes = arr(m.status_codes);
    const statusRows = codes.map((s) => {
      const kind = s.code >= 500 ? 'red' : s.code >= 400 ? 'amber' : s.code >= 300 ? 'blue' : 'green';
      return [esc(s.code), fmtNum(s.count), badge(String(s.code >= 200 && s.code < 300 ? 'ok' : s.code >= 500 ? 'error' : s.code >= 400 ? 'warn' : 'info'), kind)];
    });
    const statusTbl = document.getElementById('obs-status-table');
    if (statusTbl) statusTbl.replaceChildren(uiRef.table(['Code', 'Count', 'Class'], statusRows.length ? statusRows : [['—', '—', '—']]));

    // top paths
    const paths = arr(m.top_paths);
    const pathRows = paths.map((p) => {
      const path = Array.isArray(p) ? p[0] : p.path;
      const count = Number(Array.isArray(p) ? p[1] : p.count) || 0;
      const share = total > 0 ? `${((count / total) * 100).toFixed(1)}%` : '—';
      return [esc(path), fmtNum(count), share];
    });
    const pathsTbl = document.getElementById('obs-paths-table');
    if (pathsTbl) pathsTbl.replaceChildren(uiRef.table(['Path', 'Requests', 'Share'], pathRows.length ? pathRows : [['—', '—', '—']]));

    // sources
    const origins = arr(m.origins);
    const sourceRows = origins.map((o) => [
      esc(o.name),
      `${fmtNum(o.latency)} ms`,
      badge(o.status, o.status === 'live' ? 'green' : 'amber'),
    ]);
    const sourcesTbl = document.getElementById('obs-sources-table');
    if (sourcesTbl) sourcesTbl.replaceChildren(uiRef.table(['Source', 'Latency', 'Status'], sourceRows.length ? sourceRows : [['—', '—', '—']]));

    // cache
    const c = m.cache || {};
    const cacheRows = [
      ['Hits', fmtNum(c.hits)],
      ['Misses', fmtNum(c.misses)],
      ['Hit rate', c.hit_rate != null ? `${Number(c.hit_rate).toFixed(1)}%` : '—'],
      ['Entries', fmtNum(c.entries)],
    ];
    const cacheTbl = document.getElementById('obs-cache-table');
    if (cacheTbl) cacheTbl.replaceChildren(uiRef.table(['Metric', 'Value'], cacheRows));
  }

  function badge(text, kind) {
    return `<span class="badge badge-${kind}">${esc(text)}</span>`;
  }

  /* -------------------------------- logs ------------------------------- */

  function buildLogs(ui) {
    const panel = ui.card('Live Logs',
      ui.el('div', { class: 'row gap-8 obs-log-controls' },
        ['all', 'warn', 'error'].map((lv) =>
          ui.el('button', {
            class: `btn secondary obs-log-filter${lv === 'all' ? ' active' : ''}`,
            'data-level': lv,
            onclick: (e) => {
              logFilter = lv;
              panel.querySelectorAll('.obs-log-filter').forEach((b) => b.classList.remove('active'));
              e.currentTarget.classList.add('active');
            },
            text: lv === 'all' ? 'All' : lv === 'warn' ? 'Warn+' : 'Errors',
          })),
        ui.el('span', { class: 'text-xs muted', text: 'newest first · from the server log ring' }),
      ),
      ui.el('div', { class: 'obs-log-panel', id: 'obs-log-list' }),
    );
    return panel;
  }

  function updateLogs(logs) {
    const list = document.getElementById('obs-log-list');
    if (!list) return;
    const rows = arr(logs).filter((l) => {
      const level = String(l.level || '').toLowerCase();
      if (logFilter === 'error') return level === 'error';
      if (logFilter === 'warn') return level === 'warn' || level === 'error';
      return true;
    });
    list.replaceChildren();
    if (!rows.length) {
      list.append(uiRef.el('p', { class: 'text-xs muted', text: logFilter === 'all' ? 'No log records yet — traffic to the API will appear here.' : 'No matching log records.' }));
      return;
    }
    rows.slice(0, 120).forEach((l) => {
      const level = String(l.level || '').toLowerCase();
      const kind = level === 'error' ? 'red' : level === 'warn' ? 'amber' : 'slate';
      const ts = new Date(Number(l.ts)).toTimeString().slice(0, 8);
      const fields = l.fields && typeof l.fields === 'object' && Object.keys(l.fields).length
        ? ` · ${Object.entries(l.fields).map(([k, v]) => `${k}=${esc(String(v))}`).join(' ')}`
        : '';
      const line = uiRef.el('div', { class: `obs-log-line obs-log-${level || 'info'}` },
        uiRef.el('span', { class: 'obs-log-ts mono', text: ts }),
        uiRef.el('span', { class: 'obs-log-badge', html: `<span class="badge badge-${kind}">${esc(level || 'info')}</span>` }),
        uiRef.el('span', { class: 'obs-log-target mono', text: l.target || '' }),
        uiRef.el('span', { class: 'obs-log-msg', text: l.message || '' }),
        uiRef.el('span', { class: 'obs-log-fields mono', html: fields }),
      );
      list.append(line);
    });
  }

  return {
    title: 'Observability',
    icon: '📊',
    mount,
  };
})();
