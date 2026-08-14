/* observability.js - Observability tab. Live runtime metrics from
   GET /rail-api/observability and GET /rail-api/source-status,
   auto-refreshed every 10 seconds. */

window.Tabs = window.Tabs || {};

window.Tabs.observability = (() => {
  const REFRESH_MS = 10000;
  let timer = null;

  function stop() {
    if (timer) { clearInterval(timer); timer = null; }
  }

  return {
    title: 'Observability',
    icon: '📊',

    mount(root, ctx) {
      stop();
      const ui = ctx.ui;

      const header = ui.card('Observability',
        ui.el('p', { class: 'text-sm muted', text: 'Live runtime metrics, refreshed every 10s' }),
      );
      const refreshed = ui.el('div', { class: 'text-sm muted mt-8' });
      header.append(refreshed);

      ui.render(root, header);

      function renderAll() {
        if (!root.contains(header)) { stop(); return; }
        refreshed.textContent = `Last refreshed ${timeNow()}`;

        Promise.all([ctx.api.observability(), ctx.api.sourceStatus()])
          .then(([metrics, sources]) => {
            if (!root.contains(header)) { stop(); return; }
            const cards = [];
            if (!metrics || metrics.ok === false) {
              cards.push(ui.errorBox(`Observability: ${(metrics && metrics.error) || 'request failed'}`));
            } else {
              cards.push(renderMetrics(metrics, ui));
            }
            if (!sources || sources.ok === false) {
              cards.push(ui.errorBox(`Source status: ${(sources && sources.error) || 'request failed'}`));
            } else {
              cards.push(renderSources(sources, ui));
            }
            ui.render(root, header, ...cards);
          })
          .catch((err) => {
            if (!root.contains(header)) { stop(); return; }
            const msg = err && err.message ? err.message : String(err);
            ui.render(root, header, ui.errorBox(`Request failed: ${msg}`));
          });
      }

      renderAll();
      timer = setInterval(renderAll, REFRESH_MS);
    },
  };

  function renderMetrics(m, ui) {
    const stats = ui.card('Server',
      infoRow(ui, 'Uptime', fmtUptime(m.uptime_secs)),
      infoRow(ui, 'Total requests', fmtNum(m.requests_total)),
      infoRow(ui, 'Active connections', fmtNum(m.active_connections)),
      infoRow(ui, 'Requests / sec', fmtNum(m.req_per_sec)),
      infoRow(ui, 'Avg latency', m.latency_ms != null ? `${m.latency_ms} ms` : '—'),
      infoRow(ui, 'CPU usage', m.cpu_usage != null ? `${(m.cpu_usage * 100).toFixed(1)}%` : '—'),
      infoRow(ui, 'Memory', fmtBytes(m.mem_usage)),
    );

    const cache = renderCache(m, ui);

    const origins = Array.isArray(m.origins) && m.origins.length
      ? ui.card('Data Sources', ui.table(
          ['Source', 'Latency', 'Status'],
          m.origins.map((o) => [
            esc(o.name),
            `${o.latency} ms`,
            statusBadge(o.status),
          ]),
        ))
      : null;

    const topPaths = Array.isArray(m.top_paths) && m.top_paths.length
      ? ui.card('Top Paths', ui.table(
          ['Path', 'Requests'],
          m.top_paths.map((p) => [esc(Array.isArray(p) ? p[0] : p.path), fmtNum(Array.isArray(p) ? p[1] : p.count)]),
        ))
      : null;

    return [stats, cache, origins, topPaths].filter(Boolean);
  }

  function renderCache(m, ui) {
    if (typeof m.cache_hits !== 'number' && typeof m.cache_misses !== 'number') return null;
    const hits = Number(m.cache_hits) || 0;
    const misses = Number(m.cache_misses) || 0;
    const total = hits + misses;
    return ui.card('Cache',
      infoRow(ui, 'Hits', fmtNum(hits)),
      infoRow(ui, 'Misses', fmtNum(misses)),
      infoRow(ui, 'Hit rate', total > 0 ? `${((hits / total) * 100).toFixed(1)}%` : '—'),
    );
  }

  function renderSources(s, ui) {
    const rows = (s.sources || []).map((src) => {
      const up = !!src.reachable;
      return [esc(src.name), statusBadge(up ? 'up' : 'down')];
    });

    return ui.card('Source Status',
      infoRow(ui, 'Mode', s.mode || '—'),
      infoRow(ui, 'Cache TTL', s.cache_ttl_seconds != null ? `${s.cache_ttl_seconds}s` : '—'),
      ui.table(['Source', 'Status'], rows.length ? rows : [['—', '—']]),
    );
  }

  function statusBadge(status) {
    const text = String(status || 'unknown');
    const kind = /^(live|ok|up|healthy|reachable)$/.test(text.toLowerCase())
      ? 'green'
      : /^(down|error|unreachable|offline|failed)$/.test(text.toLowerCase())
        ? 'red'
        : 'slate';
    return `<span class="badge badge-${kind}">${esc(text)}</span>`;
  }

  function infoRow(ui, label, value) {
    const row = ui.el('div', { class: 'row justify-between mt-8' });
    row.append(ui.el('span', { class: 'label', text: label }));
    row.append(value.nodeType ? value : ui.el('span', { text: String(value) }));
    return row;
  }

  function fmtUptime(secs) {
    const total = Math.max(0, Number(secs) || 0);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    if (!h && !m) return `${s}s`;
    if (!h) return `${m}m ${s}s`;
    return `${h}h ${m}m ${s}s`;
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
})();
