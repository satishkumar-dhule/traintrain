/* schedule.js - Schedule tab. Live timetable for a train number from
   GET /rail-api/schedule. Live-data-only: renders the real API response. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.schedule = {
  title: 'Schedule',
  icon: '🚉',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Train Schedule', ui.el('p', { class: 'text-sm muted', text: 'Live timetable from Railyatri.' }));

    const input = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'e.g. 12002' });
    const submit = ui.el('button', { class: 'btn', text: 'Get Schedule' });
    const form = ui.card('', ui.label('Train Number'), input, ui.el('div', { class: 'mt-8' }, submit));

    const results = ui.el('div');

    const submitForm = () => {
      const train = input.value.trim();
      if (!/^\d+$/.test(train)) {
        ui.render(results, ui.errorBox('Enter a valid train number (digits only).'));
        return;
      }
      ui.render(results, ui.spinner());
      submit.disabled = true;

      ctx.api.schedule(train)
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load schedule.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderResults(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load schedule: ${msg}`));
        })
        .finally(() => { submit.disabled = false; });
    };

    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    submit.addEventListener('click', submitForm);

    ui.render(root, header, form, results);
  },
};

function renderResults(s, ui) {
  const today = new Date().toLocaleString('en-GB', { timeZone: 'Asia/Kolkata', weekday: 'short' }).toUpperCase().slice(0, 3);

  const trainInfo = ui.card('Train',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.el('span', { class: 'bold', text: s.train_name || 'Unknown' }),
      ui.badge(s.train_number || '', 'blue'),
    ),
    ui.el('div', { class: 'text-sm muted mt-8' }, s.route_description || ''),
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge('Data source: ' + (s.data_source || 'unknown'), 'slate'),
    ),
  );

  const dayRow = ui.el('div', { class: 'row align-center mt-8' },
    ui.el('span', { class: 'label', text: 'Runs on' }),
    (s.running_days || []).map((d) => ui.badge(String(d).slice(0, 3).toUpperCase(), d.toUpperCase() === today ? 'green' : 'slate')),
  );
  trainInfo.append(dayRow);

  const stops = s.stops;
  const stations = ui.card('Stations',
    Array.isArray(stops) && stops.length
      ? ui.table(['Day', 'Code', 'Station', 'Arrival', 'Departure'],
          stops.map((st) => [
            ui.el('span', { text: st.day || '' }).outerHTML,
            ui.el('span', { class: 'mono', text: st.code || '' }).outerHTML,
            st.name || '',
            ui.fmtTime(st.arrival),
            ui.fmtTime(st.departure),
          ]))
      : ui.notice('No stops returned.'),
  );

  const meta = [];
  if (s.notice) meta.push(ui.notice(s.notice));
  if (s.cache_ttl) meta.push(ui.el('p', { class: 'text-sm muted', text: `Cached for ${s.cache_ttl} seconds.` }));
  const footer = ui.card('', ...meta);

  return [trainInfo, stations, footer];
}
})();
