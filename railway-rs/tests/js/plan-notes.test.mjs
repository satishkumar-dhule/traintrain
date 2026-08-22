/* plan-notes.test.mjs - wording regression tests for the Plan trains view.
   Mimics the dom-smoke.test.mjs harness (fake DOM, real static scripts,
   stubbed fetch). Guards the honest note text: with "Flexible with date"
   unchecked it must name the weekday ("run on Thursday") instead of
   "Today", and the "Not on <weekday>" amber badge must use the same
   weekday names via the shared weekdayName() helper. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

/* ---------- minimal fake DOM (same harness as dom-smoke.test.mjs) ---------- */

class FakeClassList {
  constructor(owner) { this.owner = owner; this.set = new Set((owner.className || '').split(/\s+/).filter(Boolean)); }
  add(...c) { c.forEach((x) => this.set.add(x)); }
  remove(...c) { c.forEach((x) => this.set.delete(x)); }
  toggle(c, on) { if (on === undefined) { this.set.has(c) ? this.set.delete(c) : this.set.add(c); } else if (on) this.set.add(c); else this.set.delete(c); }
  contains(c) { return this.set.has(c); }
}

class FakeElement {
  constructor(tag) {
    this.tagName = String(tag).toUpperCase();
    this.nodeType = 1;
    this.children = [];
    this.attributes = {};
    this.value = '';
    this.checked = false;
    this.disabled = false;
    this.href = '';
    this.title = '';
    this.style = {};
    this._listeners = {};
    this._html = null;
    this._text = '';
    this.className = '';
    this.classList = new FakeClassList(this);
    this.scrollTop = 0;
    this.scrollHeight = 0;
  }
  append(...nodes) {
    for (const n of nodes.flat()) {
      if (n === null || n === undefined || n === false) continue;
      if (typeof n === 'string' || typeof n === 'number') {
        const t = document.createTextNode(String(n));
        t._parent = this;
        this.children.push(t);
      } else {
        if (n._parent) n.remove();
        n._parent = this;
        this.children.push(n);
      }
    }
  }
  appendChild(n) { this.append(n); return n; }
  insertBefore(node, ref) {
    if (ref && this.children.includes(ref)) {
      if (node._parent) node.remove();
      node._parent = this;
      this.children.splice(this.children.indexOf(ref), 0, node);
    } else {
      this.append(node);
    }
    return node;
  }
  prepend(...nodes) { this.children = [...(nodes.flat().map((n) => typeof n === 'string' ? document.createTextNode(n) : n)), ...this.children]; }
  replaceChildren(...nodes) { this.children = []; this.append(...nodes); }
  get textContent() {
    if (this.children.length) {
      return this.children.map((c) => c.nodeType === 3 ? c.text : (c.textContent || '')).join('');
    }
    return this._html ? String(this._html).replace(/<[^>]*>/g, '') : (this._text || '');
  }
  set textContent(v) { this.children = []; if (v != null) this.children.push(document.createTextNode(String(v))); this._text = String(v == null ? '' : v); }
  get innerHTML() { return this._html || ''; }
  set innerHTML(v) { this._html = String(v); this.children = []; }
  setAttribute(k, v) { this.attributes[k] = String(v); if (k === 'class') this.className = String(v); if (k === 'checked') this.checked = true; }
  getAttribute(k) { return this.attributes[k] || null; }
  addEventListener(type, fn) { (this._listeners[type] = this._listeners[type] || []).push(fn); }
  removeEventListener(type, fn) { this._listeners[type] = (this._listeners[type] || []).filter((f) => f !== fn); }
  dispatch(type, evt) { (this._listeners[type] || []).forEach((f) => f(evt || {})); }
  dispatchEvent(type, evt) { return this.dispatch(type, evt); }
  click() { if (this._listeners.click) this.dispatch('click'); else if (this.onclick) this.onclick({}); }
  focus() {}
  blur() {}
  remove() {
    if (this._parent) {
      const i = this._parent.children.indexOf(this);
      if (i >= 0) this._parent.children.splice(i, 1);
    }
    this._parent = null;
  }
  contains(node) {
    if (node === this) return true;
    return this.children.some((c) => c === node || (c.nodeType === 1 && c.contains && c.contains(node)));
  }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
  querySelectorAll(sel) {
    const out = [];
    const match = (n) => {
      const classes = (n.className || '').split(/\s+/).filter(Boolean);
      const tokens = String(sel).split('.').filter(Boolean);
      return tokens.every((t) => classes.includes(t));
    };
    const walk = (n) => {
      for (const c of n.children || []) {
        if (c.nodeType === 1) {
          if (match(c)) out.push(c);
          walk(c);
        }
      }
    };
    walk(this);
    return out;
  }
  get outerHTML() { return `<${this.tagName}>`; }
}

class FakeText {
  constructor(text) { this.nodeType = 3; this.text = String(text); this.textContent = this.text; }
}

const byId = new Map();
const listeners = {};
const windowEvents = [];

const document = {
  createElement: (tag) => new FakeElement(tag),
  createElementNS: (ns, tag) => new FakeElement(tag),
  createTextNode: (text) => new FakeText(text),
  getElementById: (id) => byId.get(id) || null,
  querySelector: () => null,
  querySelectorAll: () => [],
  addEventListener: (type, fn) => { (listeners[type] = listeners[type] || []).push(fn); },
  dispatch: (type, evt) => (listeners[type] || []).forEach((f) => f(evt || {})),
  body: new FakeElement('body'),
  head: new FakeElement('head'),
};

let _hash = '#/';
const location = {
  get hash() { return _hash; },
  set hash(v) {
    _hash = String(v);
    window.dispatchEvent('hashchange', {});
  },
  origin: 'http://fake',
  pathname: '/',
};

const _store = {};
const localStorage = {
  getItem: (k) => (k in _store ? _store[k] : null),
  setItem: (k, v) => { _store[k] = String(v); },
  removeItem: (k) => { delete _store[k]; },
  clear: () => { for (const k of Object.keys(_store)) delete _store[k]; },
  get length() { return Object.keys(_store).length; },
};

const window = {
  addEventListener: (type, fn) => { windowEvents.push([type, fn]); },
  dispatchEvent: (type, evt) => { windowEvents.filter(([t]) => t === type).forEach(([, fn]) => fn(evt || {})); },
  innerWidth: 390,
  innerHeight: 800,
  location,
  document,
  localStorage,
  confirm: () => true,
  matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {} }),
  history: { replaceState() {} },
  AppTheme: { current: () => 'light', set() {}, toggle() {}, icon: () => 'moon' },
};

/* ---------- shared globals the scripts reference bare ---------- */

global.window = window;
global.document = document;
global.location = location;
global.localStorage = localStorage;
global.navigator = { userAgent: 'fake', clipboard: null, onLine: true };
global.Event = class Event { constructor(type, opts) { this.type = type; Object.assign(this, opts || {}); } };
global.requestAnimationFrame = (fn) => setTimeout(fn, 0);

const okResponses = {};
const calls = [];
global.fetch = async (path) => {
  calls.push(String(path));
  const hit = okResponses[String(path)];
  if (hit) {
    return { ok: true, status: 200, text: async () => JSON.stringify(hit) };
  }
  return {
    ok: false,
    status: 502,
    text: async () => JSON.stringify({ error: `stub 502 for ${path}` }),
  };
};

/* ---------- install DOM ids used by app.js / boot.js ---------- */

const root = document.createElement('div');
root.className = 'main';
const ids = [
  'side-nav', 'mobile-nav', 'mode-badge',
  'shell-search', 'shell-search-input', 'shell-search-menu',
  'theme-toggle', 'side-theme-toggle', 'side-favs', 'offline-banner', 'toast-host',
];
ids.forEach((id) => byId.set(id, document.createElement('div')));
byId.set('tab-root', root);

/* ---------- load the app scripts in index.html order ---------- */

global.RailLog = {
  info() {}, warn() {}, error() {}, lifecycle() {}, action() {}, api() {},
  syserr() {}, entries: () => [], raw: () => '', clear() {},
};
require('../../static/boot.js');
global.RailLog = window.RailLog;
require('../../static/ui.js');
require('../../static/api.js');
global.Routes = require('../../static/routes.js');
require('../../static/sections/home.js');
require('../../static/sections/track.js');
require('../../static/sections/station.js');
require('../../static/sections/plan.js');
require('../../static/sections/system.js');
require('../../static/palette.js');
require('../../static/app.js');

/* ---------- helpers ---------- */

const tick = () => new Promise((r) => setTimeout(r, 15));

const WEEKDAYS = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

const todayIso = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
};

/* Fixed date whose weekday is KNOWN (2026-08-20 is a Thursday), so the
   weekday expectations don't mirror the implementation's formula. */
const FIXED_DATE = '2026-08-20';
const FIXED_WEEKDAY = 'Thursday';

const PLAN_STUB = {
  data_source: 'stub',
  src: 'NDLS',
  dst: 'BSB',
  route_description: 'NEW DELHI - VARANASI Express route',
  trains: [
    { number: '12301', name: 'Howrah Rajdhani', departure_time: '16:55', arrival_time: '10:35', runs_on: [true, true, true, true, true, true, true] },
    { number: '12302', name: 'Weekend Special', departure_time: '06:00', arrival_time: '20:00', runs_on: [false, false, false, false, false, false, false] },
  ],
};

/* ---------- tests ---------- */

test('boot renders Home into tab-root', async () => {
  document.dispatch('DOMContentLoaded', {});
  await tick();
  assert.ok(root.textContent.includes('Every train, live'), `home text was: ${root.textContent.slice(0, 120)}`);
});

test('weekdayName maps runs_on indexes 0..6 to Monday..Sunday', () => {
  const wd = window.Sections.plan.weekdayName;
  assert.equal(typeof wd, 'function', 'weekdayName should be exposed on window.Sections.plan');
  for (let i = 0; i < 7; i++) {
    assert.equal(wd(i), WEEKDAYS[i], `weekdayName(${i}) should be ${WEEKDAYS[i]}`);
  }
});

test('day filter shows a compact "N of M · <weekday>" counter badge, no notice paragraphs', async () => {
  calls.length = 0;
  okResponses['/rail-api/ntes/trains-between?src=NDLS&dst=BSB'] = PLAN_STUB;
  location.hash = `#/plan/NDLS/BSB/${FIXED_DATE}`;
  await tick();
  await tick();

  assert.ok(calls.some((p) => p.includes('/rail-api/ntes/trains-between') && p.includes('NDLS')),
    `trains-between should be fetched: ${calls.join(' | ')}`);

  // The old explanatory notice paragraph is gone — density rule: no
  // non-required text in results.
  const notes = root.querySelectorAll('.notice').map((n) => n.textContent);
  assert.ok(notes.length === 0, `no .notice expected, got: ${JSON.stringify(notes)}`);

  // Functional feedback survives as one compact slate badge:
  // "1 of 2 · Thursday" (12302 runs on no days).
  const badges = root.querySelectorAll('.badge')
    .map((b) => b.textContent)
    .filter((t) => t.includes(' of '));
  assert.ok(badges.length === 1, `exactly one counter badge expected, got: ${JSON.stringify(badges)}`);

  const badge = badges[0];
  assert.ok(badge.startsWith('1 of 2'), `counter should read "1 of 2": ${badge}`);
  assert.ok(badge.includes(` · ${FIXED_WEEKDAY}`), `badge should name ${FIXED_WEEKDAY}: ${badge}`);
  assert.ok(!badge.includes('Today'), `badge must not say "Today": ${badge}`);
});

test('amber "Not on <weekday>" badge uses the shared weekday name when flex is on', async () => {
  calls.length = 0;
  okResponses['/rail-api/ntes/trains-between?src=NDLS&dst=BSB'] = PLAN_STUB;
  location.hash = `#/plan/NDLS/BSB/${FIXED_DATE}/flex`;
  await tick();
  await tick();

  assert.ok(root.textContent.includes('Not on ' + FIXED_WEEKDAY),
    `badge should say "Not on ${FIXED_WEEKDAY}": ${root.textContent.slice(0, 400)}`);
});