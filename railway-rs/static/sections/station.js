/* sections/station.js - Station section. Views: live (arrival board), tt
   (station timetable). Deep links like #/station/NDLS auto-submit.
   Live-data-only. */

(() => {
window.Sections = window.Sections || {};

window.Sections.station = {
  mount(container, ctx, route) {
    const p = route.params || {};
    const view = route.view || 'live';
    if (view === 'tt') viewTT(container, ctx, p);
    else viewLive(container, ctx, p);
  },
};

function fillInput(value, input) {
  if (value && input) input.value = String(value).trim();
}

/* ---------- live ---------- */

function viewLive(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Live Station',
    ui.el('p', { class: 'text-sm muted', text: 'Arrival board for a station (NTES)' }),
    ui.notice('Live data from NTES; the board may be unavailable if NTES blocks this deployment.'),
  );

  const { wrap, input } = ui.stationInput('e.g. NDLS');
  const hoursSelect = ui.el('select', { class: 'input' },
    ui.el('option', { value: '2', text: '2 hours', selected: true }),
    ui.el('option', { value: '4', text: '4 hours' }),
    ui.el('option', { value: '8', text: '8 hours' }),
  );

  const submit = ui.el('button', { class: 'btn', text: 'Get Live Station' });
  const results = ui.el('div', { class: 'col mt-12' });

  function load() {
    let code = input.value.trim().toUpperCase();
    const hours = parseInt(hoursSelect.value, 10) || 2;
    const setLoading = ui.withLoading(submit, 'Loading…');
    setLoading(true);

    RailLog.action('live_station', 'submit', { code_raw: code, hours });

    if (!code) {
      setLoading(false);
      RailLog.action('live_station', 'validation', { error: 'empty', code_raw: code });
      ui.render(results, ui.errorBox('Enter a station code (2-4 characters, e.g. NDLS or AK).'));
      return;
    }
    const check = ui.stationCode(code);
    if (check.error) {
      setLoading(false);
      RailLog.action('live_station', 'validation', { error: check.error, code_raw: code });
      ui.render(results, ui.errorBox(check.error));
      return;
    }
    code = check.code;
    RailLog.action('live_station', 'validated', { code, hours });

    ui.fetchFlow(results, () => ctx.api.liveStation(code, hours), { failText: 'Failed to load live station' })
      .then((res) => {
        setLoading(false);
        if (!res) return;
        ui.render(results, renderStation(res, code, hours, ui), renderLiveTrains(res, ui));
      });
  }

  submit.onclick = load;
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') load(); });

  const form = ui.queryCard(
    [['Station Code', wrap], ['Hours', hoursSelect]],
    submit,
  );
  ui.render(container, header, form, results);

  if (params.station) { fillInput(params.station, input); load(); }
}

function renderStation(res, code, hours, ui) {
  const stationBadge = ui.badge(code || res.station || '?', 'blue');
  const sourceBadge = res.data_source ? ui.badge(res.data_source, 'slate') : null;
  return ui.card('Station',
    ui.el('div', { class: 'row mt-8' },
      stationBadge,
      ui.el('span', { class: 'text-sm muted', text: `last ${hours} hours` }),
    ),
    ui.el('div', { class: 'row mt-8' }, sourceBadge),
  );
}

function renderLiveTrains(res, ui) {
  const trains = Array.isArray(res.trains) ? res.trains : [];
  if (!trains.length) {
    return ui.card('Trains', ui.notice('No trains in window.'));
  }
  const rows = trains.map((t) => [
    t.number,
    t.name,
    ui.fmtTime(t.sta),
    ui.fmtTime(t.eta),
    `<span class="badge badge-${t.delay_arr ? 'red' : 'green'}">${t.delay_arr ? 'LATE' : 'ON TIME'}</span>`,
    t.platform,
  ]);
  return ui.card('Trains', ui.table(['No.', 'Train', 'STA', 'ETA', 'Delay', 'Platform'], rows));
}

/* ---------- tt ---------- */

function viewTT(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Station Time Table',
    ui.el('p', { class: 'text-sm muted', text: 'Trains scheduled at a station (NTES)' }),
  );

  const { wrap, input } = ui.stationInput('Station Code');
  const submit = ui.el('button', { class: 'btn', text: 'Get Trains' });
  const results = ui.el('div', { class: 'mt-12' });

  function submitForm() {
    RailLog.action('station_timetable', 'submit', { station_raw: input.value });
    const check = ui.stationCode(input.value);
    if (check.error) {
      RailLog.action('station_timetable', 'validation', { error: check.error, raw: input.value });
      ui.render(results, ui.errorBox(check.error));
      return;
    }
    RailLog.action('station_timetable', 'validated', { station: check.code });
    ui.fetchFlow(results, () => ctx.api.stationTimetable(check.code), { button: submit, failText: 'Failed to load the station timetable' })
      .then((res) => { if (res) ui.render(results, ...renderTT(res, ui)); });
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  ui.render(container, header, ui.queryCard([['Station Code', wrap]], submit), results);

  if (params.station) { fillInput(params.station, input); submitForm(); }
}

function renderTT(res, ui) {
  const summary = ui.card('Station Time Table',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge(res.station || '', 'blue'),
      ui.el('span', { class: 'bold', text: res.station_name || '' }),
      ui.badge('Total: ' + (res.total || 0), 'green'),
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['No.', 'Train', 'Arrival', 'Departure', 'Days', 'Type'],
          trains.map((t, i) => [
            (i + 1).toString(),
            `${t.number} ${t.name}`,
            t.arrival,
            t.departure,
            t.days,
            t.train_type,
          ]))
      : ui.notice('No trains found.'),
  );

  return [summary, list];
}
})();
