/* sections/track.js - Track section. Views: spot, schedule, map, delay,
   exceptions, journey. Deep links like #/train/12559/spot auto-submit; the
   map view delegates to the retained train_on_map tab. Live-data-only. */

(() => {
window.Sections = window.Sections || {};

window.Sections.track = {
  mount(container, ctx, route) {
    const p = route.params || {};
    const view = route.view || 'spot';
    switch (view) {
      case 'spot': viewSpot(container, ctx, p); break;
      case 'schedule': viewSchedule(container, ctx, p); break;
      case 'map': viewMap(container, ctx, p); break;
      case 'delay': viewDelay(container, ctx, p); break;
      case 'exceptions': viewExceptions(container, ctx, p); break;
      case 'journey': viewJourney(container, ctx, p); break;
      default: viewSpot(container, ctx, p);
    }
  },
};

function fillInput(value, input) {
  if (value && input) input.value = String(value).trim();
}

/* ---------- spot ---------- */

function viewSpot(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Spot Train (Live Status)',
    ui.notice('Live position from NTES (enquiry.indianrail.gov.in), with Railyatri as fallback. Enter only the train number - no date needed.'),
  );

  const { wrap, input } = ui.trainInput('Train number or name');
  const submit = ui.el('button', { class: 'btn', text: 'Spot Train' });
  const results = ui.el('div', { class: 'col mt-12' });
  results.append(ui.emptyState('Enter a train number to spot it.'));
  let lastRes = null;

  function spot() {
    const raw = input.value.trim();
    const train = raw.replace(/\s*-\s*.+$/, '').replace(/[^\d]/g, '');
    if (!/^\d+$/.test(train)) {
      ui.render(results, ui.errorBox('Enter a valid train number (digits only).'));
      return;
    }
    ui.fetchFlow(results, () => ctx.api.liveStatus(train), { button: submit, failText: 'Failed to load live status' })
      .then((res) => {
        if (!res) return;
        lastRes = res;
        renderAll(res);
      });
  }

  function renderAll(res) {
    const parts = [];
    const tabCard = renderInstanceTabs(res, ui, showInstance);
    if (tabCard) parts.push(tabCard);
    showInstance(res, 0, parts);
  }

  /* Render the selected instance's position + stations into `parts`, then
     call ui.render.  When `extra` is provided (initial render) the existing
     cards are prepended; on tab switch only the content area is replaced. */
  function showInstance(res, idx, extra) {
    const instances = res.instances || [];
    const inst = instances[idx] || {};
    const hasStops = Array.isArray(inst.stops) && inst.stops.length;
    const partList = extra || [];
    if (!extra) {
      partList.push(renderInstanceTabs(res, ui, showInstance));
    }
    if (hasStops) {
      partList.push(
        renderInstancePosition(res, inst, ui),
        renderInstanceStations(inst, ui),
      );
    } else {
      partList.push(renderPosition(res, ui), renderStations(res, ui));
    }
    ui.render(results, ...partList);
  }

  submit.onclick = spot;
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') spot(); });

  const form = ui.queryCard([['Train Number', wrap]], submit);
  ui.render(container, header, form, results);

  if (params.train) { fillInput(params.train, input); spot(); }
}

/* NTES start date `14-Aug-2026` -> wire format `2026-08-14` (empty passthrough). */
function ntesDateToIso(d) {
  const m = /^(\d{1,2})-([A-Za-z]{3})-(\d{4})$/.exec(d || '');
  if (!m) return d || '';
  const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const mi = months.indexOf(m[2][0].toUpperCase() + m[2].slice(1).toLowerCase());
  if (mi < 0) return d;
  return m[3] + '-' + String(mi + 1).padStart(2, '0') + '-' + String(Number(m[1])).padStart(2, '0');
}

/* Tabbed instance picker.  Each tab carries its own position + stops from
   the single NTES response; switching is instant (no re-fetch). */
function renderInstanceTabs(res, ui, onSwitch) {
  const instances = res.instances || [];
  if (!instances.length) return null;
  const active = res.train_start_date;
  const tabs = instances.map((inst, i) => ({
    label: inst.start_date + (inst.start_date === active ? ' \u2022' : ''),
    active: inst.start_date === active,
    click: () => onSwitch(res, i),
  }));
  const rows = instances.map((inst) => {
    const cur = inst.start_date === active;
    return [
      cur ? ui.el('span', { class: 'bold', text: inst.start_date }) : inst.start_date,
      cur ? ui.badge('Current', 'blue') : null,
      inst.position || '-',
    ];
  });
  return ui.card(
    'Train Instances (tap a date to switch the run)',
    ui.table(['Start Date', 'Position'], rows),
  );
}

/* Position card for an instance whose stops came from the NTES response. */
function renderInstancePosition(res, inst, ui) {
  const instAtSrc = inst.at_src === 'true';
  const instAtDst = inst.at_dstn === 'true';
  const stops = inst.stops || [];
  let location;
  if (instAtDst) {
    const last = stops[stops.length - 1] || {};
    location = 'Arrived at ' + (last.name || 'destination') + '.';
  } else if (instAtSrc) {
    location = 'Train at ' + (stops[0] ? stops[0].name : 'origin') + ' (origin).';
  } else {
    location = inst.position || 'Running; position awaiting update.';
  }
  const runInfo = inst.start_date
    ? ui.el('p', { class: 'text-sm muted', text: 'Run date: ' + inst.start_date })
    : null;
  return ui.card('Current Position',
    ui.el('div', { class: 'row' },
      ui.el('span', { class: 'bold', text: res.train_name }),
      ui.badge(res.train_number, 'blue'),
    ),
    ui.el('p', { class: 'text-sm bold', text: location }),
    runInfo,
    ui.el('div', { class: 'row mt-8' },
      ui.badge(res.data_source || 'unknown', 'slate'),
    ),
  );
}

/* Stations table from an instance's per-run stops. */
function renderInstanceStations(inst, ui) {
  const stations = inst.stops || [];
  if (!stations.length) return ui.card('Stations', ui.notice('No station data for this run.'));
  const showActual = stations.some((s) => s.actual_arrival);
  const headers = ['Station', 'Code', 'Sch. Arrival'];
  if (showActual) headers.push('Act. Arrival');
  headers.push('Delay', 'Status');
  const rows = stations.map((s) => {
    const cells = [s.name, s.code, ui.fmtTime(s.scheduled_arrival)];
    if (showActual) cells.push(ui.fmtTime(s.actual_arrival));
    cells.push(ui.delay(s.delay_minutes), ui.statusCell(s.status));
    return cells;
  });
  return ui.card('Stations', ui.table(headers, rows));
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
    cells.push(ui.delay(s.delay_minutes), ui.statusCell(s.status));
    return cells;
  });
  return ui.card('Stations', ui.table(headers, rows));
}

/* ---------- schedule ---------- */

function viewSchedule(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Train Schedule', ui.el('p', { class: 'text-sm muted', text: 'Live timetable from NTES (enquiry.indianrail.gov.in), with Railyatri as fallback.' }));

  const { wrap, input } = ui.trainInput('e.g. 12002 or SHATABDI');
  const submit = ui.el('button', { class: 'btn', text: 'Get Schedule' });
  const results = ui.el('div');

  function submitForm() {
    const train = input.value.trim();
    if (!/^\d+$/.test(train)) {
      ui.render(results, ui.errorBox('Enter a valid train number (digits only).'));
      return;
    }
    ui.fetchFlow(results, () => ctx.api.schedule(train), { button: submit, failText: 'Failed to load schedule' })
      .then((res) => { if (res) ui.render(results, ...renderSchedule(res, ui)); });
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  ui.render(container, header, ui.queryCard([['Train Number', wrap]], submit), results);

  if (params.train) { fillInput(params.train, input); submitForm(); }
}

function renderSchedule(s, ui) {
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

/* ---------- map (delegates to the retained train_on_map tab) ---------- */

function viewMap(container, ctx, params) {
  window.Tabs.train_on_map.mount(container, ctx);
  if (params && params.train) {
    const input = container.querySelector('input.input');
    const submit = container.querySelector('.btn');
    if (input && submit) { fillInput(params.train, input); submit.click(); }
  }
}

/* ---------- delay ---------- */

function viewDelay(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Average Delay',
    ui.el('p', { class: 'text-sm muted', text: 'Average arrival/departure delay over the last 7 days (NTES)' }),
  );

  const { wrap, input } = ui.trainInput('Train No. (5 digits)');
  const submit = ui.el('button', { class: 'btn', text: 'Check Delay' });
  const results = ui.el('div', { class: 'mt-12' });

  function submitForm() {
    const train = input.value.trim();
    if (!train) {
      ui.render(results, ui.errorBox('Train number is required.'));
      return;
    }
    ui.fetchFlow(results, () => ctx.api.averageDelay(train), { button: submit, failText: 'Failed to load average delay' })
      .then((res) => { if (res) ui.render(results, ...renderDelay(res, ui)); });
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  ui.render(container, header, ui.queryCard([['Train Number', wrap]], submit), results);

  if (params.train) { fillInput(params.train, input); submitForm(); }
}

function renderDelay(res, ui) {
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

/* ---------- exceptions ---------- */

function viewExceptions(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Exceptional Trains',
    ui.notice('Per-train exception calendar from NTES (cached 2 hours). Enter a train number to see its cancelled / rescheduled / diverted dates.'),
  );

  const { wrap, input } = ui.trainInput('Train number');
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
    const train = input.value.trim();
    const type = typeSelect.value;
    if (!/^\d{4,5}$/.test(train)) {
      ui.render(results, ui.errorBox('Enter a valid train number (4-5 digits).'));
      return;
    }
    ui.fetchFlow(results, () => ctx.api.exceptional(train, type || undefined), { button: submit, failText: 'Failed to load exceptional dates' })
      .then((res) => { if (res) renderExceptions(res, ui, results); });
  }

  submit.onclick = load;
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') load(); });

  const form = ui.queryCard(
    [['Train Number', wrap], ['Kind', typeSelect]],
    submit,
  );

  ui.render(container, header, form, results);

  if (params.train) { fillInput(params.train, input); load(); }
}

function renderExceptions(res, ui, results) {
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

  ui.render(results, headerCard, listCard);
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

/* ---------- journey ---------- */

function viewJourney(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Journey Station Basis',
    ui.notice('Enter a train number to load its journey stations, then pick one to see the live run from that station (NTES).'),
  );

  const { wrap, input } = ui.trainInput('Train No. (5 digits)');
  const loadBtn = ui.el('button', { class: 'btn', text: 'Load Stations' });
  const results = ui.el('div', { class: 'col mt-12' });
  let train = '';

  const showBasis = (stationCode) => {
    ui.fetchFlow(results, () => ctx.api.journeyBasis(train, stationCode), { failText: 'Failed to load journey basis' })
      .then((res) => { if (res) ui.render(results, ...renderBasis(res, ui)); });
  };

  const loadStations = () => {
    train = input.value.trim();
    RailLog.action('journey_basis', 'load_stations', { train_raw: train });
    if (!/^\d{5}$/.test(train)) {
      RailLog.action('journey_basis', 'validation', { error: 'train must be 5 digits', train_raw: train });
      ui.render(results, ui.errorBox('Enter a valid 5-digit train number.'));
      return;
    }
    ui.fetchFlow(results, () => ctx.api.journeyStations(train), { button: loadBtn, failText: 'Failed to load journey stations' })
      .then((res) => { if (res) ui.render(results, ...renderStationPicker(res, ui, showBasis)); });
  };

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') loadStations(); });
  loadBtn.addEventListener('click', loadStations);

  ui.render(container, header, ui.queryCard([['Train Number', wrap]], loadBtn), results);

  if (params.train) { fillInput(params.train, input); loadStations(); }
}

function renderStationPicker(res, ui, onPick) {
  const stations = Array.isArray(res.stations) ? res.stations : [];
  if (!stations.length) {
    return [ui.card('Journey Stations', ui.notice('No journey stations returned for this train.'))];
  }
  const select = ui.el('select', { class: 'input' },
    stations.map((s) => ui.el('option', { value: s.code, text: `${s.code} - ${s.name}` })),
  );
  const go = ui.el('button', { class: 'btn', text: 'Show Journey Basis' });
  go.addEventListener('click', () => onPick(select.value));

  return [
    ui.card('Journey Stations',
      ui.el('div', { class: 'row mt-8' },
        ui.el('span', { class: 'mono', text: res.train_no || '' }),
        ui.el('span', { class: 'text-sm muted', text: `${stations.length} station(s)` }),
        ui.badge('Source: ' + (res.data_source || 'unknown'), 'slate'),
      ),
      ui.label('Journey Station'),
      select,
      ui.el('div', { class: 'row mt-12' }, go),
    ),
  ];
}

function renderBasis(res, ui) {
  const cards = [];
  cards.push(ui.card('Journey Basis',
    ui.el('p', { class: 'text-sm bold', text: res.current_location_info || 'No current position reported.' }),
    ui.el('div', { class: 'row mt-8' },
      ui.el('span', { class: 'bold', text: res.train_name }),
      ui.badge(res.train_number || '', 'blue'),
      ui.badge('Source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  ));

  const js = res.journey_station;
  if (js) {
    cards.push(ui.card('Journey Station',
      ui.el('div', { class: 'row mt-8' },
        ui.el('span', { class: 'bold', text: js.name }),
        ui.badge(js.code || '', 'blue'),
        js.day_change ? ui.badge('Day Change', 'amber') : null,
      ),
      ui.el('p', { class: 'text-sm muted mt-8', text: `Seq ${js.seq ?? '-'} · Arrival days: ${js.arrival_days || '-'} · Departure days: ${js.departure_days || '-'}` }),
    ));
  }

  cards.push(stationsCard(res, ui));
  return cards;
}

function stationsCard(res, ui) {
  const stations = Array.isArray(res.stations) ? res.stations : [];
  if (!stations.length) {
    return ui.card('Stations', ui.notice('No station data returned.'));
  }
  const showActual = stations.some((s) => s.actual_arrival);
  const headers = ['Station', 'Code', 'Sch. Arrival'];
  if (showActual) headers.push('Act. Arrival');
  headers.push('Delay', 'Status');
  const rows = stations.map((s) => {
    const cells = [s.name, s.code, ui.fmtTime(s.scheduled_arrival)];
    if (showActual) cells.push(ui.fmtTime(s.actual_arrival));
    cells.push(ui.delay(s.delay_minutes), ui.statusCell(s.status));
    return cells;
  });
  return ui.card('Stations', ui.table(headers, rows));
}
})();
