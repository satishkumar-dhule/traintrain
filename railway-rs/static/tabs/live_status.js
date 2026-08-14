/* live_status.js - "Spot Train" tab. Live train position from Railyatri.
   Live-data-only: renders exactly what GET /rail-api/live-status returns. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.live_status = {
  title: 'Spot Train',
  icon: '🚄',

  mount(root, ctx) {
    const ui = ctx.ui;

    const header = ui.card('Spot Train (Live Status)',
      ui.notice('Live position from Railyatri.'),
    );

    const trainInput = ui.el('input', { class: 'input', placeholder: 'Train number', autocomplete: 'off' });
    const dateInput = ui.el('input', { class: 'input', placeholder: 'today', autocomplete: 'off' });

    const results = ui.el('div', { class: 'col mt-12' });
    results.append(ui.emptyState('Enter a train number to spot it.'));

    const submit = ui.el('button', { class: 'btn', text: 'Spot Train' });

    function spot() {
      const train = trainInput.value.trim();
      const date = dateInput.value.trim();

      if (!/^\d+$/.test(train)) {
        results.replaceChildren();
        results.append(ui.errorBox('Enter a valid train number (digits only).'));
        return;
      }
      if (date && !/^\d{4}-\d{2}-\d{2}$/.test(date)) {
        results.replaceChildren();
        results.append(ui.errorBox('Date must be YYYY-MM-DD (or leave blank for today).'));
        return;
      }

      results.replaceChildren();
      results.append(ui.spinner());

      ctx.api.liveStatus(train, date || undefined)
        .then((res) => {
          results.replaceChildren();
          if (!res || res.ok === false) {
            results.append(ui.errorBox((res && res.error) ? res.error : 'Failed to load live status.'));
            return;
          }
          results.append(renderPosition(res, ui));
          results.append(renderStations(res, ui));
        })
        .catch((err) => {
          results.replaceChildren();
          const msg = err && err.message ? err.message : String(err);
          results.append(ui.errorBox(`Failed to load live status: ${msg}`));
        });
    }

    submit.onclick = spot;
    dateInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') spot(); });
    trainInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') spot(); });

    const form = ui.card('Track',
      ui.label('Train Number'),
      trainInput,
      ui.el('div', { class: 'col mt-12' },
        ui.label('Date (YYYY-MM-DD, optional)'),
        dateInput,
      ),
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    ui.render(root, header, form, results);
  },
};

function renderPosition(res, ui) {
  const pos = ui.el('p', { class: 'text-sm bold', text: res.current_location_info || 'No current position reported.' });
  const card = ui.card('Current Position',
    ui.el('div', { class: 'row' },
      ui.el('span', { class: 'bold', text: res.train_name }),
      ui.badge(res.train_number, 'blue'),
    ),
    pos,
    ui.el('div', { class: 'row mt-8' },
      ui.badge(res.data_source || 'unknown', 'slate'),
    ),
  );
  return card;
}

function renderStations(res, ui) {
  const stations = res.stations || [];
  if (!stations.length) return ui.card('Stations', ui.notice('No station data returned.'));
  const rows = stations.map((s) => [
    s.name,
    s.code,
    ui.fmtTime(s.scheduled_arrival),
    statusBadge(s.status, ui),
  ]);
  return ui.card('Stations', ui.table(['Station', 'Code', 'Sch. Arrival', 'Status'], rows));
}

function statusBadge(status, ui) {
  const kind = status === 'departed' ? 'slate' : status === 'expected' ? 'amber' : 'blue';
  return `<span class="badge badge-${kind}">${status || 'scheduled'}</span>`;
}
})();
