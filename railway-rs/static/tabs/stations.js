/* stations.js - "Stations" tab. Live search over stations + trains against
   GET /rail-api/stations and GET /rail-api/search/trains. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.stations = {
  title: 'Stations',
  icon: '🗺️',

  mount(root, ctx) {
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

    const form = ui.card('',
      ui.label('Query'),
      input,
      ui.el('div', { class: 'row mt-12' }, btn),
    );

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

    ui.render(root, header, form, results);
  },
};

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
    esc(t.number),
    esc(t.name),
    t.type ? esc(t.type) : '—',
  ]);
  return ui.card('Trains', ui.table(['No.', 'Train', 'Type'], rows));
}

function fieldRow(label, value, ui) {
  return ui.el('div', { class: 'row mt-8' },
    ui.el('span', { class: 'text-sm muted', text: label }),
    ui.el('span', { class: 'bold', text: String(value) }),
  );
}

/* Escape a value for use as table cell HTML (ui.table injects innerHTML). */
function esc(v) {
  return String(v == null || v === '' ? '—' : v)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
})();
