/* heritage.js - "Heritage" tab. Heritage trains of Indian Railways from
   GET /rail-api/ntes/heritage. Live-data-only: renders the real API response
   or an honest error. */

(() => {
window.Tabs = window.Tabs || {};

const SELECTIONS = [
  [0, 'All Heritage Trains'],
  [1, 'Kalka Shimla Railway'],
  [2, 'Matheran Hill Railway'],
  [3, 'Kangra Valley Railway'],
  [4, 'Nilgiri Mountain Railway'],
  [5, 'Darjeeling Himalayan Railway'],
];

window.Tabs.heritage = {
  title: 'Heritage',
  icon: '🚞',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Heritage Trains',
      ui.el('p', { class: 'text-sm muted', text: 'Heritage trains of Indian Railways (NTES)' }),
    );

    const select = ui.el('select', { class: 'input' });
    SELECTIONS.forEach(([value, label]) => {
      select.append(ui.el('option', { value: String(value), text: label }));
    });

    const submit = ui.el('button', { class: 'btn', text: 'Get Trains' });
    const form = ui.card('',
      ui.label('Heritage Line'),
      select,
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    const results = ui.el('div', { class: 'mt-12' });

    const submitForm = () => {
      const selectionValue = select.value;
      ui.render(results, ui.spinner());
      submit.disabled = true;

      ctx.api.heritage(selectionValue)
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load heritage trains.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderResults(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load heritage trains: ${msg}`));
        })
        .finally(() => { submit.disabled = false; });
    };

    submit.addEventListener('click', submitForm);

    ui.render(root, header, form, results);
  },
};

function renderResults(res, ui) {
  const summary = ui.card('Summary',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge('Total: ' + (res.total ?? 0), 'blue'),
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['Train', 'Runs', 'From', 'To', 'Duration'],
          trains.map((t) => [
            `${t.number} ${t.name}`,
            `${t.runs} | ${t.train_type}`,
            `${t.source_station} (${t.source_code}) ${t.source_time}`,
            `${t.dest_station} (${t.dest_code}) ${t.dest_time}`,
            t.duration,
          ]))
      : ui.notice('No heritage trains found.'),
  );

  return [summary, list];
}
})();
