/* gate.test.mjs - unit tests for the local-first intent router.
   Pins the classification contract (trivial/replay/tool/llm), the plan
   shapes, and that client-side projections mirror the server's ai_chat
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

test('ambiguous questions fall through to llm', () => {
  for (const q of ['best food on trains?', 'availability from SC to PUNE tomorrow', 'plan a 3 day trip', '']) {
    assert.equal(gate.classify(q).kind, 'llm', q);
  }
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

test('route/schedule and average-delay intents map to their endpoints', () => {
  const route = gate.classify('route of 12951');
  assert.equal(route.plan.cardKind, 'train_schedule');
  assert.equal(route.plan.url, '/rail-api/schedule');

  const delay = gate.classify('average delay of 12951');
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
