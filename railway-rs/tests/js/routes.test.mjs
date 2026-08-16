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

test('pnr route', () => {
  assert.deepEqual(Routes.parse('#/pnr/2498761234'), {
    section: 'pnr', view: null, params: { pnr: '2498761234' },
  });
  assert.equal(Routes.href({ section: 'pnr', params: { pnr: '2498761234' } }), '#/pnr/2498761234');
  assert.equal(Routes.parse('#/pnr/123'), null);
  assert.equal(Routes.parse('#/pnr/abc1234567'), null);
});

test('track route defaults to spot view', () => {
  const r = Routes.parse('#/train/12559');
  assert.equal(r.section, 'track');
  assert.equal(r.view, 'spot');
  assert.deepEqual(r.params, { train: '12559' });
});

test('track route with explicit views', () => {
  for (const v of ['spot', 'schedule', 'map', 'delay', 'exceptions', 'journey']) {
    const r = Routes.parse(`#/train/12559/${v}`);
    assert.equal(r.section, 'track');
    assert.equal(r.view, v);
  }
});

test('track route rejects unknown view and all-zero train', () => {
  assert.equal(Routes.parse('#/train/12559/bogus'), null);
  assert.equal(Routes.parse('#/train/00000'), null);
  assert.equal(Routes.parse('#/train/12x59'), null);
});

test('href drops the default view for canonical deep links', () => {
  assert.equal(Routes.href({ section: 'track', params: { train: '12559' }, view: 'spot' }), '#/train/12559');
  assert.equal(Routes.href({ section: 'track', params: { train: '12559' }, view: 'schedule' }), '#/train/12559/schedule');
  assert.equal(Routes.href({ section: 'track', params: { train: '12559' } }), '#/train/12559');
  assert.equal(Routes.href({ section: 'track', params: { train: '12559' }, view: 'bogus' }), '#/train/12559');
});

test('station route normalizes to uppercase and defaults to live view', () => {
  const r = Routes.parse('#/station/ndls');
  assert.equal(r.section, 'station');
  assert.equal(r.view, 'live');
  assert.deepEqual(r.params, { station: 'NDLS' });
  assert.equal(Routes.parse('#/station/NDLS/tt').view, 'tt');
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

test('more route', () => {
  for (const v of ['heritage', 'parcel', 'stations', 'system', 'observability', 'debug']) {
    const r = Routes.parse(`#/more/${v}`);
    assert.equal(r.section, 'more');
    assert.equal(r.view, v);
  }
  assert.equal(Routes.parse('#/more/bogus'), null);
  assert.equal(Routes.href({ section: 'more', view: 'debug' }), '#/more/debug');
  assert.equal(Routes.href({ section: 'more', view: 'bogus' }), '#/more');
  assert.equal(Routes.href({ section: 'more' }), '#/more');
});

test('more hub: #/more resolves to the hub (no view)', () => {
  assert.deepEqual(Routes.parse('#/more'), { section: 'more', view: null, params: {} });
});

test('section-level routes default to the section view with empty params', () => {
  assert.deepEqual(Routes.parse('#/track'), { section: 'track', view: 'spot', params: {} });
  assert.deepEqual(Routes.parse('#/station'), { section: 'station', view: 'live', params: {} });
  assert.deepEqual(Routes.parse('#/plan'), { section: 'plan', view: 'trains', params: {} });
  assert.deepEqual(Routes.parse('#/pnr'), { section: 'pnr', view: null, params: {} });
});

test('href collapses entity-less sections to section-level routes', () => {
  assert.equal(Routes.href({ section: 'track' }), '#/track');
  assert.equal(Routes.href({ section: 'track', view: 'schedule' }), '#/track');
  assert.equal(Routes.href({ section: 'station' }), '#/station');
  assert.equal(Routes.href({ section: 'plan' }), '#/plan');
  assert.equal(Routes.href({ section: 'pnr' }), '#/pnr');
  assert.equal(Routes.href({ section: 'pnr', params: {} }), '#/pnr');
});

test('unknown hashes parse to null (app redirects to home)', () => {
  assert.equal(Routes.parse('#/garbage'), null);
  assert.equal(Routes.parse('#/trains/12559'), null);
  assert.equal(Routes.parse('garbage'), null);
});

test('parse(href(route)) round-trips', () => {
  const samples = [
    { section: 'home' },
    { section: 'pnr', params: { pnr: '2498761234' } },
    { section: 'track', params: { train: '12559' }, view: 'schedule' },
    { section: 'station', params: { station: 'NDLS' }, view: 'tt' },
    { section: 'plan', params: { src: 'NDLS', dst: 'BSB' }, view: 'availability' },
    { section: 'more', view: 'debug' },
    { section: 'more' },
    { section: 'track' },
    { section: 'pnr' },
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
  assert.deepEqual(Routes.NAV_ORDER, ['home', 'track', 'station', 'plan', 'pnr']);
  assert.deepEqual(Routes.viewsFor('track'), ['spot', 'schedule', 'map', 'delay', 'exceptions', 'journey']);
  assert.deepEqual(Routes.viewsFor('station'), ['live', 'tt']);
  assert.deepEqual(Routes.viewsFor('plan'), ['trains', 'availability', 'chart']);
  assert.deepEqual(Routes.viewsFor('more'), ['heritage', 'parcel', 'stations', 'system', 'observability', 'debug']);
  assert.equal(Routes.isValidView('track', 'map'), true);
  assert.equal(Routes.isValidView('track', 'bogus'), false);
  assert.deepEqual(Routes.viewsFor('nope'), []);
});
