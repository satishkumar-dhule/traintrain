(() => {
window.Sections = window.Sections || {};

const CLASSES = [
  ['ALL', 'All Classes'],
  ['1A', 'AC First Class (1A)'],
  ['2A', 'AC 2 Tier (2A)'],
  ['3A', 'AC 3 Tier (3A)'],
  ['3E', 'AC 3 Economy (3E)'],
  ['FC', 'First Class (FC)'],
  ['SL', 'Sleeper (SL)'],
  ['2S', 'Second Sitting (2S)'],
];

window.Sections.plan = {
  mount(container, ctx, route) {
    const p = route.params || {};
    const view = route.view || 'trains';
    const views = ['trains', 'availability', 'chart'];
    const labels = { trains: 'Trains', availability: 'Availability', chart: 'Chart' };
    const pills = ctx.ui.pillBar(views, labels, view, (v) => {
      ctx.navigate(Routes.href({ section: 'plan', view: v, params: p }));
    });
    const content = ctx.ui.el('div');
    ctx.ui.render(container, pills, content);
    switch (view) {
      case 'availability': viewAvailability(content, ctx, p); break;
      case 'chart': viewChart(content, ctx, p); break;
      default: viewTrains(content, ctx, p);
    }
  },
  weekdayName,
};

function fillInput(value, input) {
  if (value && input) input.value = String(value).trim();
}

function stationPairValid(ui, fromInput, toInput, results) {
  const srcCheck = ui.stationCode(fromInput.value);
  if (srcCheck.error) {
    ui.render(results, ui.errorBox(`From station: ${srcCheck.error}`));
    return null;
  }
  const dstCheck = ui.stationCode(toInput.value);
  if (dstCheck.error) {
    ui.render(results, ui.errorBox(`To station: ${dstCheck.error}`));
    return null;
  }
  const src = srcCheck.code;
  const dst = dstCheck.code;
  if (src === dst) {
    ui.render(results, ui.errorBox('Source and destination must differ.'));
    return null;
  }
  return { src, dst };
}

function planHash(src, dst, extras) {
  const params = { src, dst };
  if (extras && extras.date) params.date = extras.date;
  if (extras && extras.class) params.class = extras.class;
  if (extras && extras.flex) params.flex = '1';
  if (extras && extras.berth) params.berth = '1';
  return Routes.href({ section: 'plan', params });
}

function renderHero(wrap, ui, ctx, pair, extras) {
  const hash = planHash(pair.src, pair.dst, extras);
  const actions = [
    ui.el('button', { class: 'btn', onclick: () => ctx.copyLink(hash) }, ui.icon('copy', 'btn-ic'), 'Copy link'),
    ui.el('button', { class: 'btn', onclick: () => ctx.share(hash) }, ui.icon('share', 'btn-ic'), 'Share'),
  ];
  ui.render(wrap, ui.entityHero({
    icon: 'plan',
    title: `${pair.src} → ${pair.dst}`,
    actions,
  }));
}

function travelDuration(dep, arr) {
  const d = String(dep || '');
  const a = String(arr || '');
  if (!/^\d{4}$/.test(d) || !/^\d{4}$/.test(a)) return '';
  let depMin = (+d.slice(0, 2)) * 60 + (+d.slice(2));
  let arrMin = (+a.slice(0, 2)) * 60 + (+a.slice(2));
  if (arrMin < depMin) arrMin += 1440;
  const h = Math.floor((arrMin - depMin) / 60);
  const m = (arrMin - depMin) % 60;
  return h + 'h ' + (m ? m + 'm' : '0m');
}

/* Weekday index (0 = Mon .. 6 = Sun) of a YYYY-MM-DD date, matching the
   runs_on array order used by the backend. */
function weekdayIndex(dateIso) {
  const d = new Date(String(dateIso) + 'T00:00:00');
  if (isNaN(d.getTime())) return -1;
  return (d.getDay() + 6) % 7;
}

/* Full weekday name for a runs_on index (0 = Monday .. 6 = Sunday). */
function weekdayName(i) {
  return ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'][i] || '';
}

/* ---------- IRCTC-style search form ---------- */

function searchForm(ui, opts) {
  const rb = ui.routeBox({ from: opts.from || '', to: opts.to || '' });
  const date = ui.flDate({ label: 'Journey Date', initial: opts.date || ui.today(), cls: 'console-date' });
  const cls = opts.withClass !== false
    ? ui.flSelect({ label: 'Class', icon: 'train', cls: 'console-class', options: CLASSES, value: opts.class || 'ALL' })
    : null;
  const flex = opts.withChecks !== false ? ui.checkRow({ label: 'Flexible with date', checked: !!opts.flex }) : null;
  const berth = opts.withChecks !== false ? ui.checkRow({ label: 'Trains with available berth', checked: !!opts.berth }) : null;
  const checks = flex ? ui.el('div', { class: 'console-checks' }, flex.row, berth.row) : null;
  const btn = ui.searchBtn({ label: opts.btnLabel || 'Search', onclick: opts.onSubmit });
  const form = ui.el('div', { class: 'console-form' }, rb.wrap, date.wrap);
  if (cls) form.append(cls.wrap);
  if (checks) form.append(checks);
  form.append(btn);

  rb.from.addEventListener('keydown', (e) => { if (e.key === 'Enter') opts.onSubmit(); });
  rb.to.addEventListener('keydown', (e) => { if (e.key === 'Enter') opts.onSubmit(); });
  return { form, rb, date, cls, flex, berth, btn };
}

/* ---------- trains ---------- */

function viewTrains(container, ctx, params) {
  const ui = ctx.ui;
  const p = params || {};

  const results = ui.el('div', { class: 'mt-8' });
  const heroWrap = ui.el('div');

  const fetchTrains = (pair, dateVal, flexOn, clsVal, berthOn) => {
    ui.fetchFlow(results, () => ctx.api.trainsBetween(pair.src, pair.dst), { button: form.btn, failText: 'Failed to load trains between stations' })
      .then((res) => {
        renderHero(heroWrap, ui, ctx, pair, { date: dateVal, class: clsVal, flex: flexOn, berth: berthOn });
        if (res) ui.render(results, ...renderTrains(res, ui, ctx, pair.src, pair.dst, dateVal, flexOn, clsVal, berthOn));
      });
  };

  const submit = () => {
    RailLog.action('trains_between', 'submit', {
      src_raw: form.rb.from.value, dst_raw: form.rb.to.value,
    });
    const pair = stationPairValid(ui, form.rb.from, form.rb.to, results);
    if (!pair) return;
    RailLog.action('trains_between', 'validated', pair);
    const dateVal = form.date.getDate();
    const target = Routes.href({
      section: 'plan', view: 'trains',
      params: {
        src: pair.src, dst: pair.dst, date: dateVal,
        class: form.cls && form.cls.get() !== 'ALL' ? form.cls.get() : '',
        flex: form.flex && form.flex.get() ? '1' : '',
        berth: form.berth && form.berth.get() ? '1' : '',
      },
    });
    if (location.hash === target) fetchTrains(pair, dateVal, !!form.flex.get(), form.cls.get(), !!form.berth.get());
    else ctx.navigate(target);
  };

  const form = searchForm(ui, {
    from: p.src || '', to: p.dst || '',
    date: p.date || ui.today(),
    class: p.class || 'ALL',
    flex: p.flex, berth: p.berth,
    btnLabel: 'Find Trains',
    onSubmit: submit,
  });

  ui.render(container, heroWrap, form.form, results);

  if (p.src && p.dst) {
    renderHero(heroWrap, ui, ctx, { src: p.src, dst: p.dst }, p);
    submit();
  }
}

function renderTrains(res, ui, ctx, src, dst, dateIso, flex, clsVal, berthOn) {
  const trains = Array.isArray(res.trains) ? res.trains : [];
  if (!trains.length) {
    return [ui.emptyState('train', 'No direct trains', `No direct trains found between ${res.src || ''} and ${res.dst || ''}.`)];
  }
  const parts = [];
  const idx = dateIso ? weekdayIndex(dateIso) : -1;
  const runsOnDay = (t) => idx >= 0 && Array.isArray(t.runs_on) && t.runs_on[idx] === true;
  const running = flex ? trains : trains.filter(runsOnDay);
  if (!running.length) {
    parts.push(ui.emptyState('train', 'No trains on this day',
      `None of the ${trains.length} direct trains between ${src} and ${dst} run on ${ui.friendlyDate(dateIso)}. Tick "Flexible with date" to show all.`));
    return parts;
  }
  const dayFiltered = idx >= 0 && !flex && running.length !== trains.length;
  parts.push(ui.el('div', { class: 'card-sm' },
    ui.el('div', { class: 'row align-center' },
      ui.el('h2', { class: 'card-title', text: 'Direct trains' }),
      dayFiltered ? ui.badge(`${running.length} of ${trains.length} · ${weekdayName(idx)}`, 'slate') : null,
    ),
    ui.el('div', { class: 'col' },
      ...running.map((t) => trainCard(t, ui, ctx, src, dst, idx >= 0 && !runsOnDay(t) ? weekdayName(idx) : null)))));
  return parts;
}

function trainCard(t, ui, ctx, src, dst, notRunningDay) {
  const btn = ui.el('button', {
    class: 'train-card',
    onclick: () => ctx.navigate(Routes.href({ section: 'train', params: { train: t.number } })),
    'aria-label': 'Open train ' + (t.number || '') + (t.name ? ' ' + t.name : ''),
  });
  btn.append(ui.el('span', { class: 'train-card-num', text: t.number || '?' }));
  const body = ui.el('div', { class: 'train-card-body' });
  body.append(ui.el('div', { class: 'train-card-name', text: t.name || '' }));
  const meta = ui.el('div', { class: 'train-card-meta' });
  meta.append(ui.badge(ui.days(t.runs_on), 'slate'));
  if (notRunningDay) meta.append(ui.badge('Not on ' + notRunningDay, 'amber'));
  body.append(meta);
  const times = ui.el('div', { class: 'train-card-times' });
  times.append(
    ui.el('span', { class: 'train-card-time', text: ui.fmtTime(t.departure_time) }),
    ui.el('span', { class: 'train-card-station', text: src || '' }),
    ui.el('span', { class: 'train-card-arrow', text: '→' }),
    ui.el('span', { class: 'train-card-dur', text: travelDuration(t.departure_time, t.arrival_time) }),
    ui.el('span', { class: 'train-card-arrow', text: '→' }),
    ui.el('span', { class: 'train-card-time', text: ui.fmtTime(t.arrival_time) }),
    ui.el('span', { class: 'train-card-station', text: dst || '' }),
  );
  body.append(times);
  btn.append(body);
  return btn;
}

/* ---------- availability ---------- */

const AVL_SOURCES = [
  ['auto', 'Source: Auto (Paytm → IRCTC)'],
  ['paytm', 'Source: Paytm'],
  ['irctc', 'Source: IRCTC'],
];

function viewAvailability(container, ctx, params) {
  const ui = ctx.ui;
  const p = params || {};

  const results = ui.el('div', { class: 'mt-8' });
  const heroWrap = ui.el('div');

  const fetchAvail = (pair, dateVal, srcVal) => {
    ui.fetchFlow(results, () => ctx.api.availability(pair.src, pair.dst, dateVal, srcVal), { button: form.btn, failText: 'Failed to load availability' })
      .then((res) => {
        renderHero(heroWrap, ui, ctx, pair, res, { date: dateVal });
        if (res) ui.render(results, ...renderAvailability(res, ui, ctx));
      });
  };

  const submit = () => {
    const pair = stationPairValid(ui, form.rb.from, form.rb.to, results);
    if (!pair) return;
    const dateVal = form.date.getDate();
    const srcVal = sourceSel.get();
    const target = Routes.href({
      section: 'plan', view: 'availability',
      params: { src: pair.src, dst: pair.dst, date: dateVal, source: srcVal !== 'auto' ? srcVal : '' },
    });
    if (location.hash === target) fetchAvail(pair, dateVal, srcVal);
    else ctx.navigate(target);
  };

  const form = searchForm(ui, {
    from: p.src || '', to: p.dst || '',
    date: p.date || ui.today(),
    btnLabel: 'Check Availability',
    onSubmit: submit,
    withClass: false,
    withChecks: false,
  });
  const sourceSel = ui.flSelect({
    label: 'Source', icon: 'ticket', cls: 'console-class',
    options: AVL_SOURCES, value: p.source || 'auto',
  });
  form.form.insertBefore(sourceSel.wrap, form.btn);

  ui.render(container, heroWrap, form.form, results);

  if (p.src && p.dst) {
    renderHero(heroWrap, ui, ctx, { src: p.src, dst: p.dst }, null, p);
    submit();
  }
}

function renderAvailability(res, ui, ctx) {
  const trains = res.trains || [];
  const hasClasses = Array.isArray(trains) && trains.some((t) => Array.isArray(t.classes) && t.classes.length);
  return [ui.el('div', { class: 'card-sm' },
    ui.el('div', { class: 'row align-center' },
      ui.el('h2', { class: 'card-title', text: 'Trains' }),
      ui.badge(res.src || '', 'blue'),
      ui.el('span', { class: 'bold', text: '→' }),
      ui.badge(res.dst || '', 'blue'),
      ui.badge(ctx.ui.friendlyDate(res.date), 'slate'),
    ),
    !hasClasses && res.notice ? ctx.ui.notice(res.notice) : null,
    Array.isArray(trains) && trains.length
      ? ui.collapsibleTable(['No.', 'Train', 'Departure', 'Arrival', 'Duration', 'Classes', 'Availability'],
          trains.map((t) => [
            ui.entityLink('train', t.number || '', t.number || '', ctx.navigate),
            t.name || '',
            ui.fmtTime(t.departure_time),
            ui.fmtTime(t.arrival_time),
            t.duration || '',
            classChips(t.classes, ui),
            avlChips(t, ui),
          ]))
      : ui.emptyState('ticket', 'No availability', 'No availability data returned for this route and date.'),
  )];
}

function avlChips(t, ui) {
  const list = Array.isArray(t.availability) ? t.availability : [];
  if (!list.length) return ui.el('span', { class: 'text-sm muted', text: '—' });
  return ui.el('span', { class: 'col', style: 'gap:4px;' },
    ...list.map((a) => {
      const fare = a.fare != null ? ` ₹${a.fare}` : '';
      return ui.badge(`${a.class} ${a.status || ''}${fare}`.trim(), avlKind(a));
    }));
}

function avlKind(a) {
  if (a.available === true) return 'green';
  const s = String(a.status || '').toUpperCase();
  if (s.indexOf('AVAILABLE') !== -1 && s.indexOf('NOTAVAILABLE') === -1) return 'green';
  if (s.indexOf('RAC') !== -1 || s.indexOf('WL') !== -1) return 'amber';
  return 'slate';
}

function classChips(classes, ui) {
  if (!Array.isArray(classes) || !classes.length) return ui.el('span', { class: 'text-sm muted', text: '—' });
  return ui.el('span', { class: 'row', style: 'gap:4px;' },
    ...classes.map((c) => ui.badge(String(c), classKind(c))));
}

function classKind(cls) {
  const s = String(cls || '').toUpperCase();
  if (s.indexOf('AVAILABLE') !== -1 || s.indexOf('AVBL') !== -1) return 'green';
  if (s.indexOf('WL') !== -1 || s.indexOf('RAC') !== -1) return 'amber';
  return 'slate';
}

/* ---------- chart ---------- */

function viewChart(container, ctx, params) {
  const ui = ctx.ui;

  const train = ui.trainInput('Train No.');
  const dateInput = ui.el('input', { class: 'input', type: 'date', value: ui.today() });
  const station = ui.stationInput('Boarding (opt.)');
  const submit = ui.el('button', { class: 'btn', text: 'Get Chart' });
  const results = ui.el('div', { class: 'mt-8' });

  const submitForm = () => {
    const trainValue = train.input.value.trim();
    if (!/^[0-9]{1,8}$/.test(trainValue)) {
      ui.render(results, ui.errorBox('Enter a valid train number (digits only).'));
      return;
    }
    const date = (dateInput.value || '').trim() || undefined;
    const rawStation = station.input.value.trim();
    let stationCode;
    if (rawStation) {
      const sCheck = ui.stationCode(rawStation);
      if (sCheck.error) {
        ui.render(results, ui.errorBox(`Boarding station: ${sCheck.error}`));
        return;
      }
      stationCode = sCheck.code;
    }
    ui.fetchFlow(results, () => ctx.api.chart(trainValue, date, stationCode), { button: submit, failText: 'Failed to load the coach chart' })
      .then((res) => { if (res) ui.render(results, ...ui.chartView(res, ui, ctx)); });
  };

  train.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  dateInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  station.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  if (params.src) {
    const sCheck = ui.stationCode(params.src);
    if (!sCheck.error) fillInput(sCheck.code, station.input);
  }

  ui.render(container,
    ui.el('div', { class: 'card-sm' }, ui.el('div', { class: 'row', style: 'gap:6px;' }, train.wrap, dateInput, station.wrap, submit)),
    results);
}
})();