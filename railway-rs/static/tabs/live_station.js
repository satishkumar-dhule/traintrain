/* live_station.js - "Live Station" tab. Arrival board for a station from
   GET /rail-api/ntes/live-station. Live-data-only: renders exactly what the
   API returns, including honest "source unavailable" errors. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.live_station = {
  title: 'Live Station',
  icon: '⏱️',

  mount(root, ctx) {
    const ui = ctx.ui;

    const header = ui.card('Live Station',
      ui.el('p', { class: 'text-sm muted', text: 'Arrival board for a station (NTES)' }),
      ui.notice('Live data from NTES; the board may be unavailable if NTES blocks this deployment.'),
    );

    const codeInput = ui.el('input', { class: 'input', placeholder: 'e.g. NDLS', autocomplete: 'off' });
    let selectedCode = '';
    ctx.autocomplete.attach(codeInput, {
      type: 'station',
      onSelect(item) {
        selectedCode = item ? item.code : '';
      },
    });
    const codeWrap = ui.el('div', { class: 'autocomplete' }, codeInput);

    const hoursSelect = ui.el('select', { class: 'input' },
      ui.el('option', { value: '1', text: '1 hour' }),
      ui.el('option', { value: '2', text: '2 hours', selected: true }),
      ui.el('option', { value: '3', text: '3 hours' }),
      ui.el('option', { value: '4', text: '4 hours' }),
    );

    const submit = ui.el('button', { class: 'btn', text: 'Get Live Station' });
    const form = ui.card('',
      ui.label('Station Code'),
      codeWrap,
      ui.el('div', { class: 'col mt-12' },
        ui.label('Hours'),
        hoursSelect,
      ),
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    const results = ui.el('div', { class: 'col mt-12' });
    ui.render(root, header, form, results);

    function load() {
      const code = (selectedCode || codeInput.value).trim().toUpperCase();
      const hours = parseInt(hoursSelect.value, 10) || 2;
      const setLoading = ui.withLoading(submit, 'Loading…');
      setLoading(true);

      if (!code) {
        setLoading(false);
        ui.render(results, ui.errorBox('Enter a station code (4 characters, e.g. NDLS).'));
        return;
      }

      ui.render(results, ui.spinner());

      ctx.api.liveStation(code, hours)
        .then((res) => {
          setLoading(false);
          ui.render(results);
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load live station.';
            const errBox = ui.errorBox(msg);
            errBox.append(ui.el('p', { class: 'notice', text: 'NTES did not answer the arrival-board query, so no data is shown right now. Try again in a moment.' }));
            ui.render(results, errBox);
            return;
          }
          ui.render(results, renderStation(res, code, hours, ui), renderTrains(res, ui));
        })
        .catch((err) => {
          setLoading(false);
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load live station: ${msg}`));
        });
    }

    submit.onclick = load;
    codeInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') load(); });
  },
};

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

function renderTrains(res, ui) {
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
})();
