/* boot.js - diagnostics harness. Loaded FIRST so every later script can rely on
   window.RailLog. RailLog mirrors every message to the browser console AND keeps
   a ring buffer (persisted to localStorage) so a session's diagnostics survive a
   reload and can be exported from the Debug tab for the developer. Safe to leave
   in production; the buffer only stores logs this page generates. */

window.RailLog = (() => {
  const MAX = 1500;
  const LS_KEY = 'rail_log_v1';
  let entries = [];
  try {
    const saved = localStorage.getItem(LS_KEY);
    if (saved) {
      const parsed = JSON.parse(saved);
      if (Array.isArray(parsed)) entries = parsed.slice(-MAX);
    }
  } catch (e) { /* storage unavailable (private mode / sandbox) - logs are memory-only */ }

  function persist() {
    try { localStorage.setItem(LS_KEY, JSON.stringify(entries.slice(-MAX))); } catch (e) { /* ignore */ }
  }

  function safeStr(v) {
    if (v === null || v === undefined) return String(v);
    if (typeof v === 'string') return v;
    if (v instanceof Error) return `${v.name}: ${v.message}${v.stack ? '\n' + v.stack : ''}`;
    if (typeof v === 'object') {
      try { return JSON.stringify(v); } catch (e) { return String(v); }
    }
    return String(v);
  }

  /* Try to clone detail to plain JSON; fall back to a string on cycles. */
  function sanitize(detail) {
    if (detail === undefined || detail === null) return null;
    try { return JSON.parse(JSON.stringify(detail)); } catch (e) { return safeStr(detail); }
  }

  function push(level, type, msg, detail) {
    const entry = {
      t: Date.now(),
      ts: new Date().toISOString(),
      l: level,          // info | warn | error
      ty: type,          // log | api | action | lifecycle | error
      m: msg,
      d: sanitize(detail),
    };
    entries.push(entry);
    if (entries.length > MAX) entries.splice(0, entries.length - MAX);
    persist();
    const line = JSON.stringify({ ty: entry.ty, m: entry.m, d: entry.d });
    const prefix = `[RailCompanion][${entry.ts}]`;
    if (level === 'error') console.error(prefix, line);
    else if (level === 'warn') console.warn(prefix, line);
    else console.log(prefix, line);
  }

  return {
    info: (...args) => push('info', 'log', args.map(safeStr).join(' ')),
    warn: (...args) => push('warn', 'log', args.map(safeStr).join(' ')),
    error: (...args) => push('error', 'log', args.map(safeStr).join(' ')),

    /* Structured capture points (used by boot/api/tabs). */
    lifecycle: (msg, detail) => push('info', 'lifecycle', msg, detail),
    action: (tab, label, detail) => push('info', 'action', `${tab}: ${label}`, detail),
    api: (detail) => push(detail && detail.error ? 'warn' : 'info', 'api',
      `${detail.method} ${detail.url} -> ${detail.status || 0}${detail.error ? ' ' + detail.error : ''}`, detail),
    syserr: (kind, msg, detail) => push('error', 'error', `${kind}: ${msg}`, detail),

    entries: () => entries.slice(),
    raw: () => entries.map((e) =>
      `[${e.ts}] ${e.l.toUpperCase().padEnd(5)} ${e.ty} ${e.m}${e.d ? ' ' + JSON.stringify(e.d) : ''}`
    ).join('\n'),
    clear: () => { entries = []; persist(); },
  };
})();

/* Capture runtime errors (with stack) and unhandled promise rejections so a
   failing interaction is fully traceable even when it does not hit the console. */
window.addEventListener('error', (e) => {
  RailLog.syserr('window.onerror',
    `${e.message || 'unknown error'} at ${e.filename || '?'}:${e.lineno || '?'}:${e.colno || '?'}`,
    { stack: e.error && e.error.stack ? String(e.error.stack) : undefined });
});

window.addEventListener('unhandledrejection', (e) => {
  const r = e && e.reason;
  const msg = (r && r.message) ? r.message : String(r);
  RailLog.syserr('unhandledrejection', msg, { stack: r && r.stack ? String(r.stack) : undefined });
});

RailLog.lifecycle('boot: RailLog ready; scripts starting to load', {
  ua: navigator.userAgent,
  href: location.href,
  ts: new Date().toISOString(),
});

/* Report global availability at DOMContentLoaded so a missing script is
   immediately visible in the console and in the Debug tab. */
document.addEventListener('DOMContentLoaded', () => {
  const globals = ['UI', 'Api', 'AutoComplete']
    .map((g) => `${g}:${window[g] ? 'ok' : 'MISSING'}`)
    .join(', ');
  const tabs = Object.keys(window.Tabs || {}).sort();
  RailLog.lifecycle(`DOMContentLoaded -> globals [${globals}] tabs (${tabs.length}) [${tabs.join(', ')}]`);
});

/* ---------- Offline banner ---------- */

function updateOfflineBanner() {
  const banner = document.getElementById('offline-banner');
  if (!banner) return;
  const offline = !navigator.onLine;
  banner.classList.toggle('hidden', !offline);
}

window.addEventListener('online', () => {
  updateOfflineBanner();
  RailLog.info('network online');
});
window.addEventListener('offline', () => {
  updateOfflineBanner();
  RailLog.warn('network offline — live fetches will fail');
});
document.addEventListener('DOMContentLoaded', updateOfflineBanner);

/* ---------- PWA install prompt (no service worker; shell assets only) ---------- */

let deferredInstall = null;
window.addEventListener('beforeinstallprompt', (e) => {
  e.preventDefault();
  deferredInstall = e;
  RailLog.info('beforeinstallprompt captured');
});
window.InstallApp = {
  available: () => !!deferredInstall,
  prompt() {
    if (!deferredInstall) return Promise.resolve(false);
    return deferredInstall.prompt()
      .then(() => deferredInstall.userChoice)
      .then((choice) => choice && choice.outcome === 'accepted')
      .catch(() => false)
      .finally(() => { deferredInstall = null; });
  },
};
