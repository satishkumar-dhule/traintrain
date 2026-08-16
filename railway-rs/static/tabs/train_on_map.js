/* train_on_map.js - "Train Map" tab. Route map of a train with an optional
   live spot position from GET /rail-api/ntes/train-on-map. The backend
   resolves the route/track polyline with station coordinates from the local
   dataset and, when a station code is given, the live spot view (current +
   journey station, per-halt expected times and delay badges).
   Live-data-only: renders exactly what the API returns or an honest error. */

(() => {
window.Tabs = window.Tabs || {};

window.Tabs.train_on_map = {
  title: 'Train Map',
  icon: '🗺️',

  mount(root, ctx) {
    const ui = ctx.ui;
    const header = ui.card('Train Map',
      ui.el('p', { class: 'text-sm muted', text: 'Route map of a train; a station code adds the live spot view (NTES).' }),
    );

    const trainInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Train No. (5 digits)' });
    const stationInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Station code (optional, e.g. NDLS)' });
    const submit = ui.el('button', { class: 'btn', text: 'Show Map' });
    const form = ui.card('',
      ui.label('Train Number'),
      trainInput,
      ui.label('Station Code'),
      stationInput,
      ui.el('div', { class: 'row mt-12' }, submit),
    );

    const results = ui.el('div', { class: 'mt-12' });

    const submitForm = () => {
      const train = trainInput.value.trim();
      if (!/^\d{5}$/.test(train) || train === '00000') {
        ui.render(results, ui.errorBox('Enter a valid train number (5 digits).'));
        return;
      }
      const station = stationInput.value.trim().toUpperCase();
      if (station && !/^[A-Z0-9]{4}$/.test(station)) {
        ui.render(results, ui.errorBox('Station code must be 4 letters/digits, or left blank.'));
        return;
      }
      ui.render(results, ui.spinner());
      submit.disabled = true;

      ctx.api.trainOnMap(train, station || null)
        .then((res) => {
          if (!res || res.ok === false) {
            const msg = res && res.error ? res.error : 'Failed to load train map.';
            ui.render(results, ui.errorBox(msg));
            return;
          }
          ui.render(results, ...renderResults(res, ui));
        })
        .catch((err) => {
          const msg = err && err.message ? err.message : String(err);
          ui.render(results, ui.errorBox(`Failed to load train map: ${msg}`));
        })
        .finally(() => { submit.disabled = false; });
    };

    trainInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    stationInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
    submit.addEventListener('click', submitForm);

    ui.render(root, header, form, results);
  },
};

function renderResults(res, ui) {
  const parts = [];

  parts.push(ui.card('Train',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.el('span', { class: 'mono', text: res.train_no || '' }),
      ui.el('span', { class: 'bold', text: (res.train_no && res.train_name ? ' ' : '') + (res.train_name || '') }),
      ui.badge(res.source_code || ''),
      ui.badge(res.dest_code || '', 'blue'),
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
    ui.el('p', { class: 'text-sm muted mt-8', text: `${res.source || ''} → ${res.destination || ''}` }),
  ));

  if (res.current_station) parts.push(...renderLive(res, ui));
  parts.push(renderMap(res, ui));
  parts.push(renderStations(res, ui));

  return parts;
}

/* Live spot cards: the "current" station (where the train is) and the queried
   journey station with its expected/actual times, delay and platform. */
function renderLive(res, ui) {
  const current = res.current_station || {};
  const cards = [ui.card('Live Position',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge(`● CURRENT: ${current.code || '?'}`, 'blue'),
    ),
  )];

  const j = res.journey_station;
  if (!j) return cards;

  const rows = [
    ['Station', `${j.name || ''} (${j.code || ''})`],
    ['Label', j.label || ''],
    ['Expected arrival', j.expected_arrival || '—'],
    ['Actual arrival', j.actual_arrival || '—'],
    ['Delay', j.delay_status || '—'],
    ['Platform', j.platform || '—'],
  ];
  cards.push(ui.card('Journey Station',
    rows.map(([k, v]) => ui.el('div', { class: 'row mt-4' },
      ui.el('span', { class: 'text-sm muted', text: `${k}: ` }),
      ui.el('span', { class: 'text-sm bold', text: v }),
    )),
  ));

  return cards;
}

/* Compact inline SVG route map: a linear lat/lng projection (no external
   libs). Circles mark the halts, the source/destination and current station
   get labels, and the current station is ringed in red. */
function renderMap(res, ui) {
  const pts = (res.route || []).filter((s) => typeof s.lat === 'number' && typeof s.lng === 'number');
  if (pts.length < 2) {
    return ui.card('Route Map', ui.notice('No coordinate data for this route.'));
  }

  const lats = pts.map((s) => s.lat);
  const lngs = pts.map((s) => s.lng);
  const minLat = Math.min(...lats), maxLat = Math.max(...lats);
  const minLng = Math.min(...lngs), maxLng = Math.max(...lngs);
  const W = 600, H = 180, pad = 22;
  const span = (v, min, max) => (v - min) / (max - min || 1);
  const x = (lng) => (pad + span(lng, minLng, maxLng) * (W - 2 * pad)).toFixed(1);
  const y = (lat) => (H - pad - span(lat, minLat, maxLat) * (H - 2 * pad)).toFixed(1);
  const currentCode = res.current_station && res.current_station.code;

  const line = pts.map((s) => `${x(s.lng)},${y(s.lat)}`).join(' ');

  let dots = '';
  let labels = '';
  pts.forEach((s, i) => {
    const cx = x(s.lng), cy = y(s.lat);
    const isCurrent = currentCode && s.code === currentCode;
    const labelled = isCurrent || i === 0 || i === pts.length - 1;
    dots += isCurrent
      ? `<circle cx="${cx}" cy="${cy}" r="6" fill="none" stroke="#dc2626" stroke-width="2"/>`
      : '';
    dots += `<circle cx="${cx}" cy="${cy}" r="${isCurrent ? 4 : 3}" fill="${isCurrent ? '#dc2626' : '#2563eb'}"/>`;
    if (labelled) {
      labels += `<text x="${cx}" y="${Number(cy) - 8}" text-anchor="middle" font-size="10" fill="#475569">${esc(s.code || '')}</text>`;
    }
  });

  const svg = `<svg viewBox="0 0 ${W} ${H}" style="width:100%;height:auto;display:block" role="img" aria-label="Route map of ${esc(res.train_no || '')}">
    <polyline points="${line}" fill="none" stroke="#2563eb" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>
    ${dots}
    ${labels}
  </svg>`;

  return ui.card('Route Map', ui.el('div', { class: 'map-wrap' }, ui.el('div', { html: svg })));
}

function renderStations(res, ui) {
  const route = res.route || [];
  if (!route.length) return ui.card('Stations', ui.notice('No station data returned.'));
  const currentCode = res.current_station && res.current_station.code;

  const headers = ['#', 'Station', 'Code', 'Arr', 'Dep', 'Day', 'Dist', 'Exp. Arr', 'Exp. Dep', 'Delay', 'Status'];
  const rows = route.map((st, i) => {
    const marker = st.code === currentCode
      ? '<span style="color:#dc2626;font-weight:bold">●</span> '
      : '';
    return [
      String(i + 1),
      marker + esc(st.name || ''),
      esc(st.code || ''),
      st.arrival || '—',
      st.departure || '—',
      st.day || '',
      st.distance || '',
      st.expected_arrival || '',
      st.expected_departure || '',
      delayCell(st.arrival_delay, st.departure_delay),
      statusBadge(st.arrival_delay, st.departure_delay),
    ];
  });

  return ui.card('Stations', ui.table(headers, rows));
}

/* "Arr <delay badge> · Dep <delay badge>"; blank when the spot view gave no
   delay info for the halt. */
function delayCell(arr, dep) {
  const kind = (d) => (d === 'On Time' ? 'green' : 'amber');
  const parts = [];
  if (arr) parts.push(`Arr <span class="badge badge-${kind(arr)}">${esc(arr)}</span>`);
  if (dep) parts.push(`Dep <span class="badge badge-${kind(dep)}">${esc(dep)}</span>`);
  return parts.join(' · ') || '<span class="muted">—</span>';
}

function statusBadge(arr, dep) {
  const parts = [arr, dep].filter(Boolean);
  if (!parts.length) return '<span class="muted">—</span>';
  const onTime = parts.every((p) => p === 'On Time');
  return `<span class="badge badge-${onTime ? 'green' : 'amber'}">${onTime ? 'On Time' : 'Delayed'}</span>`;
}

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
  ));
}
})();
