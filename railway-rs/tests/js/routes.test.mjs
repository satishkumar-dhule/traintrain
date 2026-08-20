/* routes.test.mjs - unit tests for static/routes.js (pure route table).
   Runs with the built-in Node test runner: `node --test tests/js/`.
   Kept outside static/ so the test files are never served by the app. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const Routes = require('../../static/routes.js');

test('home routes', () => {
  assert.deepEqual(Routes.parse(''), { section: 'home', view: null, params: {} });
  assert.deepEqual(Routes.parse('#/'), { section: 'home', view: null, params: {} });
  assert.deepEqual(Routes.parse('#/'), Routes.parse(''));
  assert.equal(Routes.href({ section: 'home' }), '#/');
});

test('pnr legacy redirect routes to train with _pnr param', () => {
  assert.deepEqual(Routes.parse('#/pnr/2498761234'), {
    section: 'train', view: null, params: { _pnr: '2498761234' },
  });
  assert.deepEqual(Routes.parse('#/pnr'), { section: 'train', view: null, params: {} });
  assert.equal(Routes.parse('#/pnr/123'), null);
  assert.equal(Routes.parse('#/pnr/abc1234567'), null);
});

test('train route defaults to spot view', () => {
  const r = Routes.parse('#/train/12559');
  assert.equal(r.section, 'train');
  assert.equal(r.view, 'spot');
  assert.deepEqual(r.params, { train: '12559' });
});

test('train route with explicit views', () => {
  for (const v of ['spot', 'schedule', 'map', 'delay', 'exceptions', 'journey']) {
    const r = Routes.parse(`#/train/${12559}/${v}`);
    assert.equal(r.section, 'train');
    assert.equal(r.view, v);
  }
});

test('train route rejects unknown view and all-zero train', () => {
  assert.equal(Routes.parse('#/train/12559/bogus'), null);
  assert.equal(Routes.parse('#/train/00000'), null);
  assert.equal(Routes.parse('#/train/12x59'), null);
});

test('href drops the default view for canonical deep links', () => {
  assert.equal(Routes.href({ section: 'train', params: { train: '12559' }, view: 'spot' }), '#/train/12559');
  assert.equal(Routes.href({ section: 'train', params: { train: '12559' }, view: 'schedule' }), '#/train/12559/schedule');
  assert.equal(Routes.href({ section: 'train', params: { train: '12559' } }), '#/train/12559');
  assert.equal(Routes.href({ section: 'train', params: { train: '12559' }, view: 'bogus' }), '#/train/12559');
});

test('station route normalizes to uppercase and defaults to live view', () => {
  const r = Routes.parse('#/station/ndls');
  assert.equal(r.section, 'station');
  assert.equal(r.view, 'live');
  assert.deepEqual(r.params, { station: 'NDLS' });
  assert.equal(Routes.parse('#/station/NDLS/tt').view, 'tt');
  assert.equal(Routes.parse('#/station/heritage').view, 'heritage');
  assert.equal(Routes.parse('#/station/parcel').view, 'parcel');
  assert.equal(Routes.parse('#/station/NDLS/bogus'), null);
  assert.equal(Routes.parse('#/station/X'), null);
  assert.equal(Routes.href({ section: 'station', params: { station: 'ndls' }, view: 'live' }), '#/station/NDLS');
  assert.equal(Routes.href({ section: 'station', params: { station: 'ndls' }, view: 'tt' }), '#/station/NDLS/tt');
});

test('plan route defaults to trains view', () => {
  const r = Routes.parse('#/plan/NDLS/BSB');
  assert.equal(r.section, 'plan');
  assert.equal(r.view, 'trains');
  assert.deepEqual(r.params, { src: 'NDLS', dst: 'BSB' });
  assert.equal(Routes.parse('#/plan/NDLS/BSB/availability').view, 'availability');
  assert.equal(Routes.parse('#/plan/NDLS/BSB/chart').view, 'chart');
  assert.equal(Routes.parse('#/plan/NDLS/BSB/bogus'), null);
  assert.equal(Routes.parse('#/plan/NDLS'), null);
  assert.equal(Routes.href({ section: 'plan', params: { src: 'NDLS', dst: 'BSB' }, view: 'trains' }), '#/plan/NDLS/BSB');
  assert.equal(Routes.href({ section: 'plan', params: { src: 'NDLS', dst: 'BSB' }, view: 'chart' }), '#/plan/NDLS/BSB/chart');
});

test('plan route carries an optional journey date', () => {
  assert.deepEqual(Routes.parse('#/plan/NDLS/BSB/2026-08-20'), {
    section: 'plan', view: 'trains', params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20' },
  });
  assert.deepEqual(Routes.parse('#/plan/NDLS/BSB/availability/2026-08-20'), {
    section: 'plan', view: 'availability', params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20' },
  });
  assert.equal(Routes.parse('#/plan/NDLS/BSB/2026-13-99'), null);
  assert.equal(Routes.parse('#/plan/NDLS/BSB/2026-08-32'), null);
  assert.equal(Routes.parse('#/plan/NDLS/BSB/20260820'), null);
  assert.equal(Routes.href({ section: 'plan', params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20' }, view: 'trains' }), '#/plan/NDLS/BSB/2026-08-20');
  assert.equal(Routes.href({ section: 'plan', params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20' }, view: 'availability' }), '#/plan/NDLS/BSB/availability/2026-08-20');
  assert.equal(Routes.href({ section: 'plan', params: { src: 'NDLS', dst: 'BSB', date: 'not-a-date' } }), '#/plan/NDLS/BSB');
});

test('plan route carries class / flex / berth search extras', () => {
  assert.deepEqual(Routes.parse('#/plan/NDLS/BSB/2026-08-20/class/3A/flex'), {
    section: 'plan', view: 'trains',
    params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20', class: '3A', flex: '1' },
  });
  assert.deepEqual(Routes.parse('#/plan/NDLS/BSB/availability/class/sl/berth'), {
    section: 'plan', view: 'availability',
    params: { src: 'NDLS', dst: 'BSB', class: 'SL', berth: '1' },
  });
  assert.equal(Routes.parse('#/plan/NDLS/BSB/class/3A').view, 'trains');
  assert.equal(
    Routes.href({ section: 'plan', params: { src: 'NDLS', dst: 'BSB', class: '3A', flex: '1' } }),
    '#/plan/NDLS/BSB/class/3A/flex');
  assert.equal(
    Routes.href({ section: 'plan', params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20', class: '2A', berth: '1' }, view: 'availability' }),
    '#/plan/NDLS/BSB/availability/2026-08-20/class/2A/berth');
  const round = Routes.parse(Routes.href({ section: 'plan', params: { src: 'ndls', dst: 'bsb', class: '3e', flex: '1' } }));
  assert.equal(round.params.class, '3E');
  assert.equal(round.params.flex, '1');
});

test('system route defaults to observability view', () => {
  assert.deepEqual(Routes.parse('#/system'), { section: 'system', view: 'observability', params: {} });
  assert.equal(Routes.parse('#/system/settings').view, 'settings');
  assert.equal(Routes.parse('#/system/debug').view, 'debug');
  assert.equal(Routes.parse('#/system/bogus'), null);
  assert.equal(Routes.href({ section: 'system' }), '#/system');
  assert.equal(Routes.href({ section: 'system', view: 'debug' }), '#/system/debug');
});

test('more legacy paths redirect into the current sections', () => {
  assert.deepEqual(Routes.parse('#/more/observability'), { section: 'system', view: 'observability', params: {} });
  assert.deepEqual(Routes.parse('#/more/debug'), { section: 'system', view: 'debug', params: {} });
  assert.deepEqual(Routes.parse('#/more/system'), { section: 'system', view: 'settings', params: {} });
  assert.deepEqual(Routes.parse('#/more/heritage'), { section: 'station', view: 'heritage', params: {} });
  assert.deepEqual(Routes.parse('#/more/parcel'), { section: 'station', view: 'parcel', params: {} });
  assert.equal(Routes.parse('#/more/bogus'), null);
});

test('section-level routes default to the section view with empty params', () => {
  assert.deepEqual(Routes.parse('#/train'), { section: 'train', view: 'spot', params: {} });
  assert.deepEqual(Routes.parse('#/station'), { section: 'station', view: 'live', params: {} });
  assert.deepEqual(Routes.parse('#/plan'), { section: 'plan', view: 'trains', params: {} });
});

test('href collapses entity-less sections to section-level routes', () => {
  assert.equal(Routes.href({ section: 'train' }), '#/train');
  assert.equal(Routes.href({ section: 'train', view: 'schedule' }), '#/train');
  assert.equal(Routes.href({ section: 'station' }), '#/station');
  assert.equal(Routes.href({ section: 'plan' }), '#/plan');
});

test('unknown hashes parse to null (app redirects to home)', () => {
  assert.equal(Routes.parse('#/garbage'), null);
  assert.equal(Routes.parse('#/trains/12559'), null);
  assert.equal(Routes.parse('garbage'), null);
});

test('parse(href(route)) round-trips', () => {
  const samples = [
    { section: 'home' },
    { section: 'train', params: { train: '12559' }, view: 'schedule' },
    { section: 'station', params: { station: 'NDLS' }, view: 'tt' },
    { section: 'plan', params: { src: 'NDLS', dst: 'BSB' }, view: 'availability' },
    { section: 'plan', params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20' }, view: 'trains' },
    { section: 'plan', params: { src: 'NDLS', dst: 'BSB', date: '2026-08-20' }, view: 'chart' },
    { section: 'system', view: 'debug' },
    { section: 'system' },
    { section: 'train' },
  ];
  for (const s of samples) {
    const views = Routes.viewsFor(s.section);
    const defaultView = Routes.SECTIONS[s.section].defaultView;
    const view = views.length ? (s.view || defaultView) : null;
    assert.deepEqual(Routes.parse(Routes.href(s)), {
      section: s.section,
      view,
      params: s.params || {},
    });
  }
});

test('section registry', () => {
  assert.deepEqual(Routes.NAV_ORDER, ['home', 'train', 'station', 'plan', 'system']);
  assert.deepEqual(Routes.viewsFor('train'), ['spot', 'schedule', 'map', 'delay', 'exceptions', 'journey']);
  assert.deepEqual(Routes.viewsFor('station'), ['live', 'tt', 'heritage', 'parcel']);
  assert.deepEqual(Routes.viewsFor('plan'), ['trains', 'availability', 'chart']);
  assert.deepEqual(Routes.viewsFor('system'), ['observability', 'settings', 'debug']);
  assert.equal(Routes.isValidView('train', 'map'), true);
  assert.equal(Routes.isValidView('train', 'bogus'), false);
  assert.deepEqual(Routes.viewsFor('nope'), []);
});

/* ---------- canonical boot (browser history) ---------- */

import { JSDOM } from 'jsdom';

function todayIso() {
  const d = new Date();
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}

async function bootRouter(hash) {
  const dom = new JSDOM('<!doctype html><html><body></body></html>', { url: 'http://localhost/' });
  const win = dom.window;
  win.location.hash = hash;
  const prevWindow = global.window;
  global.window = win;
  return { dom, win, prevWindow };
}

test('boot canonicalizes a dateless plan deep link with one history push', async () => {
  const { dom, win, prevWindow } = await bootRouter('#/plan/NDLS/BSB');
  try {
    const len0 = win.history.length;
    await Routes.boot();
    assert.equal(win.location.hash, `#/plan/NDLS/BSB/${todayIso()}`);
    assert.equal(win.history.length, len0 + 1);
  } finally {
    global.window = prevWindow;
    dom.window.close();
  }
});

test('pnr deep links are never canonicalized (they carry the _pnr redirect param)', async () => {
  const { dom, win, prevWindow } = await bootRouter('#/pnr/2498761234');
  try {
    assert.equal(Routes.canonical('#/pnr/2498761234'), null, 'canonical must leave pnr routes untouched');
    const len0 = win.history.length;
    await Routes.boot();
    assert.equal(win.location.hash, '#/pnr/2498761234', 'boot must not rewrite a pnr hash');
    assert.equal(win.history.length, len0, 'boot must not add a history entry for a pnr hash');
  } finally {
    global.window = prevWindow;
    dom.window.close();
  }
});

test('boot with an already-canonical hash adds no history entry (no loop)', async () => {
  const { dom, win, prevWindow } = await bootRouter(`#/plan/NDLS/BSB/${todayIso()}`);
  try {
    const len0 = win.history.length;
    await Routes.boot();
    await Routes.boot();
    assert.equal(win.location.hash, `#/plan/NDLS/BSB/${todayIso()}`);
    assert.equal(win.history.length, len0);
  } finally {
    global.window = prevWindow;
    dom.window.close();
  }
});

test('hashchange onto a dateless plan hash is rewritten in place without growing history', async () => {
  const { dom, win, prevWindow } = await bootRouter(`#/plan/NDLS/BSB/${todayIso()}`);
  try {
    await Routes.boot();
    win.location.hash = '#/plan/NDLS/BSB';
    const lenAfterNav = win.history.length;
    await new Promise((r) => setTimeout(r, 15));
    assert.equal(win.location.hash, `#/plan/NDLS/BSB/${todayIso()}`);
    assert.equal(win.history.length, lenAfterNav, 'canonicalization must not add a history entry');
  } finally {
    global.window = prevWindow;
    dom.window.close();
  }
});