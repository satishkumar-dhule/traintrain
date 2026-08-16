/* sections/home.js - Home section. Quick-action tiles (deep links), recent
   lookups from localStorage, and the data-mode chip. */

(() => {
window.Sections = window.Sections || {};

window.Sections.home = {
  mount(root, ctx) {
    const ui = ctx.ui;
    const navigate = ctx.navigate;

    const tiles = [
      ['🚄', 'Track a train', 'Live position & schedule', Routes.href({ section: 'track' })],
      ['🚉', 'Live station board', 'Departures at a station', Routes.href({ section: 'station' })],
      ['📍', 'Plan a journey', 'Trains, availability, chart', Routes.href({ section: 'plan' })],
      ['🎫', 'Check PNR', 'Booking status', Routes.href({ section: 'pnr' })],
      ['🗺️', 'Browse stations', 'All stations list', Routes.href({ section: 'more', view: 'stations' })],
      ['📊', 'Observability', 'System health & metrics', Routes.href({ section: 'more', view: 'observability' })],
    ];
    const grid = ui.el('div', { class: 'grid grid-2' });
    tiles.forEach(([icon, label, hint, hash]) => {
      grid.append(ui.el('button', { class: 'tile', onclick: () => navigate(hash) },
        ui.el('span', { class: 'tile-icon', text: icon }),
        ui.el('span', { class: 'tile-label', text: label }),
        ui.el('span', { class: 'tile-hint', text: hint })));
    });

    const source = ui.el('p', { class: 'text-sm muted mt-12', text: 'Data mode: loading…' });
    ctx.api.sourceStatus().then((s) => {
      source.textContent = (s && s.ok !== false)
        ? `Data mode: ${s.mode || 'live'} · primary source: ${s.primary_source || 'unknown'}`
        : 'Data mode: unknown (source-status unavailable)';
    }).catch(() => { source.textContent = 'Data mode: unknown (source-status unavailable)'; });

    const recentWrap = ui.el('div');
    renderRecent(recentWrap, ctx);

    ui.render(root,
      ui.el('h1', { class: 'home-title', text: 'RailCompanion' }),
      ui.el('p', { class: 'text-sm muted', text: 'Live Indian Railways data — search stations & trains in the header, or jump straight in.' }),
      grid,
      source,
      recentWrap,
    );
  },
};

function renderRecent(wrap, ctx) {
  const ui = ctx.ui;
  const card = ui.card('Recent lookups');
  const list = ctx.recent.list();
  if (!list.length) {
    card.append(ui.notice('Nothing yet — your recent train, station, plan and PNR lookups appear here.'));
  } else {
    const rows = ui.el('div', { class: 'col', style: 'gap:4px;' });
    list.forEach((r) => {
      rows.append(ui.el('button', { class: 'recent-item', onclick: () => ctx.navigate(r.hash) },
        ui.el('span', { class: 'recent-label', text: r.label }),
        ui.el('span', { class: 'text-sm muted', text: r.hash })));
    });
    card.append(rows);
    card.append(ui.el('div', { class: 'row mt-12' },
      ui.el('button', { class: 'btn ghost', text: 'Clear', onclick: () => { ctx.recent.clear(); renderRecent(wrap, ctx); } })));
  }
  ui.render(wrap, card);
}
})();
