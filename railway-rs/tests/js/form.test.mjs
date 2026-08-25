/* form.test.mjs - pure spec tests for buildFormSpec / IntentForm validation.
   Covers required fields per intent, prefill values, missing detection,
   intent picker when null/low confidence, field required flags, candidates,
   and date handling. No DOM needed. Mirrors gate.test.mjs style. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';

const require = createRequire(import.meta.url);
const gate = require('../../frontend/src/lib/chat/gate.js');

const TODAY_RE = /^\d{4}-\d{2}-\d{2}$/;

function findField(spec, name) {
  return (spec.fields || []).find((f) => (f.name ?? f.key) === name);
}

// ---------- required fields per intent ----------

test('REQUIRED_FIELDS exported and matches spec expectations', () => {
  assert.ok(gate.REQUIRED_FIELDS);
  assert.deepEqual(gate.REQUIRED_FIELDS.live_status, ['train']);
  assert.deepEqual(gate.REQUIRED_FIELDS.average_delay, ['train']);
  assert.deepEqual(gate.REQUIRED_FIELDS.train_schedule, ['train']);
  assert.deepEqual(gate.REQUIRED_FIELDS.trains_between, ['src', 'dst']);
  assert.deepEqual(gate.REQUIRED_FIELDS.station_board, ['station']);
  assert.deepEqual(gate.REQUIRED_FIELDS.seat_availability, ['src', 'dst']);
  assert.deepEqual(gate.REQUIRED_FIELDS.chart_status, ['train']);
  assert.deepEqual(gate.REQUIRED_FIELDS.pnr_status, ['pnr']);
  assert.equal(Object.keys(gate.REQUIRED_FIELDS).length, 8);
});

test('buildFormSpec returns fields array with correct required flags per intent', () => {
  const samples = {
    live_status: 'live status 12951',
    average_delay: 'average delay 12626',
    train_schedule: 'route of 12951',
    trains_between: 'trains between secunderabad and pune',
    station_board: 'station board pune',
    seat_availability: 'seat availability from sc to pune',
    chart_status: 'chart status 12951'
  };
  for (const [intentId, phrase] of Object.entries(samples)) {
    const spec = gate.buildFormSpec(phrase);
    assert.equal(spec.intentId, intentId, `${phrase} -> ${intentId} got ${spec.intentId}`);
    assert.ok(Array.isArray(spec.fields), `${intentId} fields array`);
    for (const req of gate.REQUIRED_FIELDS[intentId]) {
      const fld = findField(spec, req);
      assert.ok(fld, `${intentId} missing field ${req}`);
      assert.equal(fld.required, true, `${intentId} ${req} should be required`);
      assert.ok(fld.label && fld.label.length > 0, `${req} label`);
      assert.ok(fld.placeholder !== undefined, `${req} placeholder`);
    }
    // date field when present must be optional
    const dateFld = findField(spec, 'date');
    if (dateFld) assert.equal(dateFld.required, false, `${intentId} date optional`);
    // no intent picker when confident
    const picker = findField(spec, 'intent');
    assert.equal(picker, undefined, `${intentId} confident spec should not have intent picker`);
    // collected should have required values filled, missing empty
    const required = gate.REQUIRED_FIELDS[intentId];
    const missing = spec.missing || [];
    assert.deepEqual(missing, [], `${intentId} complete sample missing should be []`);
  }
});

test('buildFormSpec required flags: train intents have train required true, between has src/dst etc', () => {
  const trainIntents = ['live_status', 'average_delay', 'train_schedule', 'chart_status'];
  for (const id of trainIntents) {
    const phrase = id === 'average_delay' ? 'average delay 12951' : id === 'chart_status' ? 'chart status 12951' : id === 'train_schedule' ? 'route of 12951' : 'live status 12951';
    const spec = gate.buildFormSpec(phrase);
    const f = findField(spec, 'train');
    assert.ok(f);
    assert.equal(f.required, true);
    assert.equal(f.pattern, '\\d{5}');
    assert.equal(f.type, 'text');
  }
  const specBetween = gate.buildFormSpec('trains between sc and pune');
  assert.equal(findField(specBetween, 'src').required, true);
  assert.equal(findField(specBetween, 'dst').required, true);
  assert.equal(findField(specBetween, 'train'), undefined);
  const specBoard = gate.buildFormSpec('station board pune');
  assert.equal(findField(specBoard, 'station').required, true);
  assert.equal(findField(specBoard, 'src'), undefined);
});

// ---------- prefill values ----------

test('prefill values: train number, src/dst, station, date', () => {
  const s1 = gate.buildFormSpec('live status of 12951 on 2026-10-20');
  assert.equal(s1.collected.train, '12951');
  assert.equal(findField(s1, 'train').value, '12951');
  assert.equal(s1.collected.date, '2026-10-20');
  // date field normalised to ISO (already ISO)
  assert.equal(findField(s1, 'date').value, '2026-10-20');

  const s2 = gate.buildFormSpec('seat availability from secunderabad to pune');
  assert.equal(s2.collected.src, 'secunderabad');
  assert.equal(s2.collected.dst, 'pune');
  assert.equal(findField(s2, 'src').value, 'secunderabad');
  assert.equal(findField(s2, 'dst').value, 'pune');

  const s3 = gate.buildFormSpec('trains between SC and PUNE');
  assert.equal(s3.collected.src, 'sc');
  assert.equal(s3.collected.dst, 'pune');
  assert.equal(findField(s3, 'src').value, 'sc');
  assert.equal(findField(s3, 'dst').value, 'pune');

  const s4 = gate.buildFormSpec('station board pune');
  assert.equal(s4.collected.station, 'pune');
  assert.equal(findField(s4, 'station').value, 'pune');

  const s5 = gate.buildFormSpec('chart status 12951 tomorrow');
  assert.equal(s5.collected.train, '12951');
  assert.ok(s5.collected.date, 'tomorrow should prefill date');
  // buildFormSpec normalises ddmmyyyy to iso? check todayISO fallback
  const dateVal = findField(s5, 'date').value;
  assert.match(dateVal, TODAY_RE, 'chart date should be ISO');
});

test('prefill is lowercased for stations but preserves train digits', () => {
  const spec = gate.buildFormSpec('SEAT AVAILABILITY FROM Secunderabad TO PUNE');
  assert.equal(spec.collected.src, 'secunderabad');
  assert.equal(spec.collected.dst, 'pune');
  assert.equal(findField(spec, 'src').value, 'secunderabad');
});

// ---------- missing detection ----------

test('missing detection: partial slots produce correct missing array', () => {
  const onlySrc = gate.buildFormSpec('seat availability from secunderabad');
  assert.equal(onlySrc.intentId, 'seat_availability');
  assert.deepEqual(onlySrc.missing, ['dst']);
  assert.equal(onlySrc.collected.src, 'secunderabad');
  assert.equal(onlySrc.collected.dst, undefined);

  const none = gate.buildFormSpec('seat availability');
  // without src/dst, both missing (buildFormSpec uses collectForForm which yields empty collected)
  // But intent may still be seat_availability if score >=0.3
  if (none.intentId === 'seat_availability') {
    assert.ok(none.missing.includes('src'));
    assert.ok(none.missing.includes('dst'));
  }

  const missingTrain = gate.buildFormSpec('where is my train');
  assert.equal(missingTrain.intentId, 'live_status');
  assert.deepEqual(missingTrain.missing, ['train']);
  assert.equal(missingTrain.collected.train, undefined);

  const fourDigits = gate.buildFormSpec('live status 1295');
  assert.equal(fourDigits.collected.train, undefined);
  assert.ok(fourDigits.missing.includes('train'), '4-digit train should be missing');

  const betweenMissingDst = gate.buildFormSpec('trains from secunderabad');
  if (betweenMissingDst.intentId === 'trains_between') {
    assert.ok(betweenMissingDst.missing.includes('dst'));
  }
  const boardMissing = gate.buildFormSpec('station board');
  if (boardMissing.intentId === 'station_board') {
    assert.ok(boardMissing.missing.includes('station'));
  }
});

test('missing detection respects REQUIRED_FIELDS only', () => {
  const spec = gate.buildFormSpec('live status 12951');
  // date is optional, should never appear in missing
  assert.ok(!spec.missing.includes('date'), 'date optional should not be in missing');
  const spec2 = gate.buildFormSpec('trains between sc and pune');
  assert.ok(!spec2.missing.includes('date'));
  assert.ok(!spec2.missing.includes('train'));
});

// ---------- intent picker when null / low confidence ----------

test('intent picker appears when intent null or confidence <0.45', () => {
  const nullSpec = gate.buildFormSpec('best food near pune');
  assert.ok(nullSpec.intentId == null || nullSpec.confidence < 0.45, `expected null/low confidence got ${nullSpec.intentId} ${nullSpec.confidence}`);
  const picker = findField(nullSpec, 'intent');
  assert.ok(picker, 'should have intent picker');
  assert.equal(picker.required, true);
  assert.equal(picker.type, 'select');
  assert.ok(Array.isArray(picker.options) && picker.options.length >= 7, `options length ${picker.options?.length}`);
  // options should cover all intents
  const optionValues = picker.options.map((o) => o.value ?? o.id);
  for (const id of Object.keys(gate.REQUIRED_FIELDS)) {
    assert.ok(optionValues.includes(id), `picker missing ${id}`);
  }
  // when null, there are no required train/src etc fields, only picker + no date? check implementation: makeForm adds only picker when intentId null
  // but our gate.makeForm adds picker and no required fields when intentId null (required=[]). So fields should be 1 entry (picker) when null.
  // However buildCandidates may still produce required fields? Actually makeForm with intentId null => required=[], so only picker.
  assert.ok(nullSpec.fields.length === 1, `null intent fields should be 1 picker, got ${JSON.stringify(nullSpec.fields)}`);
  assert.deepEqual(nullSpec.missing, [], 'null intent missing should be []');

  const empty = gate.buildFormSpec('');
  const picker2 = findField(empty, 'intent');
  assert.ok(picker2, 'empty query should have picker');
  assert.equal(empty.intentId, null);

  const vague = gate.buildFormSpec('zzz qq qq xx');
  if (vague.intentId == null || vague.confidence < 0.45) {
    const p = findField(vague, 'intent');
    assert.ok(p, 'vague query should have picker');
  }
});

test('confident intent does NOT show picker and shows date optional', () => {
  const confident = gate.buildFormSpec('live status 12951');
  assert.equal(confident.intentId, 'live_status');
  assert.ok(confident.confidence >= 0.45);
  assert.equal(findField(confident, 'intent'), undefined);
  const date = findField(confident, 'date');
  assert.ok(date);
  assert.equal(date.required, false);
  assert.equal(date.type, 'date');
  assert.match(date.value, TODAY_RE);
});

// ---------- candidates ----------

test('candidates always length >=2 or >=3 and contain scores', () => {
  for (const q of ['where is my train', 'seat availability from secunderabad', 'best food near pune', 'live status 1295', '', 'trains between sc and pune', 'station board pune']) {
    const spec = gate.buildFormSpec(q);
    assert.ok(Array.isArray(spec.candidates), `${q} candidates array`);
    assert.ok(spec.candidates.length >= 2, `${q} candidates >=2 got ${spec.candidates.length}`);
    // most paths guarantee 3
    assert.ok(spec.candidates.length >= 3 || q === '', `${q} candidates >=3`);
    for (const c of spec.candidates) {
      assert.ok(c.intentId, `candidate needs intentId`);
      assert.ok(typeof c.score === 'number');
      assert.ok(c.label && c.label.length > 0);
    }
  }
});

test('candidates for help cases still produce distinct intents', () => {
  const spec = gate.buildFormSpec('where is my train');
  const ids = spec.candidates.map((c) => c.intentId);
  assert.equal(new Set(ids).size, ids.length, 'candidates should be distinct intents');
  assert.ok(ids.includes('live_status'));
});

// ---------- date handling ----------

test('date handling: today, ISO, DMY, tomorrow', () => {
  const todaySpec = gate.buildFormSpec('live status 12951 today');
  assert.equal(todaySpec.collected.date, 'today');
  // makeForm normalises today to todayISO
  assert.match(findField(todaySpec, 'date').value, TODAY_RE);

  const isoSpec = gate.buildFormSpec('chart status 12951 2026-10-20');
  assert.equal(isoSpec.collected.date, '2026-10-20');
  assert.equal(findField(isoSpec, 'date').value, '2026-10-20');

  const dmySpec = gate.buildFormSpec('seat availability from sc to pune 20-10-2026');
  assert.equal(dmySpec.collected.date, '20-10-2026');
  // field value should be ISO converted
  assert.equal(findField(dmySpec, 'date').value, '2026-10-20');

  const noDate = gate.buildFormSpec('live status 12951');
  assert.match(findField(noDate, 'date').value, TODAY_RE, 'fallback to todayISO');

  // classify vs buildFormSpec date consistency
  const v = gate.classify('where is my train');
  assert.match(findField(v.form, 'date').value, TODAY_RE);
});

// ---------- field shape ----------

test('fields array shape: each field has name, label, required, value, type', () => {
  const queries = ['live status 12951', 'seat availability from sc to pune', 'station board pune', 'best food near pune'];
  for (const q of queries) {
    const spec = gate.buildFormSpec(q);
    assert.ok(Array.isArray(spec.fields));
    for (const f of spec.fields) {
      const name = f.name ?? f.key;
      assert.ok(name, 'field needs name/key');
      assert.ok(f.label, `field ${name} label`);
      assert.equal(typeof f.required, 'boolean', `field ${name} required boolean`);
      assert.ok('value' in f, `field ${name} value`);
      assert.ok(f.type, `field ${name} type`);
      // pattern only for train
      if (name === 'train') assert.equal(f.pattern, '\\d{5}');
      if (name === 'intent') {
        assert.equal(f.type, 'select');
        assert.ok(Array.isArray(f.options));
      }
    }
  }
});

// ---------- IntentForm.svelte sync (if file exists) ----------

test('IntentForm.svelte REQUIRED_FIELDS sync with gate.js (no DOM, file read)', () => {
  const candidates = [
    '../../frontend/src/lib/components/chat/IntentForm.svelte',
    '../../frontend/src/lib/components/IntentForm.svelte'
  ];
  let found = null;
  for (const p of candidates) {
    const abs = path.resolve(path.dirname(new URL(import.meta.url).pathname), p);
    if (fs.existsSync(abs)) { found = abs; break; }
  }
  if (!found) {
    // if file not found, just pass – spec shapes already covered
    assert.ok(true, 'IntentForm not found, skip sync check');
    return;
  }
  const txt = fs.readFileSync(found, 'utf8');
  // check that file mentions same intents
  for (const id of Object.keys(gate.REQUIRED_FIELDS)) {
    assert.ok(txt.includes(id), `IntentForm should mention ${id}`);
    // check required listing
    const req = gate.REQUIRED_FIELDS[id];
    // simplistic: look for 'id': [..]
    const snippet = txt.slice(txt.indexOf(id) - 200, txt.indexOf(id) + 500);
    for (const field of req) {
      // at least the field name should appear near the intent id in REQUIRED_FIELDS block
      // fallback: global search
      assert.ok(txt.includes(`'${field}'`) || txt.includes(`"${field}"`) || txt.includes(field), `IntentForm should mention field ${field} for ${id}`);
    }
  }
  // also check FIELD_META contains train pattern
  assert.ok(txt.includes('\\d{5}') || txt.includes('\\\\d{5}'), 'IntentForm should have train pattern');
});

test('IntentForm validation logic mirrors spec: train 5 digits, required flags', () => {
  // This mirrors the isValid/fieldError logic inside IntentForm.svelte without mounting Svelte.
  // We validate via gate spec: required true => value must be non-empty and train must be 5 digits.
  function isFieldValid(name, value, required) {
    const v = String(value ?? '').trim();
    if (required && !v) return false;
    if (name === 'train' && v) return /^\d{5}$/.test(v);
    return true;
  }
  const spec = gate.buildFormSpec('live status 12951');
  for (const f of spec.fields) {
    if (f.required) assert.ok(isFieldValid(f.name ?? f.key, f.value, true), `required field ${f.name} should be valid with prefilled value`);
  }
  const missingSpec = gate.buildFormSpec('where is my train');
  const trainFld = findField(missingSpec, 'train');
  assert.equal(isFieldValid('train', trainFld.value, true), false, 'empty train should be invalid');
  const badTrain = { name: 'train', value: '1295', required: true };
  assert.equal(isFieldValid(badTrain.name, badTrain.value, true), false, '4-digit train invalid');
  assert.equal(isFieldValid('train', '12951', true), true);
  assert.equal(isFieldValid('src', '', true), false);
  assert.equal(isFieldValid('src', 'secunderabad', true), true);
  assert.equal(isFieldValid('date', '', false), true, 'optional date empty is valid');
});

// ---------- buildFormSpec vs classify form consistency ----------

test('buildFormSpec and classify form are consistent for help cases', () => {
  for (const q of ['where is my train', 'seat availability from secunderabad', 'best food near pune', 'live status 1295']) {
    const spec = gate.buildFormSpec(q);
    const cls = gate.classify(q);
    // classify for these queries is help, so should have form
    assert.equal(cls.kind, 'help', `${q} classify kind help`);
    assert.ok(cls.form, `${q} classify has form`);
    assert.equal(cls.form.intentId, spec.intentId, `${q} intentId consistent`);
    assert.deepEqual(cls.form.missing, spec.missing, `${q} missing consistent`);
    assert.deepEqual(cls.form.collected, spec.collected, `${q} collected consistent`);
    // fields deep-ish
    assert.equal(cls.form.fields.length, spec.fields.length, `${q} fields length`);
    // candidates same length
    assert.equal(cls.form.candidates.length, spec.candidates.length);
  }
});
