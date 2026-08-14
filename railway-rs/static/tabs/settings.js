/* settings.js - Settings tab. Read-only: reports live runtime facts from
   GET /rail-api/source-status. No configuration is offered via the UI. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.settings = {
  title: 'Settings',
  icon: '⚙️',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('System Settings', ui.spinner());

    ctx.api.sourceStatus()
      .then((status) => {
        if (!status || status.ok === false) {
          const msg = status && status.error ? status.error : 'Failed to load source status.';
          ui.render(root, header, ui.errorBox(msg));
          return;
        }
        ui.render(root, header, ...renderCards(status, ui));
      })
      .catch((err) => {
        const msg = err && err.message ? err.message : String(err);
        ui.render(root, header, ui.errorBox(`Failed to load source status: ${msg}`));
      });
  },
};

function renderCards(s, ui) {
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
})();
