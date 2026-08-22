/* sections/home.js - Home dashboard (default #/). Aurora hero with the
   IRCTC-style booking console (BOOK TICKET | PNR STATUS | CHARTS/VACANCY),
   then a bento grid: quick-jump chips, favorites, recent lookups and live
   source status. Pure presentation over shared ctx. */

(() => {
window.Sections = window.Sections || {};

const CLASSES = [
  ['ALL', 'All Classes'],
  ['1A', 'AC First Class (1A)'],
  ['2A', 'AC 2 Tier (2A)'],
  ['3A', 'AC 3 Tier (3A)'],
  ['3E', 'AC 3 Economy (3E)'],
  ['FC', 'First Class (FC)'],
  ['SL', 'Sleeper (SL)'],
  ['2S', 'Second Sitting (2S)'],
];

window.Sections.home = {
  mount(container, ctx) {
    const ui = ctx.ui;
    const parts = [];
    parts.push(buildHero(ctx));

    const bento = ui.el('div', { class: 'bento' });

    const jumpWrap = ui.el('div', { class: 'bento-wide' });
    renderQuickJumps(jumpWrap, ctx);
    bento.append(jumpWrap);

    const favWrap = ui.el('div');
    bento.append(favWrap);
    const recentWrap = ui.el('div');
    bento.append(recentWrap);
    const statusWrap = ui.el('div', { class: 'bento-wide' });
    bento.append(statusWrap);

    parts.push(bento);
    renderFavs(favWrap, ctx);
    renderRecent(recentWrap, ctx);
    renderStatus(statusWrap, ctx);
    ui.render(container, ...parts);
  },
};

/* ---------- Aurora hero (console as the primary CTA) ---------- */

function buildHero(ctx) {
  const ui = ctx.ui;
  const hero = ui.el('div', { class: 'home-hero' });
  hero.append(
    ui.el('h1', { class: 'home-hero-title' },
      'Every train, ',
      ui.el('em', { text: 'live' }),
      '. Every station, now.'),
    ui.el('div', { class: 'home-hero-console' }, buildConsole(ctx)),
  );
  return hero;
}

function favHash(f) {
  if (f.type === 'train') return Routes.href({ section: 'train', params: { train: f.code } });
  if (f.type === 'station') return Routes.href({ section: 'station', params: { station: f.code } });
  return '#/';
}

/* ---------- Quick-jump chips ---------- */

function renderQuickJumps(wrap, ctx) {
  const ui = ctx.ui;
  const jumps = [
    { icon: 'train', label: 'Track a train', sub: 'live spot & delay', hash: '#/train' },
    { icon: 'station', label: 'Station board', sub: 'arrivals & departures', hash: '#/station' },
    { icon: 'map', label: 'Plan a journey', sub: 'trains between stations', hash: '#/plan' },
    { icon: 'pulse', label: 'Observability', sub: 'server vitals', hash: '#/system/observability' },
  ];
  const row = ui.el('div', { class: 'chip-row' });
  jumps.forEach((j) => {
    row.append(ui.el('button', {
      class: 'chip',
      onclick: () => ctx.navigate(j.hash),
      'aria-label': j.label + ' — ' + j.sub,
      title: j.sub,
    },
      ui.icon(j.icon),
      ui.el('span', { class: 'chip-code', text: j.label }),
    ));
  });
  ui.render(wrap, row);
}

/* ---------- IRCTC booking console ---------- */

function buildConsole(ctx) {
  const ui = ctx.ui;
  const { card } = ui.console({
    tabs: [
      ['book', 'Book Ticket', 'ticket'],
      ['pnr', 'PNR Status', 'list'],
      ['chart', 'Charts / Vacancy', 'pulse'],
    ],
    active: 'book',
    onTab: (id) => {
      if (id === 'pnr') return buildPnrTab(ctx);
      if (id === 'chart') return buildChartTab(ctx);
      return buildBookTab(ctx);
    },
  });
  return card;
}

function buildBookTab(ctx) {
  const ui = ctx.ui;
  const rb = ui.routeBox({});
  const date = ui.flDate({ label: 'Journey Date', initial: ui.today(), cls: 'console-date' });
  const cls = ui.flSelect({ label: 'Class', icon: 'train', cls: 'console-class', options: CLASSES });
  const flex = ui.checkRow({ label: 'Flexible with date' });
  const berth = ui.checkRow({ label: 'Trains with available berth' });
  const checks = ui.el('div', { class: 'console-checks' }, flex.row, berth.row);
  const btn = ui.searchBtn({ label: 'Search', onclick: submit });
  const err = ui.el('div');

  function submit() {
    const src = ui.stationCode(rb.from.value);
    const dst = ui.stationCode(rb.to.value);
    if (src.error || dst.error) { ui.render(err, ui.errorBox(src.error || dst.error)); return; }
    if (src.code === dst.code) { ui.render(err, ui.errorBox('Source and destination must differ.')); return; }
    ui.render(err);
    ctx.navigate(Routes.href({
      section: 'plan', view: 'trains',
      params: {
        src: src.code, dst: dst.code, date: date.getDate(),
        class: cls.get() !== 'ALL' ? cls.get() : '',
        flex: flex.get() ? '1' : '',
        berth: berth.get() ? '1' : '',
      },
    }));
  }

  rb.from.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
  rb.to.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });

  return ui.el('div', {},
    ui.el('div', { class: 'console-form' }, rb.wrap, date.wrap, cls.wrap, checks, btn),
    err);
}

function buildPnrTab(ctx) {
  const ui = ctx.ui;
  const input = ui.flInput({ label: 'PNR Number', icon: 'ticket', inputmode: 'numeric', placeholder: '10-digit PNR', cls: 'console-pnr' });
  const btn = ui.searchBtn({ label: 'Check Status', onclick: submit, cls: 'search-btn-row' });
  const err = ui.el('div');

  function submit() {
    const pnr = input.input.value.trim();
    if (!/^\d{10}$/.test(pnr)) { ui.render(err, ui.errorBox('Enter a valid 10-digit PNR.')); return; }
    ui.render(err);
    ctx.navigate('#/pnr/' + pnr);
  }

  input.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });

  return ui.el('div', {},
    ui.el('div', { class: 'row', style: 'gap:6px;align-items:flex-end;' },
      ui.el('div', { class: 'grow' }, input.wrap),
      btn),
    err);
}

function buildChartTab(ctx) {
  const ui = ctx.ui;
  const train = ui.flInput({ label: 'Train Number', icon: 'train', inputmode: 'numeric', placeholder: 'e.g. 12559', cls: 'console-train' });
  const date = ui.flDate({ label: 'Journey Date', initial: ui.today(), cls: 'console-date' });
  const btn = ui.searchBtn({ label: 'Get Chart', onclick: submit });
  const err = ui.el('div');
  const results = ui.el('div', { class: 'mt-8' });

  function submit() {
    const num = train.input.value.trim();
    if (!/^[0-9]{1,8}$/.test(num)) { ui.render(err, ui.errorBox('Enter a valid train number (digits only).')); return; }
    ui.render(err);
    ui.fetchFlow(results, () => ctx.api.chart(num, date.getDate()), { button: btn, failText: 'Failed to load the coach chart' })
      .then((res) => { if (res) ui.render(results, ...ui.chartView(res, ui, ctx)); });
  }

  train.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });

  return ui.el('div', {},
    ui.el('div', { class: 'console-form chart-form' }, train.wrap, date.wrap, btn),
    err,
    results);
}

/* ---------- Favorites / Recent / Status (bento tiles) ---------- */

function renderFavs(wrap, ctx) {
  const ui = ctx.ui;
  const list = ctx.fav.list();
  const card = ui.card('Favorites');
  if (!list.length) {
    card.append(ui.emptyState('star', 'No favorites yet', 'Star a train or station and it appears here.'));
  } else {
    const rows = ui.el('div', { class: 'col', style: 'gap:3px;' });
    list.slice(0, 8).forEach((f) => {
      if (f.type === 'train' && f.label && !f.label.includes(' \u00b7 ')) {
        ctx.enrichTrain(f.code, favHash(f), () => renderFavs(wrap, ctx));
      }
      rows.append(ui.el('button', { class: 'recent-item', onclick: () => ctx.navigate(favHash(f)), 'aria-label': 'Open ' + f.label },
        ui.el('span', { class: 'recent-label' },
          ui.icon(f.type === 'train' ? 'train' : 'station'),
          ' ' + f.label),
        ui.icon('star-fill', 'fav-star')));
    });
    card.append(rows);
  }
  ui.render(wrap, card);
}

function renderRecent(wrap, ctx) {
  const ui = ctx.ui;
  const card = ui.card('Recent');
  const list = ctx.recent.list();
  if (!list.length) {
    card.append(ui.el('p', { class: 'text-sm muted', text: 'No recent lookups yet.' }));
  } else {
    const rows = ui.el('div', { class: 'col', style: 'gap:3px;' });
    list.forEach((r) => {
      const entityType = r.hash.includes('/train/') ? 'train'
        : r.hash.includes('/station/') ? 'station'
        : r.hash.includes('/plan/') ? 'plan'
        : r.hash.includes('/pnr/') ? 'pnr' : '';
      const icons = { train: 'train', station: 'station', plan: 'plan', pnr: 'ticket' };
      const ts = r.ts ? ui.friendlyTime(new Date(r.ts).toISOString()) : '';
      if (entityType === 'train' && r.label && !r.label.includes(' \u00b7 ')) {
        ctx.enrichTrain(r.hash.match(/\/(\d+)/)?.[1] || '', r.hash, () => renderRecent(wrap, ctx));
      }
      rows.append(ui.el('button', { class: 'recent-item', onclick: () => ctx.navigate(r.hash) },
        ui.el('span', { class: 'recent-label' },
          icons[entityType] ? ui.icon(icons[entityType]) : null,
          ' ' + r.label),
        ui.el('span', { class: 'text-xs muted', text: ts }),
      ));
    });
    card.append(rows);
    card.append(ui.el('div', { class: 'row mt-8' },
      ui.el('button', { class: 'btn ghost btn-sm', onclick: () => { ctx.recent.clear(); renderRecent(wrap, ctx); } },
        ui.icon('trash', 'btn-ic'), ' Clear'),
    ));
  }
  ui.render(wrap, card);
}

function renderStatus(wrap, ctx) {
  const ui = ctx.ui;
  const card = ui.card('Live Data Status', ui.skeletonCard(2));
  ui.render(wrap, card);
  ctx.api.sourceStatus().then((s) => {
    if (!s || s.ok === false) {
      ui.render(wrap, ui.card('Live Data Status', ui.errorState('Status unavailable', s && s.error ? s.error : 'The source check failed.')));
      return;
    }
    const sources = (s.sources || []).map((src) => src.name);
    const up = (s.sources || []).filter((src) => src.reachable).length;
    const tiles = ui.el('div', { class: 'grid grid-2' },
      ui.statTile('Mode', s.mode || 'live', s.live_enabled ? 'live data enabled' : 'offline', s.live_enabled ? 'green' : 'red'),
      ui.statTile('Primary source', s.primary_source || '—', up + '/' + sources.length + ' sources up', up === sources.length ? 'green' : 'amber'),
    );
    const badges = ui.el('div', { class: 'row mt-8', style: 'gap:4px;flex-wrap:wrap;' },
      (s.sources || []).map((src) =>
        ui.badge(src.name + (src.reachable ? ' ↑' : ' ↓'), src.reachable ? 'green' : 'red')));
    ui.render(wrap, ui.card('Live Data Status', tiles, badges));
  }).catch((err) => {
    ui.render(wrap, ui.card('Live Data Status', ui.errorState('Status unavailable', err && err.message ? err.message : String(err))));
  });
}
})();
