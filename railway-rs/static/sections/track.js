/* sections/track.js - Train section. Landing page (PNR check, recent,
   status) when no train number; entity hero + pill tabs for
   spot/schedule/delay/journey/exceptions/map when a train is entered.
   Deep links like #/train/12559/schedule auto-submit. Live-data-only. */

(() => {
window.Sections = window.Sections || {};

const VIEWS = ['spot', 'schedule', 'delay', 'journey', 'exceptions', 'map'];
const VIEW_LABELS = { spot: 'Spot', schedule: 'Schedule', delay: 'Delay', journey: 'Journey', exceptions: 'Exceptions', map: 'Map' };

window.Sections.train = {
  mount(container, ctx, route) {
    const ui = ctx.ui;
    const p = route.params || {};
    const view = route.view || 'spot';

    /* --- landing page when no train number is in the URL --- */
    if (!p.train) {
      renderLanding(container, ctx, p._pnr || null);
      return;
    }

    const num = String(p.train);

    /* --- entity hero (title from the URL so deep links render instantly) --- */
    let refreshFn = null;
    const hero = {
      el: null,
      subtitle: '',
      facts: [],
      update(subtitle, facts) {
        this.subtitle = subtitle || '';
        this.facts = facts || [];
        this.render();
      },
      render() {
        const built = buildHero(ctx, num, view, this.subtitle, this.facts, () => this.render(), () => refreshFn);
        if (this.el) {
          this.el.replaceChildren(...built.children);
        } else {
          this.el = built;
        }
        return this.el;
      },
    };
    const heroEl = hero.render();

    /* --- shared train input --- */
    const { wrap, input } = ui.trainInput('Train number');
    const results = ui.el('div', { class: 'mt-8' });

    function submitTrain() {
      const raw = input.value.trim();
      RailLog.action('track_train', 'submit', { train_raw: raw });
      const train = raw.replace(/\s*-\s*.+$/, '').replace(/[^\d]/g, '');
      if (!/^\d+$/.test(train)) {
        RailLog.action('track_train', 'validation', { error: 'invalid train number', train_raw: raw });
        ui.render(results, ui.errorBox('Enter a valid train number (digits only).'));
        return;
      }
      RailLog.action('track_train', 'validated', { train });
      /* update URL (preserves current view + carries train across pill switches) */
      const href = Routes.href({ section: 'train', view: view, params: { train: train } });
      if (location.hash !== href) {
        location.hash = href;
        return; /* onHashChange re-mounts with the train in params */
      }
      /* already at the right hash — render the view inline */
      renderView(view, train);
    }

    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitTrain(); });

    /* --- pill bar --- */
    const pills = ui.pillBar(VIEWS, VIEW_LABELS, view, (v) => {
      const t = input.value.trim().replace(/\s*-\s*.+$/, '').replace(/[^\d]/g, '');
      const params = /^\d+$/.test(t) ? { train: t } : p;
      ctx.navigate(Routes.href({ section: 'train', view: v, params: params }));
    });

    /* --- compact form: input + button inline --- */
    const searchBtn = ui.el('button', { class: 'btn', text: 'Search', onclick: submitTrain });
    const form = ui.el('div', { class: 'card-sm' },
      ui.el('div', { class: 'row', style: 'gap:6px;' }, wrap, searchBtn),
    );

    ui.render(container, heroEl, pills, form, results);

    /* --- auto-submit if train is in the URL --- */
    fillInput(p.train, input);

    /* --- view dispatcher --- */
    function renderView(v, train) {
      switch (v) {
        case 'schedule': refreshFn = viewSchedule(results, ctx, train, hero); break;
        case 'delay': refreshFn = viewDelay(results, ctx, train, hero); break;
        case 'journey': refreshFn = viewJourney(results, ctx, train, hero); break;
        case 'exceptions': refreshFn = viewExceptions(results, ctx, train, hero); break;
        case 'map': refreshFn = viewMap(results, ctx, train, hero); break;
        default: refreshFn = viewSpot(results, ctx, train, hero); break;
      }
    }
    renderView(view, p.train);
  },
};

function fillInput(value, input) {
  if (value && input) input.value = String(value).trim();
}

/* ======================================================================
   ENTITY HERO
   ====================================================================== */

function buildHero(ctx, num, view, subtitle, facts, rerender, getRefresh) {
  const ui = ctx.ui;
  const on = ctx.fav.has('train', num);
  const badges = [];
  if (view === 'spot') badges.push(ui.liveDot('Live'));
  const favBtn = ui.iconBtn(on ? 'star-fill' : 'star', on ? 'Remove from favorites' : 'Add to favorites', () => {
    ctx.fav.toggle('train', num, 'Train ' + num);
    rerender();
    ui.toast(on ? 'Removed from favorites' : 'Added to favorites', 'success');
  }, 'fav');
  return ui.entityHero({
    icon: 'train',
    title: 'Train ' + num,
    subtitle: subtitle,
    badges: badges,
    facts: facts,
    actions: [
      favBtn,
      ui.iconBtn('copy', 'Copy link', () => ctx.copyLink()),
      ui.iconBtn('share', 'Share', () => ctx.share()),
      ui.iconBtn('refresh', 'Refresh', () => { const fn = getRefresh(); if (fn) fn(); }),
    ],
  });
}

/* ======================================================================
   SPOT
   ====================================================================== */

function viewSpot(container, ctx, train, hero) {
  const ui = ctx.ui;
  const results = ui.el('div', { class: 'mt-12' });
  let currentIdx = 0;
  let lastRes = null;
  let autoTimer = null;

  const rr = ui.refreshRow({
    updatedAt: null,
    onRefresh: fetchSpot,
    autoKey: 'train.spot.' + train,
    autoMs: 30000,
    onAuto: (on) => {
      if (autoTimer) { clearInterval(autoTimer); autoTimer = null; }
      if (on) autoTimer = setInterval(() => fetchSpot(), 30000);
    },
  });

  container.append(rr.row);
  container.append(results);

  function fetchSpot() {
    return ui.fetchFlow(results, () => ctx.api.liveStatus(train), { failText: 'Failed to load live status' })
      .then((res) => {
        if (!res) return null;
        lastRes = res;
        showInstance(res, currentIdx);
        rr.setUpdated(new Date().toISOString());
        hero.update(spotSubtitle(res, currentIdx), spotFacts(res, currentIdx, ui));
        enrichSpotLabel(ctx, train, res, currentIdx);
        return res;
      });
  }

  function showInstance(res, idx) {
    lastRes = res;
    currentIdx = idx;
    const instances = res.instances || [];
    const inst = instances[idx] || {};
    const hasStops = Array.isArray(inst.stops) && inst.stops.length;
    const parts = [];
    try {
      const segBar = renderInstanceSeg(res, idx, ui, switchInstance);
      if (hasStops) {
        parts.push(renderInstancePosition(res, inst, ui, segBar), renderInstanceStations(inst, ui, ctx));
      } else {
        parts.push(renderPosition(res, ui, segBar), renderStations(res, ui, ctx));
      }
    } catch (e) {
      parts.push(ui.errorBox('Render error: ' + (e.message || String(e))));
    }
    ui.render(results, ...parts);
  }

  function switchInstance(idx) {
    if (lastRes) showInstance(lastRes, idx);
  }

  fetchSpot();
  return fetchSpot;
}

function spotSubtitle(res, idx) {
  const instances = res.instances || [];
  const inst = instances[idx] || {};
  return res.train_name || inst.position || '';
}

function spotFacts(res, idx, ui) {
  const instances = res.instances || [];
  const inst = instances[idx] || {};
  const stops = inst.stops || res.stations || [];
  const route = stops.length >= 2
    ? stops[0].name + ' \u2192 ' + stops[stops.length - 1].name
    : (res.current_location_info || '—');
  return [
    ['Route', route],
    ['Run date', ui.friendlyDate(inst.start_date || res.train_start_date)],
    ['Delay', currentDelayText(stops)],
  ];
}

/* Index of the stop the train is at or heading to, from the per-stop
   statuses the backend stamps (departed/expected/scheduled). */
function spotCurrentIndex(stops) {
  if (!Array.isArray(stops) || !stops.length) return -1;
  const expected = stops.findIndex((s) => s && s.status === 'expected');
  if (expected >= 0) return expected;
  let last = -1;
  stops.forEach((s, i) => { if (s && s.status === 'departed') last = i; });
  return last >= 0 ? last : 0;
}

function currentDelayText(stops) {
  if (!Array.isArray(stops) || !stops.length) return '—';
  const idx = spotCurrentIndex(stops);
  const s = stops[Math.min(idx, stops.length - 1)];
  if (s && typeof s.delay_minutes === 'number' && s.delay_minutes > 0) return s.delay_minutes + ' min';
  return 'On Time';
}

function renderInstanceSeg(res, activeIdx, ui, onSwitch) {
  const instances = res.instances || [];
  if (instances.length < 2) return null;
  const options = instances.map((inst, i) => [
    i,
    inst.start_date ? ui.friendlyDate(inst.start_date) : 'Run ' + (i + 1),
  ]);
  return ui.seg(options, activeIdx, onSwitch);
}

function renderInstancePosition(res, inst, ui, segBar) {
  const stops = inst.stops || [];
  let location;
  if (inst.at_dstn === 'true') location = 'Arrived at ' + ((stops[stops.length - 1] || {}).name || 'destination') + '.';
  else if (inst.at_src === 'true') location = 'Train at ' + ((stops[0] || {}).name || 'origin') + ' (origin).';
  else location = inst.position || 'Running; position awaiting update.';
  return ui.card('Current Position',
    ...[segBar,
      ui.el('div', { class: 'row' },
        ui.el('span', { class: 'bold', text: res.train_name }),
        ui.badge(res.train_number, 'blue'),
      ),
      ui.journeyProgress(stops, spotCurrentIndex(stops)),
      ui.el('p', { class: 'text-sm bold', text: location }),
    ].filter(Boolean),
  );
}

function renderInstanceStations(inst, ui, ctx) {
  const stations = inst.stops || [];
  if (!stations.length) return ui.card('Stations', ui.notice('No station data for this run.'));
  const showActual = stations.some((s) => s.actual_arrival);
  const headers = ['Station', 'Code', 'Sch. Arrival'];
  if (showActual) headers.push('Act. Arrival');
  headers.push('Delay', 'Status');
  const rows = stations.map((s) => {
    const cells = [s.name, ui.entityLink('station', s.code, s.code, ctx.navigate), ui.fmtTime(s.scheduled_arrival)];
    if (showActual) cells.push(ui.fmtTime(s.actual_arrival));
    cells.push(ui.delay(s.delay_minutes), ui.statusCell(s.status));
    return cells;
  });
  return ui.card('Stations', ui.collapsibleTable(headers, rows));
}

function renderPosition(res, ui, segBar) {
  const stops = res.stations || [];
  return ui.card('Current Position',
    ...[segBar,
      ui.el('div', { class: 'row' },
        ui.el('span', { class: 'bold', text: res.train_name }),
        ui.badge(res.train_number, 'blue'),
      ),
      ui.journeyProgress(stops, spotCurrentIndex(stops)),
      ui.el('p', { class: 'text-sm bold', text: res.current_location_info || 'No current position reported.' }),
    ].filter(Boolean),
  );
}

function renderStations(res, ui, ctx) {
  const stations = res.stations || [];
  if (!stations.length) return ui.card('Stations', ui.notice('No station data returned.'));
  const showActual = stations.some((s) => s.actual_arrival);
  const headers = ['Station', 'Code', 'Sch. Arrival'];
  if (showActual) headers.push('Act. Arrival');
  headers.push('Delay', 'Status');
  const rows = stations.map((s) => {
    const cells = [s.name, ui.entityLink('station', s.code, s.code, ctx.navigate), ui.fmtTime(s.scheduled_arrival)];
    if (showActual) cells.push(ui.fmtTime(s.actual_arrival));
    cells.push(ui.delay(s.delay_minutes), ui.statusCell(s.status));
    return cells;
  });
  return ui.card('Stations', ui.collapsibleTable(headers, rows));
}

/* ======================================================================
   SCHEDULE
   ====================================================================== */

function viewSchedule(container, ctx, train, hero) {
  const ui = ctx.ui;
  const results = ui.el('div', { class: 'mt-12' });
  container.append(results);
  const fetchSchedule = () => ui.fetchFlow(results, () => ctx.api.schedule(train), { failText: 'Failed to load schedule' })
    .then((res) => {
      if (!res) return null;
      const stops = Array.isArray(res.stops) ? res.stops : [];
      const from = stops[0];
      const to = stops[stops.length - 1];
      const route = res.route_description
        || (from && to ? from.name + ' (' + from.code + ') \u2192 ' + to.name + ' (' + to.code + ')' : '—');
      hero.update(res.train_name || '', [
        ['Route', route],
        ['Runs on', (res.running_days || []).map((d) => String(d).slice(0, 3).toUpperCase()).join(' ') || '—'],
      ]);
      enrichTrainLabel(ctx, train, res.train_number, res.train_name, from, to);
      ui.render(results, ...renderSchedule(res, ui, ctx));
      return res;
    });
  fetchSchedule();
  return fetchSchedule;
}

/* Enrich the stored recent/favorite label for a train with
   "number · name (from → to)" once we have real schedule data. */
function enrichTrainLabel(ctx, train, trainNumber, trainName, from, to) {
  const label = 'Train ' + (trainNumber || train)
    + (trainName ? ' · ' + trainName : '')
    + (from && to ? ' (' + from.code + ' \u2192 ' + to.code + ')' : '');
  ctx.recent.update(Routes.href({ section: 'train', params: { train } }), label);
  ctx.fav.update('train', train, label);
}

function enrichSpotLabel(ctx, train, res, idx) {
  const inst = (res.instances || [])[idx] || {};
  const stops = inst.stops || res.stations || [];
  enrichTrainLabel(ctx, train, res.train_number, res.train_name, stops[0], stops[stops.length - 1]);
}

function renderSchedule(s, ui, ctx) {
  const today = new Date().toLocaleString('en-GB', { timeZone: 'Asia/Kolkata', weekday: 'short' }).toUpperCase().slice(0, 3);
  const days = s.running_days || [];
  const stops = s.stops;
  return [ui.card('Schedule',
    ui.el('div', { class: 'row align-center', style: 'gap:4px;flex-wrap:wrap;' },
      days.length
        ? days.map((d) => ui.badge(String(d).slice(0, 3).toUpperCase(), d.toUpperCase() === today ? 'green' : 'slate'))
        : [ui.notice('Not available.')],
    ),
    Array.isArray(stops) && stops.length
      ? ui.collapsibleTable(['Day', 'Code', 'Station', 'Arrival', 'Departure'],
          stops.map((st) => [
            ui.el('span', { text: st.day || '' }).outerHTML,
            ui.entityLink('station', st.code || '', st.code || '', ctx.navigate),
            st.name || '',
            ui.fmtTime(st.arrival),
            ui.fmtTime(st.departure),
          ]))
      : ui.notice('No stops returned.'),
  )];
}

/* ======================================================================
   DELAY
   ====================================================================== */

function viewDelay(container, ctx, train, hero) {
  const ui = ctx.ui;
  const results = ui.el('div', { class: 'mt-12' });
  const rr = ui.refreshRow({ updatedAt: null, onRefresh: fetchDelay });
  container.append(rr.row);
  container.append(results);

  function fetchDelay() {
    return ui.fetchFlow(results, () => ctx.api.averageDelay(train), { failText: 'Failed to load average delay' })
      .then((res) => {
        if (!res) return null;
        rr.setUpdated(new Date().toISOString());
        hero.update(res.train_name || '', [
          ['Days of run', res.days_of_run || '—'],
          ['Type', res.train_type || '—'],
        ]);
        ui.render(results, ...renderDelay(res, ui));
        return res;
      });
  }

  fetchDelay();
  return fetchDelay;
}

function renderDelay(res, ui) {
  const stations = res.stations || [];
  const list = ui.card('Stations',
    Array.isArray(stations) && stations.length
      ? ui.collapsibleTable(['Sr.', 'Station', 'Code', 'Arr. Delay', 'Dep. Delay'],
          stations.map((st) => [st.sr || '', st.name || '', st.code || '', delayBadge(st.arrival_delay), delayBadge(st.departure_delay)]))
      : ui.notice('No delay data found.'),
  );
  return [list];
}

/* Delay cell (HTML string): green "On Time" when on time or 0, amber when
   the train is running late. */
function delayBadge(v) {
  if (v === null || v === undefined || v === '') return '<span class="muted">\u2014</span>';
  const t = String(v);
  if (/^[-/]+$/.test(t) || /^[\u2013\u2014]+$/.test(t)) return '<span class="muted">\u2014</span>';
  if (/on time/i.test(t)) return '<span class="badge badge-green">On Time</span>';
  const m = /\d+/.exec(t);
  const n = m ? parseInt(m[0], 10) : 0;
  if (n <= 0) return '<span class="badge badge-green">On Time</span>';
  return '<span class="badge badge-amber">' + mapEsc(t) + '</span>';
}

/* ======================================================================
   JOURNEY
   ====================================================================== */

function viewJourney(container, ctx, train, hero) {
  const ui = ctx.ui;
  const results = ui.el('div', { class: 'mt-12' });
  container.append(results);

  const showBasis = (stationCode) => {
    ui.fetchFlow(results, () => ctx.api.journeyBasis(train, stationCode), { failText: 'Failed to load journey basis' })
      .then((res) => {
        if (!res) return;
        hero.update(res.train_name || '', []);
        ui.render(results, ...renderBasis(res, ui));
      });
  };

  const fetchStations = () => ui.fetchFlow(results, () => ctx.api.journeyStations(train), { failText: 'Failed to load journey stations' })
    .then((res) => {
      if (!res) return null;
      ui.render(results, ...renderStationPicker(res, ui, showBasis));
      return res;
    });

  fetchStations();
  return fetchStations;
}

function renderStationPicker(res, ui, onPick) {
  const stations = Array.isArray(res.stations) ? res.stations : [];
  if (!stations.length) return [ui.card('Journey Stations', ui.notice('No journey stations returned for this train.'))];
  const select = ui.el('select', { class: 'input', 'aria-label': 'Journey Station' },
    stations.map((s) => ui.el('option', { value: s.code, text: s.code + ' - ' + s.name })),
  );
  select.style.flex = '1';
  const go = ui.el('button', { class: 'btn', text: 'Show' });
  go.addEventListener('click', () => onPick(select.value));
  return [ui.card('Journey Stations',
    ui.el('div', { class: 'row', style: 'gap:6px;' }, select, go),
  )];
}

function renderBasis(res, ui) {
  const js = res.journey_station;
  const cards = [];
  cards.push(ui.card('Journey Basis',
    ...[
      ui.el('p', { class: 'text-sm bold', text: res.current_location_info || 'No current position reported.' }),
      js ? ui.el('div', { class: 'mt-8' },
        ui.el('div', { class: 'row' },
          ui.el('span', { class: 'bold', text: js.name }),
          ui.badge(js.code || '', 'blue'),
          js.day_change ? ui.badge('Day Change', 'amber') : null,
        ),
        ui.el('p', { class: 'text-sm muted mt-8', text: 'Seq ' + (js.seq ?? '-') + ' \u00b7 Arrival days: ' + (js.arrival_days || '-') + ' \u00b7 Departure days: ' + (js.departure_days || '-') }),
      ) : null,
    ].filter(Boolean),
  ));
  cards.push(stationsCard(res, ui));
  return cards;
}

function stationsCard(res, ui) {
  const stations = Array.isArray(res.stations) ? res.stations : [];
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
  return ui.card('Stations', ui.collapsibleTable(headers, rows));
}

/* ======================================================================
   EXCEPTIONS
   ====================================================================== */

function viewExceptions(container, ctx, train, hero) {
  const ui = ctx.ui;
  const results = ui.el('div', { class: 'mt-12' });
  container.append(results);
  const fetchExceptions = () => ui.fetchFlow(results, () => ctx.api.exceptional(train), { failText: 'Failed to load exceptional dates' })
    .then((res) => {
      if (!res) return null;
      const t = res.train || {};
      hero.update(t.name || '', [
        ['Route', (t.source && t.destination) ? t.source + ' \u2192 ' + t.destination : '—'],
        ['Runs on', (Array.isArray(t.days_of_run) && t.days_of_run.length) ? t.days_of_run.join(' ') : '—'],
      ]);
      renderExceptions(res, ui, results);
      return res;
    });
  fetchExceptions();
  return fetchExceptions;
}

function renderExceptions(res, ui, container) {
  const t = res.train || {};
  const exceptions = Array.isArray(res.exceptions) ? res.exceptions : [];
  let listCard;
  if (!exceptions.length) {
    listCard = ui.card('Exceptional Dates',
      ui.notice(res.message || ('No exceptional details found for train ' + (t.number || '') + '.')));
  } else {
    listCard = ui.card('Exceptional Dates',
      ui.collapsibleTable(['Date', 'Kind', 'Note'], exceptions.map((e) => [
        '<span title="' + mapEsc(e.date || '') + '">' + ui.friendlyDate(e.date) + '</span>',
        kindBadge(e.kind),
        e.note || '-',
      ])),
    );
  }
  ui.render(container, listCard);
}

function kindBadge(kind) {
  const color = kind === 'cancelled' ? 'red' : kind === 'rescheduled' ? 'amber' : kind === 'diverted' ? 'amber' : kind === 'new_source' ? 'green' : kind === 'new_destination' ? 'blue' : 'slate';
  return '<span class="badge badge-' + color + '">' + (kind || '-') + '</span>';
}

/* ======================================================================
   MAP (inline from former train_on_map tab)
   ====================================================================== */

function viewMap(container, ctx, train, hero) {
  const ui = ctx.ui;

  /* station input is specific to map view */
  const stationInput = ui.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Station code (optional, e.g. NDLS)', 'aria-label': 'Station code (optional)' });
  stationInput.style.flex = '1';
  const submit = ui.el('button', { class: 'btn', text: 'Show Map' });
  const mapForm = ui.el('div', { class: 'card-sm' },
    ui.el('div', { class: 'row', style: 'gap:6px;' }, stationInput, submit),
  );
  const results = ui.el('div', { class: 'mt-12' });

  const submitMap = () => {
    let station = null;
    if (stationInput.value.trim()) {
      const check = ui.stationCode(stationInput.value);
      if (check.error) { ui.render(results, ui.errorBox(check.error)); return; }
      station = check.code;
    }
    ui.render(results, ui.spinner());
    submit.disabled = true;
    ctx.api.trainOnMap(train, station)
      .then((res) => {
        if (!res || res.ok === false) { ui.render(results, ui.errorBox(res && res.error ? res.error : 'Failed to load train map.')); return; }
        hero.update(res.train_name || '', [
          ['Route', (res.source || '') + ' \u2192 ' + (res.destination || '')],
        ]);
        ui.render(results, ...renderMapResults(res, ui));
      })
      .catch((err) => ui.render(results, ui.errorBox('Failed to load train map: ' + (err.message || String(err)))))
      .finally(() => { submit.disabled = false; });
  };

  stationInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitMap(); });
  submit.addEventListener('click', submitMap);
  ui.render(container, mapForm, results);
  submitMap();
  return submitMap;
}

function renderMapResults(res, ui) {
  const parts = [];
  if (res.current_station) parts.push(renderMapLive(res, ui));
  parts.push(renderMapLeaflet(res, ui));
  parts.push(renderMapStations(res, ui));
  return parts;
}

function renderMapLive(res, ui) {
  const current = res.current_station || {};
  const j = res.journey_station;
  return ui.card('Live Position',
    ...[
      ui.el('div', { class: 'row' },
        ui.badge('\u25cf CURRENT: ' + (current.code || '?'), 'blue'),
        j ? ui.el('span', { class: 'bold', text: (j.name || '') + ' (' + (j.code || '') + ')' }) : null,
        j && j.day_change ? ui.badge('Day Change', 'amber') : null,
      ),
      j ? ui.el('div', { class: 'mt-8' },
        [['Expected arrival', j.expected_arrival], ['Actual arrival', j.actual_arrival],
         ['Delay', j.delay_status], ['Platform', j.platform],
        ].filter((pair) => pair[1]).map(([k, v]) => ui.el('div', { class: 'row mt-4' },
          ui.el('span', { class: 'text-sm muted', text: k + ': ' }),
          ui.el('span', { class: 'text-sm bold', text: v }),
        )),
      ) : null,
    ].filter(Boolean),
  );
}

function renderMapLeaflet(res, ui) {
  var track = (res.track || []).filter(function(s) { return typeof s.lat === 'number' && typeof s.lng === 'number'; });
  var route = (res.route || []).filter(function(s) { return typeof s.lat === 'number' && typeof s.lng === 'number'; });
  var coords = track.length >= 2 ? track : route;
  if (coords.length < 2) return ui.card('Route Map', ui.notice('No coordinate data for this route.'));
  var mapDiv = ui.el('div', { class: 'route-map' });
  var card = ui.card('Route Map', mapDiv);
  setTimeout(function() {
    if (typeof L === 'undefined') return;
    var map = L.map(mapDiv, { zoomControl: true, attributionControl: true });
    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
      attribution: '&copy; OpenStreetMap contributors',
      maxZoom: 18,
    }).addTo(map);
    var allCoords = track.length >= 2 ? track : route;
    var latlngs = allCoords.map(function(s) { return [s.lat, s.lng]; });
    L.polyline(latlngs, { color: '#2563eb', weight: 3, opacity: 0.85, lineJoin: 'round', lineCap: 'round' }).addTo(map);
    var currentCode = res.current_station && res.current_station.code;
    route.forEach(function(s) {
      var isCurrent = currentCode && s.code === currentCode;
      var marker;
      if (isCurrent) {
        marker = L.circleMarker([s.lat, s.lng], {
          radius: 7, fillColor: '#dc2626', color: '#fff', weight: 2, fillOpacity: 0.9,
        }).addTo(map);
      } else {
        marker = L.circleMarker([s.lat, s.lng], {
          radius: 4, fillColor: '#2563eb', color: '#fff', weight: 1.5, fillOpacity: 0.85,
        }).addTo(map);
      }
      marker.bindTooltip(s.code + (s.name ? ' \u2014 ' + s.name : ''), { permanent: false, direction: 'top', offset: [0, -6] });
    });
    if (res.current_station && typeof res.current_station.lat === 'number') {
      var cs = res.current_station;
      L.circleMarker([cs.lat, cs.lng], {
        radius: 5, fillColor: '#f97316', color: '#fff', weight: 2, fillOpacity: 0.9,
      }).addTo(map).bindTooltip('Current: ' + (cs.code || ''), { permanent: true, direction: 'top', offset: [0, -6], className: 'map-tooltip-current' });
    }
    map.fitBounds(allCoords.map(function(s) { return [s.lat, s.lng]; }), { padding: [30, 30] });
  }, 0);
  return card;
}

function renderMapStations(res, ui) {
  const route = res.route || [];
  if (!route.length) return ui.card('Stations', ui.notice('No station data returned.'));
  const currentCode = res.current_station && res.current_station.code;
  const rows = route.map((st, i) => {
    const marker = st.code === currentCode ? '<span style="color:#dc2626;font-weight:bold">\u25cf</span> ' : '';
    return [String(i + 1), marker + mapEsc(st.name || ''), mapEsc(st.code || ''),
      st.arrival || '\u2014', st.departure || '\u2014', st.day || '', st.distance || '',
      st.expected_arrival || '', st.expected_departure || '',
      mapDelayCell(st.arrival_delay, st.departure_delay),
      mapStatusBadge(st.arrival_delay, st.departure_delay),
    ];
  });
  return ui.card('Stations', ui.collapsibleTable(['#', 'Station', 'Code', 'Arr', 'Dep', 'Day', 'Dist', 'Exp. Arr', 'Exp. Dep', 'Delay', 'Status'], rows));
}

function mapDelayCell(arr, dep) {
  const kind = (d) => (d === 'On Time' ? 'green' : 'amber');
  const p = [];
  if (arr) p.push('Arr <span class="badge badge-' + kind(arr) + '">' + mapEsc(arr) + '</span>');
  if (dep) p.push('Dep <span class="badge badge-' + kind(dep) + '">' + mapEsc(dep) + '</span>');
  return p.join(' \u00b7 ') || '<span class="muted">\u2014</span>';
}

function mapStatusBadge(arr, dep) {
  const p = [arr, dep].filter(Boolean);
  if (!p.length) return '<span class="muted">\u2014</span>';
  const onTime = p.every((x) => x === 'On Time');
  return '<span class="badge badge-' + (onTime ? 'green' : 'amber') + '">' + (onTime ? 'On Time' : 'Delayed') + '</span>';
}

function mapEsc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

/* ======================================================================
   LANDING PAGE (PNR check, recent lookups, system status)
   ====================================================================== */

function renderLanding(container, ctx, prefillPnr) {
  const ui = ctx.ui;
  const navigate = ctx.navigate;

  const landing = ui.el('div');

  if (prefillPnr) {
    /* PNR deep-link: show only PNR card, auto-submit */
    const pnrCard = buildPNRCard(ui, ctx);
    ui.render(landing, pnrCard);
    container.append(landing);
    const pnrInput = landing.querySelector('.pnr-input');
    if (pnrInput) {
      pnrInput.value = prefillPnr;
      pnrInput.dispatchEvent(new Event('input'));
      const pnrBtn = landing.querySelector('.pnr-submit');
      if (pnrBtn) pnrBtn.click();
    }
  } else {
    /* Normal landing: train search + PNR + recent chips + status */
    const trainCard = buildTrainSearchCard(ui, navigate);
    const pnrCard = buildPNRCard(ui, ctx);
    const recentWrap = ui.el('div');
    renderRecent(recentWrap, ctx);
    const statusWrap = ui.el('div');
    renderStatus(statusWrap, ctx);
    ui.render(landing, ui.el('div', { class: 'grid grid-2' }, trainCard, pnrCard, recentWrap, statusWrap));
    container.append(landing);
    const trainInput = landing.querySelector('.autocomplete .input');
    if (trainInput) trainInput.focus();
  }
}

/* ---------- Train Search ---------- */

function buildTrainSearchCard(ui, navigate) {
  const card = ui.card('Track a Train');
  const { wrap, input } = ui.trainInput('Train number (e.g. 12559)');
  const btn = ui.el('button', { class: 'btn', text: 'Search' });

  function submit() {
    const raw = input.value.trim();
    RailLog.action('track_train', 'submit', { train_raw: raw });
    const train = raw.replace(/\s*-\s*.+$/, '').replace(/[^\d]/g, '');
    if (!/^\d+$/.test(train)) {
      RailLog.action('track_train', 'validation', { error: 'invalid train number', train_raw: raw });
      input.focus();
      return;
    }
    RailLog.action('track_train', 'validated', { train });
    navigate(Routes.href({ section: 'train', params: { train: train } }));
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
  btn.addEventListener('click', submit);

  card.append(
    ui.el('div', { class: 'row', style: 'gap:6px;' }, wrap, btn),
  );
  return card;
}

/* ---------- PNR Check ---------- */

function buildPNRCard(ui, ctx) {
  const card = ui.card('PNR Check');
  const input = ui.el('input', {
    class: 'input pnr-input',
    autocomplete: 'off',
    maxlength: '10',
    inputmode: 'numeric',
    placeholder: '10-digit PNR number',
  });
  const btn = ui.el('button', { class: 'btn pnr-submit', text: 'Check' });
  const results = ui.el('div');

  function submit() {
    const pnr = input.value.trim();
    if (!/^\d{10}$/.test(pnr)) {
      ui.render(results, ui.errorBox('PNR must be exactly 10 digits.'));
      return;
    }
    const setLoading = ui.withLoading(btn, 'Checking...');
    setLoading(true);
    ui.render(results, ui.spinner());
    fetchPNR(pnr, null, 3, ctx, setLoading)
      .then((resolved) => {
        if (!resolved) return;
        if (resolved.kind === 'ok') {
          ui.render(results, ...renderPNRResult(resolved.data, ctx));
        } else if (resolved.kind === 'error') {
          ui.render(results, ui.errorBox(resolved.text));
        } else if (resolved.kind === 'notice') {
          ui.render(results, ui.notice(resolved.text));
        }
      })
      .catch((err) => {
        ui.render(results, ui.errorBox('Request failed: ' + (err && err.message ? err.message : String(err))));
      })
      .finally(() => setLoading(false));
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });
  btn.addEventListener('click', submit);

  card.append(
    ui.el('div', { class: 'row', style: 'gap:6px;' }, input, btn),
    results,
  );
  return card;
}

async function fetchPNR(pnr, captcha, attemptsLeft, ctx, setLoading) {
  const res = await ctx.api.pnr(pnr, captcha);
  if (!res || res.ok !== false) {
    return res ? { kind: 'ok', data: res } : { kind: 'error', text: 'No response from server.' };
  }
  if (res.status === 428 && attemptsLeft > 1) {
    const params = await ctx.captcha.show(res.body);
    if (!params) return { kind: 'notice', text: 'Check cancelled.' };
    return fetchPNR(pnr, {
      session_id: params.session_id,
      source: params.source,
      text: params.text,
    }, attemptsLeft - 1, ctx, setLoading);
  }
  return { kind: 'error', text: res.error || 'Request failed.' };
}

function renderPNRResult(data, ctx) {
  const ui = ctx.ui;
  const passengers = (Array.isArray(data.passengers) && data.passengers.length)
    ? ui.table(
        ['Booking Status', 'Current Status', 'Coach', 'Berth'],
        data.passengers.map((p) => [
          ui.esc(p && p.booking_status),
          ui.esc(p && p.current_status),
          ui.esc(p && p.coach),
          ui.esc(p && p.berth),
        ]),
      )
    : ui.notice('No passenger data returned.');

  const card = ui.card('PNR Status',
    ...[
      ui.el('div', { class: 'row' },
        ui.el('span', { class: 'bold', text: data.train_name || 'Unknown train' }),
        data.train_number != null ? ui.entityLink('train', String(data.train_number), String(data.train_number), ctx.navigate) : null,
      ),
      data.journey_date
        ? ui.el('div', { class: 'text-sm muted mt-8', text: 'Journey: ' + ui.friendlyDate(data.journey_date) })
        : null,
      ui.el('div', { class: 'row mt-8' },
        stationCell(data.from, ui, ctx),
        ui.el('span', { class: 'muted', text: '\u2192' }),
        stationCell(data.to, ui, ctx),
      ),
      ui.el('div', { class: 'mt-8' }, passengers),
    ].filter(Boolean),
  );

  const actions = ui.contextualActions(
    data.train_number ? { type: 'train', code: String(data.train_number) } : null,
    ctx.navigate,
  );

  return [card, actions];
}

function stationCell(s, ui, ctx) {
  if (!s || !s.name) return ui.el('span', { class: 'muted', text: '\u2014' });
  const sub = [s.code, ui.fmtTime(s.time), s.day ? 'Day ' + s.day : ''].filter(Boolean).join(' \u00b7 ');
  const codeLink = s.code ? ui.entityLink('station', s.code, s.code, ctx.navigate) : null;
  return ui.el('div', { class: 'col' },
    ui.el('span', { class: 'bold', text: s.name }),
    ui.el('span', { class: 'text-sm muted', text: sub }),
  );
}

/* ---------- Recent ---------- */

function renderRecent(wrap, ctx) {
  const ui = ctx.ui;
  const card = ui.card('Recent lookups');
  const list = ctx.recent.list();
  if (!list.length) {
    card.append(ui.notice('No recent lookups.'));
  } else {
    const row = ui.el('div', { class: 'chip-row' });
    list.forEach((r) => {
      const entityType = r.hash.includes('/train/') ? 'train'
        : r.hash.includes('/station/') ? 'station'
        : r.hash.includes('/plan/') ? 'plan'
        : r.hash.includes('/pnr/') ? 'pnr' : '';
      const icons = { train: 'train', station: 'station', plan: 'map', pnr: 'ticket' };
      row.append(ui.el('button', {
        class: 'chip',
        onclick: () => ctx.navigate(r.hash),
        title: r.hash,
        'aria-label': 'Open recent lookup ' + r.label,
      },
        icons[entityType] ? ui.icon(icons[entityType]) : null,
        ui.el('span', { class: 'chip-code', text: r.label }),
      ));
    });
    card.append(row);
    card.append(ui.el('div', { class: 'row mt-8' },
      ui.el('button', { class: 'btn ghost btn-sm', text: 'Clear', onclick: () => { ctx.recent.clear(); renderRecent(wrap, ctx); } }),
    ));
  }
  ui.render(wrap, card);
}

/* ---------- System Status ---------- */

function renderStatus(wrap, ctx) {
  const ui = ctx.ui;
  const card = ui.card('Status', ui.skeleton(1));
  ui.render(wrap, card);
  ctx.api.sourceStatus().then((s) => {
    if (!s || s.ok === false) {
      ui.render(wrap, ui.card('Status', ui.notice('Unavailable')));
      return;
    }
    const liveBadge = s.live_enabled ? ui.badge('Live', 'green') : ui.badge('Offline', 'red');
    const sources = (s.sources || []).map((src) =>
      ui.badge(src.name + (src.reachable ? ' ↑' : ' ↓'), src.reachable ? 'green' : 'red')
    );
    ui.render(wrap, ui.card('Status',
      ui.el('div', { class: 'row align-center' },
        liveBadge,
        ui.el('span', { class: 'text-xs muted', text: (s.primary_source || '?') }),
      ),
      ui.el('div', { class: 'row mt-8', style: 'gap:4px;flex-wrap:wrap;' }, ...sources),
    ));
  }).catch(() => {
    ui.render(wrap, ui.card('Status', ui.notice('Unavailable')));
  });
}
})();
