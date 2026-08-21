(() => {
window.Sections = window.Sections || {};

window.Sections.station = {
  mount(container, ctx, route) {
    const p = route.params || {};
    const view = route.view || 'live';
    const views = ['live', 'tt', 'heritage', 'parcel'];
    const labels = { live: 'Live', tt: 'Timetable', heritage: 'Heritage', parcel: 'Parcel' };
    const pills = ctx.ui.pillBar(views, labels, view, (v) => {
      ctx.navigate(Routes.href({ section: 'station', view: v, params: p }));
    });
    const content = ctx.ui.el('div');
    ctx.ui.render(container, pills, content);
    switch (view) {
      case 'tt': viewTT(content, ctx, p); break;
      case 'heritage': viewHeritage(content, ctx, p); break;
      case 'parcel': viewParcel(content, ctx, p); break;
      default: viewLive(content, ctx, p); break;
    }
  },
};

function fillInput(value, input) {
  if (value && input) input.value = String(value).trim();
}

function stationHero(code, res, ui, ctx, extra) {
  extra = extra || {};
  const title = 'Station ' + code.toUpperCase();
  const on = ctx.fav.has('station', code);
  const favBtn = ui.iconBtn(on ? 'star-fill' : 'star', (on ? 'Remove from' : 'Add to') + ' favorites', () => {
    const added = ctx.fav.toggle('station', code, 'Station ' + code);
    favBtn.replaceChildren(ui.icon(added ? 'star-fill' : 'star'));
    favBtn.setAttribute('aria-label', (added ? 'Remove from' : 'Add to') + ' favorites');
    ui.toast((added ? 'Added ' : 'Removed ') + title + (added ? ' to' : ' from') + ' favorites', 'success');
  }, 'fav');
  const hash = Routes.href({ section: 'station', view: extra.view || 'live', params: { station: code } });
  return ui.entityHero({
    icon: 'station',
    title,
    subtitle: res.station_name || '',
    badges: extra.badges || [ui.badge(code, 'blue'), ui.badge(res.data_source || 'unknown', 'slate')],
    facts: extra.facts || [['Total trains', Array.isArray(res.trains) ? res.trains.length : 0], ['Data source', res.data_source || 'unknown']],
    actions: [favBtn, ui.iconBtn('copy', 'Copy link', () => ui.copyLink(hash)), ui.iconBtn('share', 'Share', () => ui.share(hash))],
  });
}

/* ---------- live ---------- */

function viewLive(container, ctx, params) {
  const ui = ctx.ui;
  let hours = 2;
  let autoTimer = null;
  let autoOn = false;

  const { wrap, input } = ui.stationInput('e.g. NDLS');
  const hoursSeg = ui.seg([['2', '2 hrs'], ['4', '4 hrs'], ['8', '8 hrs']], String(hours), (v) => { hours = parseInt(v, 10) || 2; });
  const submit = ui.el('button', { class: 'btn', text: 'Get Live' });
  const results = ui.el('div', { class: 'col mt-8' });

  function setAuto(on) {
    autoOn = !!on;
    if (autoTimer) { clearInterval(autoTimer); autoTimer = null; }
    if (autoOn) autoTimer = setInterval(load, 30000);
  }

  function load() {
    let raw = input.value.trim().toUpperCase();
    const hrs = hours;
    const setLoading = ui.withLoading(submit, 'Loading…');
    setLoading(true);

    RailLog.action('live_station', 'submit', { code_raw: raw, hours: hrs });

    if (!raw) {
      setLoading(false);
      RailLog.action('live_station', 'validation', { error: 'empty', code_raw: raw });
      ui.render(results, ui.errorBox('Enter a station code (2-4 characters, e.g. NDLS or AK).'));
      return;
    }
    const check = ui.stationCode(raw);
    if (check.error) {
      setLoading(false);
      RailLog.action('live_station', 'validation', { error: check.error, code_raw: raw });
      ui.render(results, ui.errorBox(check.error));
      return;
    }
    const code = check.code;
    RailLog.action('live_station', 'validated', { code, hours: hrs });

    const targetHash = Routes.href({ section: 'station', view: 'live', params: { station: code } });
    if (location.hash !== targetHash) history.replaceState(null, '', targetHash);

    ui.fetchFlow(results, () => fetchLiveWithRoutes(ctx.api, code, hrs), { failText: 'Failed to load live station' })
      .then((res) => {
        setLoading(false);
        if (!res) return;
        if (pendingHero) pendingHero.remove();
        ui.render(results, ...renderLive(res, code, hrs, ui, ctx, { onRefresh: load, onAuto: setAuto }));
        if (autoOn) {
          const tgl = results.querySelector('.auto-toggle');
          if (tgl) { tgl.setAttribute('aria-pressed', 'true'); tgl.classList.add('on'); }
        }
      });
  }

  submit.onclick = load;
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') load(); });

  const form = ui.el('div', { class: 'card-sm' },
    ui.el('div', { class: 'row', style: 'gap:6px;' }, wrap, hoursSeg, submit),
  );
  ui.render(container, form, results);

  let pendingHero = null;
  if (params.station) {
    fillInput(params.station, input);
    pendingHero = stationHero(params.station, {}, ui, ctx, {
      view: 'live',
      facts: [['Total trains', '—'], ['Window', hours + ' hrs'], ['Data source', '—']],
    });
    container.append(pendingHero);
    load();
  }
}

/* Live station + (best-effort) timetable merge so every row can carry its
   from → to route. The timetable is cached server-side, so this only costs a
   second request on the first load; a failed timetable never blocks the board. */
function fetchLiveWithRoutes(api, code, hrs) {
  return api.liveStation(code, hrs).then((live) => {
    if (!live || live.ok === false || !Array.isArray(live.trains)) return live;
    return api.stationTimetable(code)
      .then((tt) => {
        if (tt && tt.ok !== false && Array.isArray(tt.trains)) {
          const routes = {};
          tt.trains.forEach((t) => { if (t && t.number) routes[t.number] = t.route || ''; });
          live.trains.forEach((t) => { if (t) t.route = routes[t.number] || ''; });
        }
        return live;
      })
      .catch(() => live);
  });
}

function renderLive(res, code, hours, ui, ctx, opts) {
  const trains = Array.isArray(res.trains) ? res.trains : [];
  const hero = stationHero(code, res, ui, ctx, {
    view: 'live',
    badges: [ui.badge(code, 'blue'), ui.badge('LIVE', 'green'), ui.badge(res.data_source || 'unknown', 'slate')],
    facts: [['Total trains', trains.length], ['Window', hours + ' hrs'], ['Data source', res.data_source || 'unknown']],
  });
  const rr = ui.refreshRow({
    updatedAt: new Date().toISOString(),
    onRefresh: opts.onRefresh,
    autoKey: 'station.live.' + code,
    autoMs: 30000,
    onAuto: opts.onAuto,
  });
  const hasArr = trains.some((t) => t.eta);
  const hasDep = trains.some((t) => t.dep);
  const useFilter = hasArr && hasDep;
  const wrap = ui.el('div', { class: 'col' });
  const renderBoard = (filter) => {
    ui.render(wrap,
      useFilter ? ui.seg([['all', 'All'], ['arr', 'Arr'], ['dep', 'Dep']], filter || 'all', renderBoard) : null,
      liveBoard(trains, filter || 'all', ui, ctx));
  };
  renderBoard('all');
  return [hero, rr, wrap];
}

function liveBoard(trains, filter, ui, ctx) {
  const shown = filter === 'arr' ? trains.filter((t) => t.eta) : filter === 'dep' ? trains.filter((t) => t.dep) : trains;
  if (!shown.length) {
    return ui.card('Live Board', ui.notice('No trains in window.'));
  }
  const board = ui.el('div', { class: 'board' });
  shown.forEach((t) => {
    const late = delayMinutes(t);
    const row = ui.el('button', {
      class: 'board-row',
      onclick: () => ctx.navigate(Routes.href({ section: 'train', params: { train: t.number } })),
      'aria-label': 'Open train ' + t.number + (t.name ? ' ' + t.name : ''),
    });
    row.append(
      ui.el('span', { class: 'board-num', text: t.number }),
      ui.el('span', { class: 'board-name' },
        ui.el('span', { class: 'board-train-name', text: t.name || '' }),
        ui.el('span', { class: 'board-route', text: t.route || '\u2014' })),
      ui.el('span', { class: 'board-times' },
        boardTime(ui, 'SCH', t.sta, false),
        boardTime(ui, t.eta ? 'ETA' : 'ETD', t.eta || t.dep || '', !!late),
        platformCell(ui, t)),
    );
    board.append(row);
  });
  return ui.card('Live Board', board);
}

function boardTime(ui, label, value, late) {
  const cell = ui.el('span', { class: 'board-time' + (late ? ' late' : '') });
  cell.append(
    ui.el('span', { class: 'bt-label', text: label }),
    ui.el('span', { class: 'bt-val', text: ui.fmtTime(value) }),
  );
  return cell;
}

function platformCell(ui, t) {
  const cell = ui.el('span', { class: 'board-time' });
  cell.append(
    ui.el('span', { class: 'bt-label', text: 'PF' }),
    ui.el('span', { class: 'bt-platform', text: t.platform || '\u2014' }),
  );
  return cell;
}

function delayMinutes(t) {
  const parse = (s) => {
    const m = /^(\d{1,2}):?(\d{2})$/.exec(String(s || '').trim());
    return m ? (+m[1] * 60 + +m[2]) : null;
  };
  const a = parse(t.eta);
  const b = parse(t.sta);
  return a != null && b != null && a > b ? a - b : null;
}

/* ---------- tt ---------- */

function viewTT(container, ctx, params) {
  const ui = ctx.ui;

  const { wrap, input } = ui.stationInput('Station Code');
  const submit = ui.el('button', { class: 'btn', text: 'Get Trains' });
  const results = ui.el('div', { class: 'mt-8' });

  function submitForm() {
    RailLog.action('station_timetable', 'submit', { station_raw: input.value });
    const check = ui.stationCode(input.value);
    if (check.error) {
      RailLog.action('station_timetable', 'validation', { error: check.error, raw: input.value });
      ui.render(results, ui.errorBox(check.error));
      return;
    }
    RailLog.action('station_timetable', 'validated', { station: check.code });
    const targetHash = Routes.href({ section: 'station', view: 'tt', params: { station: check.code } });
    if (location.hash !== targetHash) history.replaceState(null, '', targetHash);
    ui.fetchFlow(results, () => ctx.api.stationTimetable(check.code), { button: submit, failText: 'Failed to load the station timetable' })
      .then((res) => {
        if (res) {
          if (pendingHero) pendingHero.remove();
          ui.render(results, ...renderTT(res, check.code, ui, ctx));
        }
      });
  }

  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  ui.render(container, ui.el('div', { class: 'card-sm' }, ui.el('div', { class: 'row', style: 'gap:6px;' }, wrap, submit)), results);

  let pendingHero = null;
  if (params.station) {
    fillInput(params.station, input);
    pendingHero = stationHero(params.station, {}, ui, ctx, {
      view: 'tt',
      facts: [['Total trains', '—'], ['Data source', '—']],
    });
    container.append(pendingHero);
    submitForm();
  }
}

function renderTT(res, code, ui, ctx) {
  const hero = stationHero(code, res, ui, ctx, {
    view: 'tt',
    facts: [['Total trains', res.total || 0], ['Data source', res.data_source || 'unknown']],
  });
  const trains = Array.isArray(res.trains) ? res.trains : [];
  const list = ui.card('Trains',
    trains.length
      ? ui.collapsibleTable(['No.', 'Train', 'Route', 'Arrival', 'Departure', 'Days', 'Type'],
          trains.map((t, i) => [
            (i + 1).toString(),
            ui.entityLink('train', t.number, t.number + ' ' + t.name, ctx.navigate),
            t.route || '—',
            `<span class="mono">${ui.fmtTime(t.arrival)}</span>`,
            `<span class="mono">${ui.fmtTime(t.departure)}</span>`,
            t.days,
            t.train_type,
          ]))
      : ui.notice('No trains found.'));
  return [hero, list];
}

/* ---------- heritage ---------- */

const SELECTIONS = [
  [0, 'All Heritage Trains'],
  [1, 'Kalka Shimla Railway'],
  [2, 'Matheran Hill Railway'],
  [3, 'Kangra Valley Railway'],
  [4, 'Nilgiri Mountain Railway'],
  [5, 'Darjeeling Himalayan Railway'],
];

function viewHeritage(container, ctx) {
  const ui = ctx.ui;
  const select = ui.el('select', { class: 'input' });
  SELECTIONS.forEach(([value, label]) => {
    select.append(ui.el('option', { value: String(value), text: label }));
  });
  const submit = ui.el('button', { class: 'btn', text: 'Get Trains' });
  const results = ui.el('div', { class: 'mt-8' });
  const submitForm = () => {
    ui.render(results, ui.spinner());
    submit.disabled = true;
    ctx.api.heritage(select.value)
      .then((res) => {
        if (!res || res.ok === false) {
          ui.render(results, ui.errorState('Could not load heritage trains', res && res.error ? res.error : 'Failed to load heritage trains', submitForm));
          return;
        }
        ui.render(results, ...renderHeritage(res, ui));
      })
      .catch((err) => {
        ui.render(results, ui.errorState('Could not load heritage trains', err && err.message ? err.message : String(err), submitForm));
      })
      .finally(() => { submit.disabled = false; });
  };
  submit.addEventListener('click', submitForm);
  ui.render(container, ui.el('div', { class: 'card-sm' }, ui.el('div', { class: 'row', style: 'gap:6px;' }, select, submit)), results);
  submitForm();
}

function renderHeritage(res, ui) {
  const trains = Array.isArray(res.trains) ? res.trains : [];
  const summary = ui.card('Summary',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge('Total: ' + (res.total ?? 0), 'blue'),
      res.data_source ? ui.badge('Source: ' + res.data_source, 'slate') : null,
    ),
  );
  const list = ui.card('Trains',
    trains.length
      ? ui.collapsibleTable(['Train', 'Runs', 'From', 'To', 'Duration'],
          trains.map((t) => [
            `${t.number} ${t.name}`,
            `${t.runs} | ${t.train_type}`,
            `${t.source_station} (${t.source_code}) ${t.source_time}`,
            `${t.dest_station} (${t.dest_code}) ${t.dest_time}`,
            t.duration,
          ]))
      : ui.emptyState('train', 'No heritage trains', 'Try a different selection.'),
  );
  return [summary, list];
}

/* ---------- parcel ---------- */

function viewParcel(container, ctx) {
  const ui = ctx.ui;
  const refresh = ui.el('button', { class: 'btn', text: 'Refresh' });
  const results = ui.el('div', { class: 'mt-8' });
  const fetchParcel = () => {
    ui.render(results, ui.spinner());
    refresh.disabled = true;
    ctx.api.parcel()
      .then((res) => {
        if (!res || res.ok === false) {
          ui.render(results, ui.errorState('Could not load parcel special trains', res && res.error ? res.error : 'Failed to load parcel special trains', fetchParcel));
          return;
        }
        ui.render(results, ...renderParcel(res, ui));
      })
      .catch((err) => {
        ui.render(results, ui.errorState('Could not load parcel special trains', err && err.message ? err.message : String(err), fetchParcel));
      })
      .finally(() => { refresh.disabled = false; });
  };
  refresh.addEventListener('click', fetchParcel);
  ui.render(container, ui.el('div', { class: 'card-sm' }, ui.el('div', { class: 'row', style: 'gap:6px;' }, refresh)), results);
  fetchParcel();
}

function renderParcel(res, ui) {
  const trains = Array.isArray(res.trains) ? res.trains : [];
  const source = ui.card('',
    ui.el('div', { class: 'row align-center mt-8' },
      res.data_source ? ui.badge('Source: ' + res.data_source, 'slate') : null,
    ),
  );
  const list = ui.card('Parcel Special Trains',
    trains.length
      ? ui.collapsibleTable(['No.', 'Train', 'Route', 'Days', 'Validity', 'From', 'To', 'Travel'],
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
      : ui.emptyState('train', 'No parcel special trains', 'Try again later.'),
  );
  return [source, list];
}
})();