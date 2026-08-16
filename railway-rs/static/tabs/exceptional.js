/* exceptional.js - Exceptional tab. Live-only: per-train exception calendar
   from GET /rail-api/ntes/exceptional?train=...&type=... . The NTES batch
   "Exceptional Trains" form is disabled server-side, so the backend checks one
   train at a time (cached 2h) - this tab asks for a train number, exactly like
   the Spot Train tab. Renders exactly what the API returns. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.exceptional = {
  title: 'Exceptional',
  icon: '⚠️',

  mount(root, ctx) {
    const ui = ctx.ui;

    const header = ui.card('Exceptional Trains',
      ui.notice('Per-train exception calendar from NTES (cached 2 hours). Enter a train number to see its cancelled / rescheduled / diverted dates.'),
    );

    const trainInput = ui.el('input', { class: 'input', placeholder: 'Train number', autocomplete: 'off' });
    ctx.autocomplete.attach(trainInput, { type: 'train' });

    const typeSelect = ui.el('select', { class: 'input' },
      ui.el('option', { value: '', text: 'all kinds' }),
      ui.el('option', { value: 'cancelled', text: 'cancelled' }),
      ui.el('option', { value: 'rescheduled', text: 'rescheduled' }),
      ui.el('option', { value: 'diverted', text: 'diverted' }),
    );

    const submit = ui.el('button', { class: 'btn', text: 'Check Train' });
    const results = ui.el('div', { class: 'col mt-12' });
    results.append(ui.emptyState('Enter a train number to check its exceptional dates.'));

    function load() {
      const train = trainInput.value.trim();
      const type = typeSelect.value;

      if (!/^\d{4,5}$/.test(train)) {
        results.replaceChildren();
        results.append(ui.errorBox('Enter a valid train number (4-5 digits).'));
        return;
      }

      results.replaceChildren();
      results.append(ui.spinner());
      submit.disabled = true;
      const reenable = () => { submit.disabled = false; };

      ctx.api.exceptional(train, type || undefined)
        .then((res) => {
          results.replaceChildren();
          if (!res || res.ok === false) {
            results.append(ui.errorBox((res && res.error) ? res.error : 'Failed to load exceptional dates.'));
            return;
          }
          renderResult(res, ui, results);
        })
        .catch((err) => {
          results.replaceChildren();
          const msg = err && err.message ? err.message : String(err);
          results.append(ui.errorBox(`Failed to load exceptional dates: ${msg}`));
        })
        .finally(reenable);
    }

    submit.onclick = load;
    trainInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') load(); });

    const form = ui.card('Check Train',
      ui.label('Train Number'),
      trainInput,
      ui.label('Kind'),
      typeSelect,
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    ui.render(root, header, form, results);
  },
};

function renderResult(res, ui, results) {
  const t = res.train || {};
  const name = t.name ? `${t.number} - ${t.name}` : `Train ${t.number || '?'}`;
  const route = (t.source && t.destination) ? `${t.source} → ${t.destination}` : null;
  const days = Array.isArray(t.days_of_run) && t.days_of_run.length
    ? ui.el('div', { class: 'row mt-8' }, t.days_of_run.map((d) => ui.badge(d.toUpperCase(), 'slate')))
    : null;

  const headerCard = ui.card('Train',
    ui.el('div', { class: 'row' },
      ui.el('span', { class: 'bold', text: name }),
    ),
    route ? ui.el('p', { class: 'text-sm muted', text: route }) : null,
    days,
    ui.el('div', { class: 'row mt-8' },
      ui.badge(res.data_source || 'unknown', 'slate'),
      res.cache_ttl ? ui.badge(`cached ${Math.round(res.cache_ttl / 60)} min`, 'blue') : null,
    ),
  );

  const exceptions = Array.isArray(res.exceptions) ? res.exceptions : [];
  let listCard;
  if (res.message) {
    listCard = ui.card('Train Exception Info', ui.notice(res.message));
  } else if (!exceptions.length) {
    listCard = ui.card('Exceptional Dates',
      ui.notice(`No exceptional details found for train ${t.number || ''}.`),
    );
  } else {
    listCard = ui.card('Exceptional Dates',
      ui.table(['Date', 'Kind', 'Note'],
        exceptions.map((e) => [e.date, kindBadge(e.kind), e.note || '-'])),
    );
  }

  results.append(headerCard, listCard);
}

/* HTML string; ui.table renders cells via innerHTML. */
function kindBadge(kind) {
  const color = kind === 'cancelled' ? 'red'
    : kind === 'rescheduled' ? 'amber'
    : kind === 'diverted' ? 'amber'
    : kind === 'new_source' ? 'green'
    : kind === 'new_destination' ? 'blue'
    : 'slate';
  return `<span class="badge badge-${color}">${kind || '-'}</span>`;
}
})();
