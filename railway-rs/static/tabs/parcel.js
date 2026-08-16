/* parcel.js - "Parcel SPL" tab. Currently running parcel special trains from
   GET /rail-api/ntes/parcel. Auto-loads on mount; Refresh re-fetches. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.parcel = {
  title: 'Parcel SPL',
  icon: '📦',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Parcel Special Trains',
      ui.el('p', { class: 'text-sm muted', text: 'Currently running time-tabled parcel special trains (NTES)' }),
    );

    const refresh = ui.el('button', { class: 'btn', text: 'Refresh' });
    header.append(ui.el('div', { class: 'row mt-12' }, refresh));

    const results = ui.el('div', { class: 'mt-12' });

    const fetchParcel = () => {
      ui.render(results, ui.spinner());
      refresh.disabled = true;

      ctx.api.parcel()
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load parcel special trains.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderResults(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load parcel special trains: ${msg}`));
        })
        .finally(() => { refresh.disabled = false; });
    };

    refresh.addEventListener('click', fetchParcel);

    ui.render(root, header, results);
    fetchParcel();
  },
};

function renderResults(res, ui) {
  const source = ui.card('',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Parcel Special Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['No.', 'Train', 'Route', 'Days', 'Validity', 'From', 'To', 'Travel'],
          trains.map((t, i) => [
            String(i + 1),
            `${t.number || ''} ${t.name || ''}`,
            t.route || '',
            t.days_of_run || '',
            `${t.validity_from || ''} → ${t.validity_to || ''}`,
            `${t.source_code || ''} ${t.source_time || ''}`,
            `${t.dest_code || ''} ${t.dest_time || ''}`,
            t.travel_time || '',
          ]))
      : ui.notice('No parcel special trains found.'),
  );

  return [source, list];
}
})();
