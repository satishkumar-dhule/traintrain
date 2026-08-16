/* debug.js - diagnostics tab. Shows everything RailLog captured for this
   browser session (ring buffer, persisted to localStorage across reloads):
   API calls with status/latency/error, form actions and validation results,
   runtime errors, and unhandled rejections. Buttons let the user export the
   log (copy / download / send to the server log) so the developer can fix or
   explain an issue. */

window.Tabs = window.Tabs || {};

window.Tabs.debug = (() => {
  const MAX_ROWS = 500;

  function counts(entries) {
    const byLevel = { info: 0, warn: 0, error: 0 };
    const byType = {};
    entries.forEach((e) => {
      byLevel[e.l] = (byLevel[e.l] || 0) + 1;
      byType[e.ty] = (byType[e.ty] || 0) + 1;
    });
    return { byLevel, byType };
  }

  function esc(s) {
    return String(s == null ? '' : s)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function systemInfo() {
    const sizes = window.localStorage && window.localStorage.length != null
      ? `localStorage items: ${window.localStorage.length}`
      : 'localStorage: unavailable';
    return {
      href: location.href,
      ua: navigator.userAgent,
      viewport: `${window.innerWidth}x${window.innerHeight}`,
      size,
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

  function mount(root, ctx) {
    const ui = ctx.ui || window.UI;
    RailLog.lifecycle('debug tab mounted', systemInfo());
    buildSummary(ui, root);
  }

  return {
    title: 'Debug',
    icon: '🐞',
    mount,
  };
})();
