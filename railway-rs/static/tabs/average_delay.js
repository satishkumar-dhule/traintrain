/* average_delay.js - "Avg Delay" tab. Average arrival/departure delay per
   station over the last 7 days from GET /rail-api/ntes/average-delay.
   Live-data-only: renders the real API response or an honest error. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.average_delay = {
  title: 'Avg Delay',
  icon: '⏰',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Average Delay',
      ui.el('p', { class: 'text-sm muted', text: 'Average arrival/departure delay over the last 7 days (NTES)' }),
    );

    const trainInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Train No. (5 digits)' });
    const submit = ui.el('button', { class: 'btn', text: 'Check Delay' });
    const form = ui.card('',
      ui.label('Train Number'),
      trainInput,
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    const results = ui.el('div', { class: 'mt-12' });

    const submitForm = () => {
      const train = trainInput.value.trim();
      if (!train) {
        ui.render(results, ui.errorBox('Train number is required.'));
        return;
      }
      ui.render(results, ui.spinner());
      submit.disabled = true;

      ctx.api.averageDelay(train)
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load average delay.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderResults(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load average delay: ${msg}`));
        })
        .finally(() => { submit.disabled = false; });
    };

    trainInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    submit.addEventListener('click', submitForm);

    ui.render(root, header, form, results);
  },
};

function renderResults(res, ui) {
  const train = ui.card('Train',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.el('span', { class: 'mono', text: res.train_no || '' }),
      ui.el('span', { class: 'bold', text: (res.train_no && res.train_name ? ' ' : '') + (res.train_name || '') }),
      ui.badge(res.days_of_run || ''),
      ui.badge(res.train_type || '', 'slate'),
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const stations = res.stations || [];
  const list = ui.card('Stations',
    Array.isArray(stations) && stations.length
      ? ui.table(['Sr.', 'Station', 'Code', 'Arr. Delay', 'Dep. Delay'],
          stations.map((st) => [
            st.sr || '',
            st.name || '',
            st.code || '',
            st.arrival_delay || '—',
            st.departure_delay || '—',
          ]))
      : ui.notice('No delay data found.'),
  );

  return [train, list];
}
})();
