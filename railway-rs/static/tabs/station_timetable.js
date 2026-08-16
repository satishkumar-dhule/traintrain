/* station_timetable.js - "Station TT" tab. Trains scheduled at a station from
   GET /rail-api/ntes/station-timetable. Live-data-only: renders the real API
   response or an honest error. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.station_timetable = {
  title: 'Station TT',
  icon: '🗓️',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Station Time Table',
      ui.el('p', { class: 'text-sm muted', text: 'Trains scheduled at a station (NTES)' }),
    );

    const stnWrap = ui.el('div', { class: 'autocomplete' });
    const stnInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Station Code' });
    stnWrap.append(stnInput);

    const dateInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Date (optional, e.g. 15-Aug-2026)' });

    const submit = ui.el('button', { class: 'btn', text: 'Get Trains' });
    const form = ui.card('',
      ui.label('Station Code'),
      stnWrap,
      ui.el('div', { class: 'col mt-12' },
        ui.label('Date'),
        dateInput,
      ),
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    const results = ui.el('div', { class: 'mt-12' });

    const submitForm = () => {
      const station = stnInput.value.trim();
      if (!station) {
        ui.render(results, ui.errorBox('Station code is required.'));
        return;
      }
      ui.render(results, ui.spinner());
      submit.disabled = true;

      ctx.api.stationTimetable(station)
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load the station timetable.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderResults(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load the station timetable: ${msg}`));
        })
        .finally(() => { submit.disabled = false; });
    };

    stnInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    dateInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    submit.addEventListener('click', submitForm);

    ctx.autocomplete.attach(stnInput, { type: 'station' });

    ui.render(root, header, form, results);
  },
};

function renderResults(res, ui) {
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
