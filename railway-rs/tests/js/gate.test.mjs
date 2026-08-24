/* gate.test.mjs - unit tests for the local-first intent router.
   Pins the classification contract (trivial/replay/tool/confirm/help — there
   is no llm kind anymore), the plan shapes, the fuzzy/Hinglish matching
   pipeline, and that client-side projections mirror the server's ai_chat
   card data exactly (ToolCards.svelte renders these shapes). */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const gate = require('../../frontend/src/lib/chat/gate.js');
const memory = require('../../frontend/src/lib/chat/memory.js');

test('greetings, thanks, bye and help never reach the network', () => {
  for (const q of ['hi', 'hello!', 'namaste', 'thanks', 'thank you!', 'bye', 'what can you do?']) {
    const v = gate.classify(q);
    assert.equal(v.kind, 'trivial', `${q} -> ${v.kind}`);
    assert.ok(v.reply.length > 5);
  }
});

test('classify routes remembered questions to replay (integration with memory.js)', () => {
  const m = memory.createMemory();
  // Must not throw: classify wires findReplay from memory.js internally.
  assert.equal(gate.classify('live status of 12951', m).kind, 'tool');
  memory.remember(m, 'live status of 12951', { content: 'cached answer', cards: [], actions: [] });
  const v = gate.classify('live status of 12951', m);
  assert.equal(v.kind, 'replay');
  assert.equal(v.entry.answer.content, 'cached answer');
});

test('classify NEVER returns kind llm for any input', () => {
  const samples = [
    '', 'best food near pune?', 'plan a 3 day trip', 'status',
    'where is my train', 'liv staus of 12951', '12951 kahan hai',
    'how much delay usually in 12626', 'seat available from SC to PUNE',
    'chart bana 12951?', 'trains from secunderabad to pune', 'zzz qq qq xx'
  ];
  for (const q of samples) {
    assert.notEqual(gate.classify(q).kind, 'llm', q);
    assert.notEqual(gate.classify(q).kind, undefined, q);
  }
});

test('garbage and unmatchable questions fall through to help with capability chips', () => {
  for (const q of ['best food near pune?', 'best food on trains?', 'plan a 3 day trip', '']) {
    const v = gate.classify(q);
    assert.equal(v.kind, 'help', q);
    assert.ok(v.reply.includes('live'), `${q} -> reply mentions capabilities`);
    assert.ok(Array.isArray(v.actions) && v.actions.length >= 3 && v.actions.length <= 4);
    for (const a of v.actions) assert.ok(a.label && a.prompt);
  }
  assert.equal(gate.HELP_CHIPS.length >= 3, true);
  assert.equal(typeof gate.HELP_REPLY, 'string');
});

test('live status intent plans the plain REST endpoint with no resolution step', () => {
  for (const q of ['live status of 12951', 'where is 12951', '12951 running status', 'is 12626 delayed']) {
    const v = gate.classify(q);
    assert.equal(v.kind, 'tool', q);
    assert.equal(v.plan.cardKind, 'live_status');
    assert.equal(v.plan.params.train, q.match(/\d{4,5}/)[0]);
    assert.equal(v.plan.resolve, undefined);
    assert.equal(v.plan.url, '/rail-api/live-status');
  }
});

test('typos still match: "liv staus of 12951" routes via fuzzy corpus to live_status', () => {
  const v = gate.classify('liv staus of 12951');
  assert.equal(v.kind, 'tool');
  assert.equal(v.plan.cardKind, 'live_status');
  assert.equal(v.plan.params.train, '12951');
  assert.ok(v.confidence > 0.62, `confidence ${v.confidence}`);
});

test('hinglish phrasing routes correctly', () => {
  const where = gate.classify('12951 kahan hai');
  assert.equal(where.kind, 'tool');
  assert.equal(where.plan.cardKind, 'live_status');
  assert.equal(where.plan.params.train, '12951');

  const delay = gate.classify('how much delay usually in 12626');
  assert.equal(delay.kind, 'tool');
  assert.equal(delay.plan.cardKind, 'average_delay');
  assert.equal(delay.plan.url, '/rail-api/ntes/average-delay');

  const chart = gate.classify('chart bana 12951?');
  assert.equal(chart.kind, 'tool');
  assert.equal(chart.plan.cardKind, 'chart_status');
  assert.equal(chart.plan.url, '/rail-api/irctc/chart');
  assert.equal(chart.plan.params.train, '12951');
});

test('normalize applies punctuation strip, suffix strip and the exported HINGLISH map', () => {
  assert.equal(gate.normalize('Liv Staus!! of  12951.'), 'liv staus of 12951');
  assert.equal(gate.normalize('kahan hai mera train'), 'where is my train');
  assert.equal(gate.normalize('chal rahi hai kya'), 'running is kya');
  assert.ok(gate.HINGLISH.kahan === 'where');
  assert.ok(gate.HINGLISH.der === 'delay');
  assert.ok(gate.HINGLISH['ban gaya'] === 'prepared');
  // suffix stripper keeps short stems and -us words intact
  assert.equal(gate.normalize('status this yes'), 'status this yes');
  assert.equal(gate.normalize('trains seats charts'), 'train seat chart');
});

test('extractEntities pulls train numbers and normalizes dates', () => {
  assert.deepEqual(gate.extractEntities('liv staus of 12951'), { train: '12951', date: null });
  assert.deepEqual(gate.extractEntities('aaj 12951'), { train: '12951', date: 'today' });
  const kal = gate.extractEntities('kal 12951');
  assert.equal(kal.train, '12951');
  assert.match(kal.date, /^\d{4}-\d{2}-\d{2}$/); // tomorrow materialized as ISO
  assert.deepEqual(gate.extractEntities('seat SC PUNE 20-10-2026'), { train: null, date: '20-10-2026' });
  assert.deepEqual(gate.extractEntities('chart 12951 2026-10-20'), { train: '12951', date: '2026-10-20' });
  assert.deepEqual(gate.extractEntities('hello'), { train: null, date: null });
});

test('route/schedule and average-delay intents map to their endpoints', () => {
  const route = gate.classify('route of 12951');
  assert.equal(route.kind, 'tool');
  assert.equal(route.plan.cardKind, 'train_schedule');
  assert.equal(route.plan.url, '/rail-api/schedule');

  const delay = gate.classify('average delay of 12951');
  assert.equal(delay.kind, 'tool');
  assert.equal(delay.plan.cardKind, 'average_delay');
  assert.equal(delay.plan.url, '/rail-api/ntes/average-delay');
});

test('between intent resolves both stations then calls trains-between', () => {
  const v = gate.classify('trains from secunderabad to pune');
  assert.equal(v.kind, 'tool');
  assert.equal(v.plan.cardKind, 'trains_between');
  assert.deepEqual(
    v.plan.resolve.map((r) => r.slot),
    ['src', 'dst']
  );
  assert.equal(v.plan.params.src, '$src');
  assert.equal(v.plan.params.dst, '$dst');
});

test('station board intent resolves one station code', () => {
  const v = gate.classify('station board pune');
  assert.equal(v.kind, 'tool');
  assert.equal(v.plan.cardKind, 'station_board');
  assert.equal(v.plan.resolve[0].slot, 'station');
  // "trains at <station>" form works too
  assert.equal(gate.classify('trains at secunderabad').kind, 'tool');
});

test('heavy seat_availability always confirms before executing (never a silent tool)', () => {
  const v = gate.classify('seat available from SC to PUNE');
  assert.equal(v.kind, 'confirm');
  assert.equal(v.plan.cardKind, 'seat_availability');
  assert.equal(v.plan.url, '/rail-api/availability');
  assert.equal(v.plan.params.src, '$src');
  assert.equal(v.plan.params.dst, '$dst');
  assert.deepEqual(
    v.choices,
    [
      { label: 'Confirm', value: '__exec' },
      { label: 'Cancel', value: '__cancel' }
    ]
  );
  assert.match(v.text, /Check seat availability SC → PUNE\?/);

  // even a weak/ambiguous seat hit stays in the confirm lane
  const vague = gate.classify('availability from SC to PUNE tomorrow');
  assert.equal(vague.kind, 'confirm');
  assert.equal(vague.plan.cardKind, 'seat_availability');
  assert.match(vague.plan.params.date, /^(today|\d{4}-\d{2}-\d{2})$/);

  // without stations it asks for them instead of confirming
  const missing = gate.classify('check seat availability please');
  assert.equal(missing.kind, 'help');
  assert.match(missing.reply, /(SC|from)/i);
});

test('ambiguous band asks did-you-mean with choices and runnerUp', () => {
  // 'details of <train>' partially overlaps several corpora — best score
  // lands between REJECT and ACCEPT -> confirm lane.
  const v = gate.classify('details of 12951 please');
  assert.equal(v.kind, 'confirm');
  assert.match(v.text, /^Did you mean: .+\? $/);
  assert.deepEqual(v.choices[0], { label: 'Yes, fetch it', value: '__exec' });
  assert.deepEqual(v.choices[1], { label: 'Cancel', value: '__cancel' });
  assert.ok(v.plan, 'ambiguous confirm carries the ready-to-run plan');
  // runnerUp is the strongest OTHER-intent candidate in the fuzzy top-5;
  // absent when every candidate shares the best intent.
  assert.ok(v.runnerUp === null || v.runnerUp.id !== v.plan.cardKind);
  if (v.runnerUp) assert.ok(typeof v.runnerUp.score === 'number' && v.runnerUp.score <= v.confidence);
});

test('missing required slots produce targeted one-line help with example prompts', () => {
  const noTrain = gate.classify('where is my train');
  assert.equal(noTrain.kind, 'help');
  assert.match(noTrain.reply, /\b\d{5}\b/); // inline example number
  assert.ok(noTrain.actions.length >= 1 && noTrain.actions.length <= 2);
  assert.match(noTrain.actions[0].prompt, /\b\d{5}\b/);

  const noStations = gate.classify('which trains run between these stations');
  assert.equal(noStations.kind, 'help');
  assert.match(noStations.reply, /stations/i);

  const boardless = gate.classify('show me the station announcement board');
  assert.equal(boardless.kind, 'help');
  assert.match(boardless.reply, /station/i);
});

test('"status" alone is too short and lands in generic help', () => {
  const v = gate.classify('status');
  assert.equal(v.kind, 'help');
  assert.equal(v.reply, gate.HELP_REPLY);
});

test('buildPlanFor emits the stable plan shape for every intent', () => {
  assert.deepEqual(gate.buildPlanFor('live_status', { train: '12951' }), {
    cardKind: 'live_status',
    url: '/rail-api/live-status',
    params: { train: '12951' }
  });
  assert.deepEqual(gate.buildPlanFor('average_delay', { train: '12626' }), {
    cardKind: 'average_delay',
    url: '/rail-api/ntes/average-delay',
    params: { train: '12626' }
  });
  assert.deepEqual(gate.buildPlanFor('train_schedule', { train: '12951' }), {
    cardKind: 'train_schedule',
    url: '/rail-api/schedule',
    params: { train: '12951' }
  });
  const between = gate.buildPlanFor('trains_between', {}, { srcQuery: 'secunderabad', dstQuery: 'pune' });
  assert.deepEqual(between.resolve.map((r) => r.query), ['secunderabad', 'pune']);
  const seat = gate.buildPlanFor('seat_availability', { date: 'today' }, { srcQuery: 'SC', dstQuery: 'PUNE' });
  assert.deepEqual(seat.params, { src: '$src', dst: '$dst', date: 'today' });
  const chart = gate.buildPlanFor('chart_status', { train: '12951', date: '2026-10-20' });
  assert.deepEqual(chart, { cardKind: 'chart_status', url: '/rail-api/irctc/chart', params: { train: '12951', date: '2026-10-20' } });
  assert.equal(gate.INTENTS.length, 7);
  for (const i of gate.INTENTS) {
    assert.ok(i.id && i.phrases.length >= 10, `${i.id} has >=10 phrases`);
    for (const p of i.phrases) assert.doesNotMatch(p, /\d/, `${i.id} phrase digit-free`);
  }
});

test('tokenSetDice scores exact overlap 1 and decays on unmatched tokens', () => {
  assert.equal(gate.tokenSetDice(['live', 'status'], ['live', 'status']), 1);
  const partial = gate.tokenSetDice(['liv', 'staus', 'of'], ['live', 'status', 'of', 'train']);
  assert.ok(partial > 0.8 && partial < 1, `partial ${partial}`);
  assert.equal(gate.tokenSetDice([], ['live']), 0);
  assert.equal(gate.tokenSetDice(['food'], []), 0);
});

test('resolveSlot short-circuits bare codes and picks station hits via suggest', async () => {
  const calls = [];
  const fetcher = async (url) => {
    calls.push(url);
    return {
      ok: true,
      json: async () => [
        { type: 'train', number: '17013' },
        { type: 'station', code: 'PUNE', name: 'Pune Jn' }
      ]
    };
  };
  assert.equal(await gate.resolveSlot(fetcher, { query: 'pune jn' }), 'PUNE');
  assert.equal(calls.length, 1);

  const fail = await gate.resolveSlot(
    async () => ({ ok: false, json: async () => [] }),
    { query: 'nowhere ville' }
  );
  assert.equal(fail, null);
});

test('executePlan substitutes resolved slots into params and projects nothing itself', async () => {
  const seen = [];
  const fetcher = async (url) => {
    seen.push(url);
    const code = url.includes('secunderabad') ? 'SC' : 'PUNE';
    return { ok: true, json: async () => [{ type: 'station', code }] };
  };
  const out = await gate.executePlan(
    {
      cardKind: 'trains_between',
      url: '/rail-api/ntes/trains-between',
      params: { src: '$src', dst: '$dst' },
      resolve: [
        { slot: 'src', query: 'secunderabad' },
        { slot: 'dst', query: 'pune' }
      ]
    },
    fetcher
  );
  assert.equal(seen.length, 2); // one suggest ("secunderabad") + one fetch; "pune" is already code-shaped
  assert.match(seen[1], /\/rail-api\/ntes\/trains-between\?src=SC&dst=PUNE/);
  assert.deepEqual(out, [{ type: 'station', code: 'PUNE' }]);
});

test('live_status projection mirrors server shape', () => {
  const card = gate.projectLiveStatus({
    train_number: '12951',
    train_name: 'MMCT TEJAS RAJDHANI',
    current_location_info: 'Departed from BRC',
    platform_number: '1',
    data_source: 'NTES',
    stations: [
      { name: 'BRC', code: 'BRC', scheduled_arrival: '15:02', actual_arrival: '15:14', platform: '2', delay_minutes: 12, status: 'departed' },
      { name: 'ST', code: 'ST', scheduled_arrival: '17:33', actual_arrival: '', platform: '', delay_minutes: 10, status: 'expected' }
    ]
  });
  assert.deepEqual(card, {
    train_number: '12951',
    train_name: 'MMCT TEJAS RAJDHANI',
    position: 'Departed from BRC',
    platform: '1',
    data_source: 'NTES',
    last_seen_delay_minutes: 12,
    next_stops: [{ code: 'ST', name: 'ST', sch: '17:33', act: null, delay_min: 10, platform: null }]
  });
});

test('trains_between projection caps rows, labels runs, carries resolved codes', () => {
  const mask7 = [true, true, true, true, true, true, true];
  const many = Array.from({ length: 15 }, (_, i) => ({
    number: `1234${i}`,
    name: `T${i}`,
    departure_time: '10:00',
    arrival_time: '12:00',
    runs_on: mask7
  }));
  const card = gate.projectTrainsBetween({ src: 'SC - SECUNDERABAD', dst: 'PUNE - PUNE JN', trains: many }, { src: 'SC', dst: 'PUNE' });
  assert.equal(card.total_found, 15);
  assert.equal(card.trains.length, 12);
  assert.equal(card.note, 'showing first 12 of 15');
  assert.equal(card.trains[0].runs, 'Daily');
  assert.equal(card.src_code, 'SC');
  assert.equal(card.dst_code, 'PUNE');
});

test('average_delay projection parses +/- minutes and sorts worst first', () => {
  const card = gate.projectAverageDelay({
    train_no: '11077',
    train_name: 'JHELUM EXP',
    days_of_run: 'Daily',
    data_source: 'Railyatri',
    stations: [
      { code: 'NDLS', name: 'New Delhi', arrival_delay: '+35', departure_delay: '+40' },
      { code: 'ETW', name: 'Etawah', arrival_delay: '-3', departure_delay: '' },
      { code: 'ALJN', name: 'Aligarh', arrival_delay: 'junk', departure_delay: '+5' }
    ]
  });
  assert.deepEqual(card.stations_worst_first.map((s) => s.code), ['NDLS', 'ETW', 'ALJN']);
  assert.equal(card.stations_worst_first[0].arr_delay_min, 35);
  assert.equal(card.stations_worst_first[1].arr_delay_min, -3);
  assert.equal(card.stations_worst_first[2].arr_delay_min, null);
  assert.equal(card.stations_worst_first[2].dep_delay_min, 5); // '+5' parses like the server
  assert.equal(card.stations_worst_first[1].dep_delay_min, null); // '' -> absent
});

test('schedule projection trims stops and notes the total', () => {
  const stops = Array.from({ length: 20 }, (_, i) => ({
    code: `S${i}`,
    name: `Stop ${i}`,
    arrival: '10:00',
    departure: '10:02',
    day: i > 8 ? 2 : 1,
    distance_km: i * 11.4
  }));
  const card = gate.projectSchedule({
    train_number: '12951',
    train_name: 'TEJAS',
    running_days: ['MON', 'TUE'],
    source: 'AskDISHA',
    stops
  });
  assert.equal(card.total_stops, 20);
  assert.equal(card.stops.length, 12);
  assert.equal(card.data_source, 'AskDISHA');
  assert.equal(card.stops[0].km, 0);
});

test('station_board projection maps sta->sch and delay_arr->late', () => {
  const card = gate.projectStationBoard({
    station: 'SC',
    hours: 2,
    data_source: 'NTES',
    trains: [{ number: '12723', name: 'TELANGANA EXP', sta: '09:45', eta: '10:10', delay_arr: true, platform: '3' }]
  });
  assert.deepEqual(card.trains[0], { number: '12723', name: 'TELANGANA EXP', sch: '09:45', eta: '10:10', platform: '3', late: true });
});

// Fixture mirrors tools.rs seat_availability_projection test DTO.
test('seat_availability projection ranks class-status trains first and tones classes', () => {
  const manyClasses = [
    { class: 'SL', class_name: 'Sleeper', status: 'AVAILABLE 0122', available: true, fare: 500, prediction: 92 },
    { class: '3A', class_name: 'AC 3 Tier', status: 'AVAILABLE 0044', available: true, fare: 501, prediction: 88 },
    { class: '2A', class_name: 'AC 2 Tier', status: 'RAC 12', fare: 1305 },
    { class: '1A', class_name: 'First AC', status: 'WL 8', fare: 2300 },
    { class: 'CC', class_name: 'Chair Car', status: 'REGRET/WL', available: false, fare: 900 },
    { class: 'EC', class_name: 'Exec Chair', status: 'NOT AVAILABLE', available: false },
    { class: 'EA', class_name: 'Anubhuti', status: 'AVAILABLE 0002', available: true }
  ];
  const card = gate.projectSeatAvailability({
    src: 'SC',
    dst: 'PUNE',
    date: '2026-08-22',
    data_source: 'Paytm',
    notice: 'n',
    trains: [
      { number: '00001', name: 'NO CLASSES EXP', departure_time: '06:00', arrival_time: '12:00', duration: '06:00', classes: ['SL'] },
      { number: '11111', name: 'RICH EXP', departure_time: '08:00', arrival_time: '14:00', duration: '06:00', availability: manyClasses },
      { number: '22222', name: 'FARELESS EXP', departure_time: '09:00', arrival_time: '15:00', duration: '06:00', availability: [{ class: '3A', status: 'RAC 5' }] }
    ]
  }, { src: 'SC', dst: 'PUNE' });

  assert.equal(card.from, 'SC');
  assert.equal(card.to, 'PUNE');
  assert.equal(card.date, '2026-08-22');
  assert.equal(card.notice, 'n');
  assert.deepEqual([card.trains[0].number, card.trains[1].number, card.trains[2].number], ['11111', '22222', '00001']);
  assert.equal(card.src_code, 'SC');
  assert.equal(card.dst_code, 'PUNE');

  const rich = card.trains[0];
  assert.deepEqual(rich.dep, '08:00');
  assert.deepEqual(rich.duration, '06:00');
  assert.equal(rich.classes.length, 6, 'class list capped at 6');
  assert.equal(rich.classes[0].tone, 'ok');
  assert.equal(rich.classes[0].prediction, 92);
  assert.equal(rich.classes[1].fare, 501);
  assert.equal(rich.classes[2].tone, 'warn'); // RAC
  assert.equal(rich.classes[3].tone, 'warn'); // WL
  assert.equal(rich.classes[4].tone, 'bad'); // REGRET + available=false
  assert.equal(rich.classes[5].tone, 'bad'); // NOT AVAILABLE

  const bare = card.trains[1].classes[0];
  assert.equal(bare.tone, 'warn'); // RAC 5
  assert.equal('fare' in bare, false);
  assert.equal('prediction' in bare, false);

  const empty = gate.projectSeatAvailability({ trains: [{ number: '1', availability: [] }] });
  assert.deepEqual(empty.trains, [{ number: '1', name: undefined, dep: undefined, arr: undefined, duration: undefined, classes: [] }]);
  assert.equal(empty.from, null);
  assert.equal(empty.src_code, undefined);
});

// Fixture mirrors tools.rs ChartResponse.
test('chart_status projection keeps identity plus coach count only', () => {
  const card = gate.projectChartStatus({
    train_number: '12951',
    train_name: 'MMCT TEJAS RAJDHANI', // dropped by the server projection too
    journey_date: '2026-08-25',
    boarding_station: 'BCT',
    coaches: [{ coach: 'HA1', class: '3A' }, { coach: 'B5', class: 'SL' }],
    data_source: 'IRCTC',
    notice: null
  });
  assert.deepEqual(card, {
    train_number: '12951',
    journey_date: '2026-08-25',
    boarding_station: 'BCT',
    coach_count: 2,
    data_source: 'IRCTC',
    notice: null
  });
  assert.equal('train_name' in card, false);
  assert.equal('coaches' in card, false);

  assert.equal(gate.projectChartStatus({}).coach_count, null);
  assert.ok(gate.PROJECTORS.chart_status === gate.projectChartStatus);
});

test('nextActionsFor mirrors the server chip rules and dedupes', () => {
  const chips = gate.nextActionsFor('trains_between', {
    src_code: 'SC',
    dst_code: 'PUNE',
    trains: [{ number: '17013' }]
  });
  assert.deepEqual(chips[0], { label: 'Track 17013', prompt: 'live status of 17013' });
  assert.equal(chips.length <= 4, true);

  const live = gate.nextActionsFor('live_status', { train_number: '12951' });
  assert.ok(live.some((c) => c.label === 'Route of 12951'));
  assert.ok(live.some((c) => c.label === 'Avg delay 12951'));
});

test('nextActionsFor chains seat->chart and chart->track like the server', () => {
  const seat = gate.nextActionsFor('seat_availability', {
    src_code: 'SC',
    dst_code: 'PUNE',
    trains: [{ number: '11111', classes: [] }, { number: '22222', classes: [] }]
  });
  assert.ok(seat.some((c) => c.label === 'Chart 11111' && c.prompt === 'chart status of train 11111'));
  // no derivable train -> no chip
  assert.equal(gate.nextActionsFor('seat_availability', { trains: [] }).length, 0);

  const chart = gate.nextActionsFor('chart_status', { train_number: '12951' });
  assert.deepEqual(chart[0], { label: 'Track 12951', prompt: 'live status of 12951' });

  assert.ok(gate.PROJECTORS.seat_availability && gate.PROJECTORS.chart_status);
});
