/* sections/plan.js - Plan section. Views: trains (trains between), availability
   (IRCTC berth availability), chart (IRCTC coach chart). Deep links like
   #/plan/NDLS/BSB/availability auto-submit; the chart view needs a train
   number, so it opens the form with the boarding station prefilled from src.
   Live-data-only. */

(() => {
window.Sections = window.Sections || {};

window.Sections.plan = {
  mount(container, ctx, route) {
    const p = route.params || {};
    const view = route.view || 'trains';
    switch (view) {
      case 'availability': viewAvailability(container, ctx, p); break;
      case 'chart': viewChart(container, ctx, p); break;
      default: viewTrains(container, ctx, p);
    }
  },
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

/* ---------- trains ---------- */

function viewTrains(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Trains Between Stations',
    ui.el('p', { class: 'text-sm muted', text: 'Direct trains (NTES)' }),
  );

  const from = ui.stationInput('From Station Code');
  const to = ui.stationInput('To Station Code');
  const submit = ui.el('button', { class: 'btn', text: 'Find Trains' });
  const results = ui.el('div', { class: 'mt-12' });

  const submitForm = () => {
    RailLog.action('trains_between', 'submit', {
      src_raw: from.input.value, dst_raw: to.input.value,
    });
    const pair = stationPairValid(ui, from.input, to.input, results);
    if (!pair) return;
    RailLog.action('trains_between', 'validated', pair);
    ui.fetchFlow(results, () => ctx.api.trainsBetween(pair.src, pair.dst), { button: submit, failText: 'Failed to load trains between stations' })
      .then((res) => { if (res) ui.render(results, ...renderTrains(res, ui)); });
  };

  from.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  to.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  ui.render(container, header,
    ui.queryCard([['From Station Code', from.wrap], ['To Station Code', to.wrap]], submit),
    results);

  if (params.src && params.dst) {
    fillInput(params.src, from.input);
    fillInput(params.dst, to.input);
    submitForm();
  }
}

function renderTrains(res, ui) {
  const route = ui.card('Route',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge(res.src || '', 'blue'),
      ui.el('span', { class: 'bold', text: '→' }),
      ui.badge(res.dst || '', 'blue'),
      ui.badge('Data source: ' + (res.data_source || 'unknown'), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['No.', 'Train', 'Departure', 'Arrival', 'Days'],
          trains.map((t) => [
            ui.el('span', { class: 'mono', text: t.number || '' }).outerHTML,
            t.name || '',
            ui.fmtTime(t.departure_time),
            ui.fmtTime(t.arrival_time),
            ui.days(t.runs_on),
          ]))
      : ui.notice('No direct trains found.'),
  );

  return [route, list];
}

/* ---------- availability ---------- */

function viewAvailability(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Availability',
    ui.el('p', { class: 'text-sm muted', text: 'Live berth availability (IRCTC). Classes vary by train.' }),
  );

  const from = ui.stationInput('From Station Code');
  const to = ui.stationInput('To Station Code');
  const dateInput = ui.el('input', { class: 'input', type: 'date', value: ui.today() });
  const submit = ui.el('button', { class: 'btn', text: 'Check Availability' });
  const results = ui.el('div', { class: 'mt-12' });

  const submitForm = () => {
    const pair = stationPairValid(ui, from.input, to.input, results);
    if (!pair) return;
    const date = (dateInput.value || '').trim() || undefined;
    ui.fetchFlow(results, () => ctx.api.availability(pair.src, pair.dst, date), { button: submit, failText: 'Failed to load availability' })
      .then((res) => { if (res) ui.render(results, ...renderAvailability(res, ui)); });
  };

  from.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  to.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  dateInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  ui.render(container, header,
    ui.queryCard([['From Station Code', from.wrap], ['To Station Code', to.wrap], ['Journey Date', dateInput]], submit),
    results);

  if (params.src && params.dst) {
    fillInput(params.src, from.input);
    fillInput(params.dst, to.input);
    submitForm();
  }
}

function renderAvailability(res, ui) {
  const route = ui.card('Route',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge(res.src || '', 'blue'),
      ui.el('span', { class: 'bold', text: '→' }),
      ui.badge(res.dst || '', 'blue'),
      ui.badge((res.data_source || 'unknown') + ' · ' + (res.date || ''), 'slate'),
    ),
  );

  const trains = res.trains || [];
  const list = ui.card('Trains',
    Array.isArray(trains) && trains.length
      ? ui.table(['No.', 'Train', 'Departure', 'Arrival', 'Duration', 'Classes'],
          trains.map((t) => [
            ui.el('span', { class: 'mono', text: t.number || '' }).outerHTML,
            t.name || '',
            ui.fmtTime(t.departure_time),
            ui.fmtTime(t.arrival_time),
            t.duration || '',
            Array.isArray(t.classes) ? t.classes.join(' · ') : '',
          ]))
      : ui.notice('No availability data returned.'),
  );
  if (res.notice) list.append(ui.el('p', { class: 'notice' }, res.notice));
  return [route, list];
}

/* ---------- chart ---------- */

function viewChart(container, ctx, params) {
  const ui = ctx.ui;
  const header = ui.card('Coach Chart',
    ui.el('p', { class: 'text-sm muted', text: 'Live coach & berth reservation chart (IRCTC).' }),
  );

  const train = ui.trainInput('Train No. (5 digits)');
  const dateInput = ui.el('input', { class: 'input', type: 'date', value: ui.today() });
  const station = ui.stationInput('Boarding station (optional)');
  const submit = ui.el('button', { class: 'btn', text: 'Get Coach Chart' });
  const results = ui.el('div', { class: 'mt-12' });

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
      .then((res) => { if (res) ui.render(results, ...renderChart(res, ui)); });
  };

  train.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  dateInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  station.input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submitForm(); });
  submit.addEventListener('click', submitForm);

  if (params.src) {
    const sCheck = ui.stationCode(params.src);
    if (!sCheck.error) fillInput(sCheck.code, station.input);
  }

  ui.render(container, header,
    ui.queryCard([['Train Number', train.wrap], ['Journey Date', dateInput], ['Boarding Station', station.wrap]], submit),
    results);
}

function renderChart(res, ui) {
  const headerRow = ui.card('Train',
    ui.el('div', { class: 'row align-center mt-8' },
      ui.badge(res.train_number || '', 'blue'),
      ui.el('span', { class: 'bold', text: res.train_name || '' }),
      ui.badge('Journey ' + (res.journey_date || ''), 'slate'),
      res.boarding_station ? ui.badge('Boarding ' + res.boarding_station, 'slate') : null,
    ),
  );
  if (res.notice) headerRow.append(ui.el('p', { class: 'notice' }, res.notice));

  const coaches = res.coaches || [];
  const legend = ui.el('div', { class: 'row align-center mt-8', style: 'gap:12px;flex-wrap:wrap;' },
    ui.el('span', { class: 'text-sm muted', text: 'Berth status:' }),
    ui.badge('vacant', 'green'),
    ui.badge('occupied', 'red'),
    ui.badge('not reserved', 'slate'),
  );

  const list = ui.card('Coaches',
    Array.isArray(coaches) && coaches.length
      ? ui.table(['Coach', 'Class', 'Berths'],
          coaches.map((c) => [
            ui.el('span', { class: 'mono', text: c.code || '' }).outerHTML,
            c.class_code || '',
            berthCell(c.berths, ui),
          ]))
      : ui.notice('No coach data returned.'),
  );

  return [headerRow, legend, list];
}

/* Render berths as a row of small status-coloured squares (title = "N: status"). */
function berthCell(berths, ui) {
  const row = ui.el('div', { class: 'row', style: 'gap:4px;flex-wrap:wrap;' });
  (Array.isArray(berths) ? berths : []).forEach((b) => {
    const cls = b.status === 'vacant' ? 'berth vacant' : b.status === 'occupied' ? 'berth occupied' : 'berth not-reserved';
    row.append(ui.el('span', {
      class: cls,
      title: `${b.number}: ${b.status}`,
      text: String(b.number),
    }));
  });
  if (!row.children.length) row.append(ui.el('span', { class: 'text-sm muted', text: '—' }));
  return row;
}
})();
