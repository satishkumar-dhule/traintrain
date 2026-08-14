/* exceptional.js - Exceptional tab. Live-only: cancelled / rescheduled /
   diverted trains from GET /rail-api/ntes/exceptional?type=... */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.exceptional = {
  title: 'Exceptional',
  icon: '⚠️',

  mount(root, ctx) {
    const ui = ctx.ui;
    let loaded = false;

    const header = ui.card('Exceptional Trains',
      ui.el('p', { class: 'text-sm muted', text: 'Cancelled / rescheduled / diverted (NTES)' }),
    );

    const select = ui.el('select', { class: 'input' },
      ui.el('option', { value: 'cancelled', text: 'cancelled' }),
      ui.el('option', { value: 'rescheduled', text: 'rescheduled' }),
      ui.el('option', { value: 'diverted', text: 'diverted' }),
    );

    const loadBtn = ui.el('button', { class: 'btn', text: 'Load' });
    const form = ui.card('',
      ui.label('Type'),
      select,
      ui.el('div', { class: 'row mt-12' }, loadBtn),
    );

    const results = ui.el('div', { class: 'col' });
    ui.render(root, header, form, results);

    function load() {
      const type = select.value;
      const setLoading = ui.withLoading(loadBtn, 'Loading…');
      setLoading(true);
      results.replaceChildren(ui.spinner());

      ctx.api.exceptional(type)
        .then((res) => {
          setLoading(false);
          results.replaceChildren();
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load exceptional trains.';
            results.append(ui.errorBox(msg));
            return;
          }
          renderResult(res, type, ui, results);
        })
        .catch((err) => {
          setLoading(false);
          const msg = err && err.message ? err.message : String(err);
          results.replaceChildren(ui.errorBox(`Failed to load exceptional trains: ${msg}`));
        });
    }

    loadBtn.onclick = load;
    if (!loaded) {
      loaded = true;
      load();
    }
  },
};

function renderResult(res, requestedType, ui, results) {
  const type = res.type || requestedType;
  const typeBadge = ui.badge(String(type).toUpperCase(), 'red');
  const sourceBadge = res.data_source ? ui.badge(res.data_source, 'blue') : null;

  const typeCard = ui.card('Type',
    ui.el('div', { class: 'row mt-8' }, typeBadge, sourceBadge),
  );

  const trains = Array.isArray(res.trains) ? res.trains : [];
  let listCard;
  if (!trains.length) {
    listCard = ui.card('List', ui.notice('No exceptional trains reported.'));
  } else {
    listCard = ui.card('List',
      ui.table(['No.', 'Train', 'Date', 'Reason'],
        trains.map((t) => [t.number, t.name, t.date, t.reason])),
    );
  }

  results.append(typeCard, listCard);
}
})();
