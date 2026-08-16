/* live_status.js - "Spot Train" tab. Live train position from NTES
   (enquiry.indianrail.gov.in), with Railyatri as fallback. The backend always
   reports the real source in res.data_source and surfaces every run date NTES
   gives for the train in res.instances - exactly like NTES "Spot Train (Live
   Status)" shows its "Train Instances". No date is ever asked for; the backend
   resolves the current run itself.
   Live-data-only: renders exactly what GET /rail-api/live-status returns. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.live_status = {
  title: 'Spot Train',
  icon: '🚄',

  mount(root, ctx) {
    const ui = ctx.ui;

    const header = ui.card('Spot Train (Live Status)',
      ui.notice('Live position from NTES (enquiry.indianrail.gov.in), with Railyatri as fallback. Enter only the train number - no date needed.'),
    );

    const trainInput = ui.el('input', { class: 'input', placeholder: 'Train number or name', autocomplete: 'off' });

    /* IntelliSense: pick a train by number or name over the pre-warmed local
       list; the input is filled with the real number before spotting. */
    ctx.autocomplete.attach(trainInput, { type: 'train' });

    const results = ui.el('div', { class: 'col mt-12' });
    results.append(ui.emptyState('Enter a train number to spot it.'));

    const submit = ui.el('button', { class: 'btn', text: 'Spot Train' });

    function spot() {
      const train = trainInput.value.trim();

      if (!/^\d+$/.test(train)) {
        results.replaceChildren();
        results.append(ui.errorBox('Enter a valid train number (digits only).'));
        return;
      }

      results.replaceChildren();
      results.append(ui.spinner());
      submit.disabled = true;
      const reenable = () => { submit.disabled = false; };

      ctx.api.liveStatus(train)
        .then((res) => {
          results.replaceChildren();
          if (!res || res.ok === false) {
            results.append(ui.errorBox((res && res.error) ? res.error : 'Failed to load live status.'));
            return;
          }
          const instances = renderInstances(res, ui);
          if (instances) results.append(instances);
          results.append(renderPosition(res, ui));
          results.append(renderStations(res, ui));
        })
        .catch((err) => {
          results.replaceChildren();
          const msg = err && err.message ? err.message : String(err);
          results.append(ui.errorBox(`Failed to load live status: ${msg}`));
        })
        .finally(reenable);
    }

    submit.onclick = spot;
    trainInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') spot(); });

    const form = ui.card('Track',
      ui.label('Train Number'),
      trainInput,
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    ui.render(root, header, form, results);
  },
};

/* All run dates NTES reports for the train - the "Train Instances" list the
   NTES Spot Train (Live Status) page shows under the train search. */
function renderInstances(res, ui) {
  const instances = res.instances || [];
  if (!instances.length) return null;
  const rows = instances.map((i) => [
    i.start_date,
    i.position || '-',
    i.start_date === res.train_start_date ? '<span class="badge badge-blue">Current</span>' : '',
  ]);
  const headers = ['Start Date', 'Position', ''];
  return ui.card('Train Instances (dates from NTES)', ui.table(headers, rows));
}

function renderPosition(res, ui) {
  const pos = ui.el('p', { class: 'text-sm bold', text: res.current_location_info || 'No current position reported.' });
  const runInfo = res.train_start_date
    ? ui.el('p', { class: 'text-sm muted', text: `Run date: ${res.train_start_date}` })
    : null;
  const card = ui.card('Current Position',
    ui.el('div', { class: 'row' },
      ui.el('span', { class: 'bold', text: res.train_name }),
      ui.badge(res.train_number, 'blue'),
    ),
    pos,
    runInfo,
    ui.el('div', { class: 'row mt-8' },
      ui.badge(res.data_source || 'unknown', 'slate'),
    ),
  );
  return card;
}

function renderStations(res, ui) {
  const stations = res.stations || [];
  if (!stations.length) return ui.card('Stations', ui.notice('No station data returned.'));
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
