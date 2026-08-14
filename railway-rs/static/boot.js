/* boot.js - diagnostics harness. Loaded FIRST so every later script can rely on
   window.RailLog and so that any runtime error is captured and printed to the
   browser console with a [RailCompanion] prefix. Safe to leave in production. */

window.RailLog = {
  info: (...args) => console.log('[RailCompanion]', ...args),
  warn: (...args) => console.warn('[RailCompanion]', ...args),
  error: (...args) => console.error('[RailCompanion]', ...args),
};

window.addEventListener('error', (e) => {
  RailLog.error(
    'window.onerror:',
    e.message || 'unknown error',
    'at',
    `${e.filename || '?'}:${e.lineno || '?'}:${e.colno || '?'}`,
  );
});

window.addEventListener('unhandledrejection', (e) => {
  const r = e && e.reason;
  const msg = r && r.message ? r.message : String(r);
  RailLog.error('unhandledrejection:', msg);
});

RailLog.info('boot: scripts starting to load');

/* Report global availability at DOMContentLoaded so a missing script is
   immediately visible in the console. */
document.addEventListener('DOMContentLoaded', () => {
  const globals = ['UI', 'Api', 'AutoComplete']
    .map((g) => `${g}:${window[g] ? 'ok' : 'MISSING'}`)
    .join(', ');
  const tabs = Object.keys(window.Tabs || {}).sort();
  RailLog.info(`DOMContentLoaded -> globals [${globals}] tabs (${tabs.length}) [${tabs.join(', ')}]`);
});
