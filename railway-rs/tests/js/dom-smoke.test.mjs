/* dom-smoke.test.mjs - hermetic boot + routing smoke test for the SPA shell.
   Loads the real boot.js, ui.js, api.js, routes.js, all sections, the two
   retained tabs, and app.js into a minimal fake DOM, then drives hash
   navigation and asserts the expected views mount. The Api layer is stubbed
   to return honest failures, so no network and no real server are needed. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

/* ---------- minimal fake DOM ---------- */

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
    this.disabled = false;
    this.href = '';
    this.title = '';
    this.style = {};
    this._listeners = {};
    this._html = null;
    this._text = '';
    this.className = '';
    this.classList = new FakeClassList(this);
  }
  append(...nodes) {
    for (const n of nodes.flat()) {
      if (n === null || n === undefined || n === false) continue;
      if (typeof n === 'string' || typeof n === 'number') this.children.push(document.createTextNode(String(n)));
      else this.children.push(n);
    }
  }
  replaceChildren(...nodes) { this.children = []; this.append(...nodes); }
  get textContent() {
    return this.children.map((c) => c.nodeType === 3 ? c.text : (c.textContent || '')).join('');
  }
  set textContent(v) { this.children = []; if (v != null) this.children.push(document.createTextNode(String(v))); this._text = String(v == null ? '' : v); }
  get innerHTML() { return this._html || ''; }
  set innerHTML(v) { this._html = String(v); this.children = []; }
  setAttribute(k, v) { this.attributes[k] = String(v); if (k === 'class') this.className = String(v); }
  getAttribute(k) { return this.attributes[k] || null; }
  addEventListener(type, fn) { (this._listeners[type] = this._listeners[type] || []).push(fn); }
  removeEventListener(type, fn) { this._listeners[type] = (this._listeners[type] || []).filter((f) => f !== fn); }
  dispatch(type, evt) { (this._listeners[type] || []).forEach((f) => f(evt || {})); }
  click() { if (this._listeners.click) this.dispatch('click'); else if (this.onclick) this.onclick({}); }
  focus() {}
  blur() {}
  remove() { /* detached */ }
  querySelector() { return null; }
  querySelectorAll() { return []; }
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
  createTextNode: (text) => new FakeText(text),
  getElementById: (id) => byId.get(id) || null,
  querySelector: () => null,
  querySelectorAll: () => [],
  addEventListener: (type, fn) => { (listeners[type] = listeners[type] || []).push(fn); },
  dispatch: (type, evt) => (listeners[type] || []).forEach((f) => f(evt || {})),
  body: new FakeElement('body'),
};

let _hash = '#/';
const location = {
  get hash() { return _hash; },
  set hash(v) {
    _hash = String(v);
    window.dispatchEvent('hashchange', {});
  },
};

const localStorage = {
  _data: {},
  getItem: (k) => (k in this._data ? this._data[k] : null),
  setItem: (k, v) => { this._data[k] = String(v); },
  removeItem: (k) => { delete this._data[k]; },
  clear: () => { this._data = {}; },
  get length() { return Object.keys(this._data).length; },
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
};

/* ---------- shared globals the scripts reference bare ---------- */

global.window = window;
global.document = document;
global.location = location;
global.localStorage = localStorage;
global.navigator = { userAgent: 'fake', clipboard: null };
global.Event = class Event { constructor(type) { this.type = type; } };
global.fetch = async (path) => {
  calls.push(String(path));
  return {
    ok: false,
    status: 502,
    text: async () => JSON.stringify({ error: `stub 502 for ${path}` }),
  };
};

/* ---------- install DOM ids used by app.js ---------- */

const root = document.createElement('div');
root.className = 'main';
const ids = ['side-nav', 'mobile-nav', 'mode-badge', 'shell-search', 'shell-search-input', 'shell-search-menu'];
ids.forEach((id) => byId.set(id, document.createElement('div')));
byId.set('tab-root', root);

/* ---------- load the app scripts in index.html order ---------- */

/* boot.js invokes bare `RailLog` at load time; pre-stub it, then replace it
   with the real implementation boot.js installs on window.RailLog. */
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
require('../../static/sections/pnr.js');
require('../../static/sections/more.js');
require('../../static/tabs/train_on_map.js');
require('../../static/tabs/observability.js');
require('../../static/app.js');

/* Track every endpoint the app asks for through the real Api layer. */
const calls = [];

const tick = () => new Promise((r) => setTimeout(r, 10));

test('boot renders Home into tab-root', async () => {
  document.dispatch('DOMContentLoaded', {});
  await tick();
  assert.ok(root.textContent.includes('RailCompanion'), `home text was: ${root.textContent.slice(0, 120)}`);
  assert.ok(root.textContent.includes('Recent lookups'));
});

test('navigate to #/track mounts the spot form', async () => {
  location.hash = '#/track';
  await tick();
  assert.ok(root.textContent.includes('Spot Train'), `got: ${root.textContent.slice(0, 120)}`);
});

test('deep link #/train/12559/schedule auto-submits through Api.schedule', async () => {
  calls.length = 0;
  location.hash = '#/train/12559/schedule';
  await tick();
  assert.ok(root.textContent.includes('Train 12559'), `title missing: ${root.textContent.slice(0, 120)}`);
  assert.ok(calls.some((p) => p.includes('/rail-api/schedule') && p.includes('12559')), `calls: ${calls.join(' | ')}`);
  assert.ok(root.textContent.includes('stub 502'), 'honest error should be rendered');
});

test('deep link #/station/NDLS/tt auto-submits through stationTimetable', async () => {
  calls.length = 0;
  location.hash = '#/station/NDLS/tt';
  await tick();
  assert.ok(root.textContent.includes('Station NDLS'));
  assert.ok(calls.some((p) => p.includes('/rail-api/ntes/station-timetable') && p.includes('NDLS')));
});

test('deep link #/plan/NDLS/BSB/availability auto-submits through availability', async () => {
  calls.length = 0;
  location.hash = '#/plan/NDLS/BSB/availability';
  await tick();
  assert.ok(root.textContent.includes('NDLS'), `title missing: ${root.textContent.slice(0, 120)}`);
  assert.ok(calls.some((p) => p.includes('/rail-api/irctc/availability') && p.includes('src=NDLS')));
});

test('deep link #/pnr/2498761234 auto-submits through pnr', async () => {
  calls.length = 0;
  location.hash = '#/pnr/2498761234';
  await tick();
  assert.ok(calls.some((p) => p.includes('/rail-api/pnr') && p.includes('2498761234')));
});

test('more hub and more view mount', async () => {
  location.hash = '#/more';
  await tick();
  assert.ok(root.textContent.includes('More'), `got: ${root.textContent.slice(0, 120)}`);
  calls.length = 0;
  location.hash = '#/more/heritage';
  await tick();
  assert.ok(calls.some((p) => p.includes('/rail-api/ntes/heritage')));
});

test('recent lookups are recorded on entity deep links', async () => {
  location.hash = '#/train/12002';
  await tick();
  location.hash = '#/';
  await tick();
  assert.ok(root.textContent.includes('Train 12002'), 'recent list should show the deep-linked train');
  assert.ok(root.textContent.includes('#/train/12002'));
});
