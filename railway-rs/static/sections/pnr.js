/* sections/pnr.js - PNR section. Live enquiry against GET /rail-api/pnr with
   the captcha 428 retry flow (up to 3 total attempts). Deep links like
   #/pnr/2498761234 auto-submit. */

(() => {
window.Sections = window.Sections || {};

window.Sections.pnr = {
  mount(container, ctx, route) {
    const ui = ctx.ui;
    const MAX_ATTEMPTS = 3;
    const params = route.params || {};

    const header = ui.card('PNR Status',
      ui.el('p', { class: 'text-sm muted', text: 'Live PNR enquiry from Indian Railways (captcha protected)' }),
    );

    const input = ui.el('input', {
      class: 'input',
      autocomplete: 'off',
      maxlength: '10',
      inputmode: 'numeric',
      placeholder: '10 digit PNR',
    });

    const btn = ui.el('button', { class: 'btn', text: 'Check Status' });
    const results = ui.el('div');

    function submit() {
      const pnr = input.value.trim();
      if (!/^\d{10}$/.test(pnr)) {
        ui.render(results, ui.errorBox('PNR must be exactly 10 digits.'));
        return;
      }
      const setLoading = ui.withLoading(btn, 'Checking…');
      setLoading(true);
      ui.render(results, ui.spinner());

      fetchResult(pnr, null, MAX_ATTEMPTS, ctx, results, setLoading)
        .then((resolved) => {
          if (!resolved) return;
          if (resolved.kind === 'ok') {
            try {
              ui.render(results, ...renderResult(resolved.data, ctx));
            } catch (err) {
              const msg = err && err.stack ? err.stack : (err && err.message ? err.message : String(err));
              RailLog.error('pnr render threw:', msg);
              RailLog.error('pnr render data:', JSON.stringify(resolved.data));
              ui.render(results, ui.errorBox(`Rendering failed: ${msg}`));
            }
          } else if (resolved.kind === 'error') {
            ui.render(results, ui.errorBox(resolved.text));
          } else if (resolved.kind === 'notice') {
            ui.render(results, ui.notice(resolved.text));
          }
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Request failed: ${msg}`));
        })
        .finally(() => setLoading(false));
    }

    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
    btn.addEventListener('click', submit);

    ui.render(container, header,
      ui.queryCard([['PNR (10 digits)', input]], btn),
      results);

    if (params.pnr) {
      input.value = String(params.pnr);
      submit();
    }
  },
};

/* Resolve the PNR into a plain renderable result. */
function fetchResult(pnr, captcha, attemptsLeft, ctx, results, setLoading) {
  return ctx.api.pnr(pnr, captcha).then((res) => {
    if (!res || res.ok !== false) {
      return res ? { kind: 'ok', data: res } : { kind: 'error', text: 'No response from server.' };
    }
    if (res.status === 428 && attemptsLeft > 1) {
      return ctx.captcha.show(res.body).then((params) => {
        if (!params) return { kind: 'notice', text: 'Check cancelled.' };
        return fetchResult(pnr, {
          session_id: params.session_id,
          source: params.source,
          text: params.text,
        }, attemptsLeft - 1, ctx, results, setLoading);
      });
    }
    return { kind: 'error', text: res.error || 'Request failed.' };
  });
}

/* Build the DOM for a successful response. */
function renderResult(data, ctx) {
  const ui = ctx.ui;

  const train = ui.card('Train',
    ui.el('div', { class: 'row mt-8' },
      ui.el('span', { class: 'bold', text: data.train_name || 'Unknown train' }),
      data.train_number != null && data.train_number !== '' ? ui.badge(String(data.train_number), 'blue') : null,
    ),
    data.journey_date
      ? ui.el('div', { class: 'text-sm muted mt-8', text: `Journey date: ${data.journey_date}` })
      : null,
    ui.el('div', { class: 'row mt-8' },
      stationCell(data.from, ui),
      ui.el('span', { class: 'muted', text: '→' }),
      stationCell(data.to, ui),
    ),
  );

  const passengers = (Array.isArray(data.passengers) && data.passengers.length)
    ? ui.card('Passengers', ui.table(
        ['Booking Status', 'Current Status', 'Coach', 'Berth'],
        data.passengers.map((p) => [
          ui.esc(p && p.booking_status),
          ui.esc(p && p.current_status),
          ui.esc(p && p.coach),
          ui.esc(p && p.berth),
        ]),
      ))
    : ui.card('Passengers', ui.notice('No passenger data returned.'));

  const meta = ui.card('Details',
    ui.el('div', { class: 'row mt-8' },
      data.data_source ? ui.badge(String(data.data_source), 'green') : null,
      data.freshness ? ui.el('span', { class: 'text-sm muted', text: data.freshness }) : null,
      data.last_updated ? ui.el('span', { class: 'text-sm muted', text: `Updated ${data.last_updated}` }) : null,
    ),
    data.notice ? ui.el('p', { class: 'text-sm muted mt-8', text: data.notice }) : null,
  );

  return [train, passengers, meta];
}

function stationCell(s, ui) {
  if (!s || !s.name) return ui.el('span', { class: 'muted', text: '—' });
  const sub = [s.code, ui.fmtTime(s.time), s.day ? `Day ${s.day}` : ''].filter(Boolean).join(' · ');
  return ui.el('div', { class: 'col' },
    ui.el('span', { class: 'bold', text: s.name }),
    ui.el('span', { class: 'text-sm muted', text: sub }),
  );
}
})();
