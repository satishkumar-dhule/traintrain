/* journey_basis.js - "Journey Basis" tab. Two-step flow on the NTES "Spot Your
   Train" second mode: load the journey stations a train offers, then pick one
   to see that run's live basis from it (GET /rail-api/ntes/journey-stations
   then /rail-api/ntes/journey-basis). Live-data-only: renders the real API
   responses or honest errors. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.journey_basis = {
  title: 'Journey Basis',
  icon: '🚉',

  mount(root, ctx) {
    const ui = ctx.ui;

    const header = ui.card('Journey Station Basis',
      ui.notice('Enter a train number to load its journey stations, then pick one to see the live run from that station (NTES).'),
    );

    const trainInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Train No. (5 digits)' });
    const loadBtn = ui.el('button', { class: 'btn', text: 'Load Stations' });
    const form = ui.card('',
      ui.label('Train Number'),
      trainInput,
      ui.el('div', { class: 'row mt-12' }, loadBtn),
    );

    const results = ui.el('div', { class: 'col mt-12' });
    let train = '';

    const showBasis = (stationCode) => {
      ui.render(results, ui.spinner());
      ctx.api.journeyBasis(train, stationCode)
        .then((res) => {
          ui.render(results);
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load journey basis.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderBasis(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load journey basis: ${msg}`));
        });
    };

    const loadStations = () => {
      train = trainInput.value.trim();
      if (!/^\d{5}$/.test(train)) {
        ui.render(results, ui.errorBox('Enter a valid 5-digit train number.'));
        return;
      }
      ui.render(results, ui.spinner());
      loadBtn.disabled = true;

      ctx.api.journeyStations(train)
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load journey stations.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderStationPicker(res, ui, showBasis));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load journey stations: ${msg}`));
        })
        .finally(() => { loadBtn.disabled = false; });
    };

    trainInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') loadStations(); });
    loadBtn.addEventListener('click', loadStations);

    ui.render(root, header, form, results);
  },
};

function renderStationPicker(res, ui, onPick) {
  const stations = Array.isArray(res.stations) ? res.stations : [];
  if (!stations.length) {
    return [ui.card('Journey Stations', ui.notice('No journey stations returned for this train.'))];
  }
  const select = ui.el('select', { class: 'input' },
    stations.map((s) => ui.el('option', { value: s.code, text: `${s.code} - ${s.name}` })),
  );
  const go = ui.el('button', { class: 'btn', text: 'Show Journey Basis' });
  go.addEventListener('click', () => onPick(select.value));

  return [
    ui.card('Journey Stations',
      ui.el('div', { class: 'row mt-8' },
        ui.el('span', { class: 'mono', text: res.train_no || '' }),
        ui.el('span', { class: 'text-sm muted', text: `${stations.length} station(s)` }),
        ui.badge('Source: ' + (res.data_source || 'unknown'), 'slate'),
      ),
      ui.label('Journey Station'),
      select,
      ui.el('div', { class: 'row mt-12' }, go),
    ),
  ];
}

function renderBasis(res, ui) {
  const cards = [];
  cards.push(ui.card('Journey Basis',
    ui.el('p', { class: 'text-sm bold', text: res.current_location_info || 'No current position reported.' }),
    ui.el('div', { class: 'row mt-8' },
      ui.el('span', { class: 'bold', text: res.train_name }),
      ui.badge(res.train_number || '', 'blue'),
      ui.badge('Source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  ));

  const js = res.journey_station;
  if (js) {
    cards.push(ui.card('Journey Station',
      ui.el('div', { class: 'row mt-8' },
        ui.el('span', { class: 'bold', text: js.name }),
        ui.badge(js.code || '', 'blue'),
        js.day_change ? ui.badge('Day Change', 'amber') : null,
      ),
      ui.el('p', { class: 'text-sm muted mt-8', text: `Seq ${js.seq ?? '-'} · Arrival days: ${js.arrival_days || '-'} · Departure days: ${js.departure_days || '-'}` }),
    ));
  }

  cards.push(stationsCard(res, ui));
  return cards;
}

function stationsCard(res, ui) {
  const stations = Array.isArray(res.stations) ? res.stations : [];
  if (!stations.length) {
    return ui.card('Stations', ui.notice('No station data returned.'));
  }
  const showActual = stations.some((s) => s.actual_arrival);
  const headers = ['Station', 'Code', 'Sch. Arrival'];
  if (showActual) headers.push('Act. Arrival');
  headers.push('Delay', 'Status');
  const rows = stations.map((s) => {
    const cells = [s.name, s.code, ui.fmtTime(s.scheduled_arrival)];
    if (showActual) cells.push(ui.fmtTime(s.actual_arrival));
    cells.push(delayCell(s.delay_minutes), statusBadge(s.status));
    return cells;
  });
  return ui.card('Stations', ui.table(headers, rows));
}

/* HTML strings (ui.table renders cell content via innerHTML). */
function delayCell(minutes) {
  if (!minutes || minutes <= 0) return '<span class="muted">-</span>';
  return `<span class="bold">${minutes} min</span>`;
}

function statusBadge(status) {
  const kind = status === 'departed' ? 'slate' : status === 'expected' ? 'amber' : 'blue';
  return `<span class="badge badge-${kind}">${status || 'scheduled'}</span>`;
}
})();
