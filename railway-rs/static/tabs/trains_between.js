/* trains_between.js - "Trains B/W" tab. Direct trains between two stations
   from GET /rail-api/ntes/trains-between. Live-data-only: renders the real
   API response or an honest error. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.trains_between = {
  title: 'Trains B/W',
  icon: '📍',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Trains Between Stations',
      ui.el('p', { class: 'text-sm muted', text: 'Direct trains (NTES)' }),
    );

    const fromWrap = ui.el('div', { class: 'autocomplete' });
    const fromInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'From Station Code' });
    fromWrap.append(fromInput);

    const toWrap = ui.el('div', { class: 'autocomplete' });
    const toInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'To Station Code' });
    toWrap.append(toInput);

    const submit = ui.el('button', { class: 'btn', text: 'Find Trains' });
    const form = ui.card('',
      ui.label('From Station Code'),
      fromWrap,
      ui.el('div', { class: 'col mt-12' },
        ui.label('To Station Code'),
        toWrap,
      ),
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    const results = ui.el('div', { class: 'mt-12' });

    const submitForm = () => {
      const src = fromInput.value.trim();
      const dst = toInput.value.trim();
      if (!src || !dst) {
        ui.render(results, ui.errorBox('Both "From" and "To" station codes are required.'));
        return;
      }
      ui.render(results, ui.spinner());
      submit.disabled = true;

      ctx.api.trainsBetween(src, dst)
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load trains between stations.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderResults(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load trains between stations: ${msg}`));
        })
        .finally(() => { submit.disabled = false; });
    };

    fromInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    toInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    submit.addEventListener('click', submitForm);

    ctx.autocomplete.attach(fromInput, { type: 'station' });
    ctx.autocomplete.attach(toInput, { type: 'station' });

    ui.render(root, header, form, results);
  },
};

function renderResults(res, ui) {
  const route = ui.card('Route',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge(res.src || '', 'blue'),
      ui.el('span', { class: 'bold', text: '→' }),
      ui.badge(res.dst || '', 'blue'),
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['No.', 'Train', 'Departure', 'Arrival', 'Days'],
          trains.map((t) => [
            ui.el('span', { class: 'mono', text: t.number || '' }).outerHTML,
            t.name || '',
            ui.fmtTime(t.departure_time),
            ui.fmtTime(t.arrival_time),
            daysString(t.runs_on),
          ]))
      : ui.notice('No direct trains found.'),
  );

  return [route, list];
}

/* Deterministic "days" string from runs_on (7 booleans, Mon..Sun). */
function daysString(runsOn) {
  const letters = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
  if (!Array.isArray(runsOn)) return '—';
  return runsOn.map((on, i) => on ? letters[i] : '-').join('');
}
})();
