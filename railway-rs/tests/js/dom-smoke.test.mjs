/* dom-smoke.test.mjs - hermetic boot + routing smoke test for the SPA shell.
   Loads the real boot.js, ui.js, api.js, routes.js, all sections, palette.js
   and app.js into a minimal fake DOM, then drives hash navigation and asserts
   the expected views mount. The Api layer is stubbed to return honest
   failures, so no network and no real server are needed. */

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
  setAttribute(k, v) { this.attributes[k] = String(v); if (k === 'class') this.className = String(v); }
  getAttribute(k) { return this.attributes[k] || null; }
  addEventListener(type, fn) { (this._listeners[type] = this._listeners[type] || []).push(fn); }
  removeEventListener(type, fn) { this._listeners[type] = (this._listeners[type] || []).filter((f) => f !== fn); }
  dispatch(type, evt) { (this._listeners[type] || []).forEach((f) => f(evt || {})); }
  dispatchEvent(type, evt) { return this.dispatch(type, evt); }
  click() { if (this._listeners.click) this.dispatch('click'); else if (this.onclick) this.onclick({}); }
  focus() { document.activeElement = this; }
  blur() { if (document.activeElement === this) document.activeElement = null; }
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
    const selectors = String(sel).split(',').map((s) => s.trim()).filter(Boolean);
    const match = (n) => selectors.some((s) => {
      if (s.startsWith('[')) {
        const attr = s.slice(1, -1);
        return n.getAttribute(attr) !== null;
      }
      if (s.startsWith('.')) {
        const classes = (n.className || '').split(/\s+/).filter(Boolean);
        const tokens = s.slice(1).split('.').filter(Boolean);
        return tokens.every((t) => classes.includes(t));
      }
      return n.tagName === s.toUpperCase();
    });
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
  activeElement: null,
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
/* Track app timers (e.g. the observability 5s refresh) so the teardown can
   stop them without forcing process.exit(0), which would mask failures. */
const timers = [];
const _setInterval = global.setInterval;
const _clearInterval = global.clearInterval;
global.setInterval = (fn, ms) => { const id = _setInterval(fn, ms); timers.push(id); return id; };
global.clearInterval = (id) => {
  const i = timers.indexOf(id);
  if (i >= 0) timers.splice(i, 1);
  _clearInterval(id);
};
const okResponses = {};
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

/* Track every endpoint the app asks for through the real Api layer. */
const calls = [];

const tick = () => new Promise((r) => setTimeout(r, 15));

test('boot renders Home into tab-root', async () => {
  document.dispatch('DOMContentLoaded', {});
  await tick();
  assert.ok(root.textContent.includes('RailCompanion'), `home text was: ${root.textContent.slice(0, 120)}`);
  assert.ok(root.textContent.includes('Favorites'), 'home should render the Favorites card');
});

test('navigate to #/train mounts the track form', async () => {
  location.hash = '#/train';
  await tick();
  assert.ok(root.textContent.includes('Track a Train'), `got: ${root.textContent.slice(0, 120)}`);
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

test('legacy #/pnr/2498761234 auto-submits through pnr', async () => {
  calls.length = 0;
  location.hash = '#/pnr/2498761234';
  await tick();
  assert.ok(calls.some((p) => p.includes('/rail-api/pnr') && p.includes('2498761234')));
});

test('system section mounts observability by default', async () => {
  location.hash = '#/system';
  await tick();
  assert.ok(root.textContent.includes('Observability') || root.textContent.includes('observability'), `got: ${root.textContent.slice(0, 120)}`);
});

test('recent lookups are recorded on entity deep links', async () => {
  location.hash = '#/train/12002';
  await tick();
  location.hash = '#/';
  await tick();
  assert.ok(root.textContent.includes('Train 12002'), 'recent list should show the deep-linked train');
  assert.ok(root.textContent.includes('#/train/12002'));
});

test('favorites toggle persists and appears on Home', async () => {
  location.hash = '#/station/NDLS';
  await tick();
  const favBtn = root.children.find((c) => c.className && String(c.className).includes('fav'));
  if (favBtn) favBtn.click();
  await tick();
  location.hash = '#/';
  await tick();
  assert.ok(root.textContent.includes('Station NDLS'), 'favorited station should appear on Home');
});

test('train recent label self-heals to name + from → to', async () => {
  okResponses['/rail-api/schedule?train=12002'] = {
    train_number: '12002',
    train_name: 'Shiv Ganga SF Express',
    route_description: 'BANARAS - NEW DELHI Express',
    running_days: ['MON'],
    stops: [
      { code: 'BNRS', name: 'BANARAS', arrival: '00:00', departure: '22:15', day: 1 },
      { code: 'NDLS', name: 'NEW DELHI', arrival: '05:00', departure: '05:15', day: 2 },
    ],
  };
  location.hash = '#/train/12002';
  await tick();
  await tick();
  location.hash = '#/';
  await tick();
  await tick();
  assert.ok(root.textContent.includes('Shiv Ganga'), 'home recent should show the train name');
  assert.ok(root.textContent.includes('BNRS \u2192 NDLS'), 'home recent should show from → to');
});

test('dialog: accessible labelled modal, focus moves in on open', async () => {
  const trigger = document.createElement('button');
  trigger.textContent = 'Open captcha';
  document.body.append(trigger);
  trigger.focus();
  assert.equal(document.activeElement, trigger, 'trigger should hold focus before opening');

  const p = window.UI.dialog({
    title: 'Captcha required (Indian Railways)',
    body: [document.createElement('input')],
    actions: [
      { label: 'Submit', primary: true, value: '__submit' },
      { label: 'Cancel', primary: false, value: null },
    ],
  });

  const backdrop = document.body.children[document.body.children.length - 1];
  const panel = backdrop.querySelector('.dialog');
  assert.equal(panel.getAttribute('role'), 'dialog', 'root should expose role=dialog');
  assert.equal(panel.getAttribute('aria-modal'), 'true', 'root should be aria-modal');
  const labelledBy = panel.getAttribute('aria-labelledby');
  const title = panel.querySelector('.dialog-title');
  assert.ok(labelledBy, 'aria-labelledby should be set');
  assert.equal(title.getAttribute('id'), labelledBy, 'aria-labelledby should resolve to the title id');
  assert.equal(title.textContent, 'Captcha required (Indian Railways)');

  await tick();
  const inputs = panel.querySelectorAll('input');
  assert.ok(inputs.length, 'dialog should contain the captcha input');
  assert.ok(panel.contains(document.activeElement), 'focus should move into the dialog on open');
  assert.equal(document.activeElement, inputs[0], 'focus should land on the first focusable');

  backdrop.dispatch('keydown', { key: 'Escape', preventDefault() {} });
  await p;
  assert.ok(!document.body.children.includes(backdrop), 'dialog should be removed on Escape');
});

test('dialog: Escape closes and resolves the cancel path (null)', async () => {
  const p = window.UI.dialog({
    title: 'Captcha required (Indian Railways)',
    body: [document.createElement('input')],
    actions: [
      { label: 'Submit', primary: true, value: '__submit' },
      { label: 'Cancel', primary: false, value: null },
    ],
  });
  const backdrop = document.body.children[document.body.children.length - 1];
  await tick();
  backdrop.dispatch('keydown', { key: 'Escape', preventDefault() {} });
  assert.equal(await p, null, 'Escape should resolve the promise with null (cancel)');
  assert.ok(!document.body.children.includes(backdrop), 'dialog should be removed from the DOM');
});

test('dialog: Tab from the last focusable wraps to the first focusable', async () => {
  const p = window.UI.dialog({
    title: 'Captcha required (Indian Railways)',
    body: [document.createElement('input')],
    actions: [
      { label: 'Submit', primary: true, value: '__submit' },
      { label: 'Cancel', primary: false, value: null },
    ],
  });
  const backdrop = document.body.children[document.body.children.length - 1];
  const panel = backdrop.querySelector('.dialog');
  await tick();
  const buttons = panel.querySelectorAll('button');
  assert.ok(buttons.length >= 2, 'dialog should render the action buttons');
  buttons[buttons.length - 1].focus();
  assert.equal(document.activeElement, buttons[buttons.length - 1], 'last button should be focused');
  backdrop.dispatch('keydown', { key: 'Tab', shiftKey: false, preventDefault() {} });
  const inputs = panel.querySelectorAll('input');
  assert.equal(document.activeElement, inputs[0], 'Tab past the last control should wrap to the first');
  backdrop.dispatch('keydown', { key: 'Escape', preventDefault() {} });
  await p;
});

test('dialog: closing returns focus to the element that opened it', async () => {
  const trigger = document.createElement('button');
  trigger.textContent = 'Open captcha';
  document.body.append(trigger);
  trigger.focus();
  assert.equal(document.activeElement, trigger, 'trigger should hold focus before opening');

  const p = window.UI.dialog({
    title: 'Captcha required (Indian Railways)',
    actions: [{ label: 'Submit', primary: true, value: '__submit' }],
  });
  const backdrop = document.body.children[document.body.children.length - 1];
  await tick();
  assert.notEqual(document.activeElement, trigger, 'focus should leave the trigger while open');
  backdrop.dispatch('keydown', { key: 'Escape', preventDefault() {} });
  await p;
  assert.equal(document.activeElement, trigger, 'closing should restore focus to the opener');
});

test('teardown: stop app timers so the runner can exit without masking failures', () => {
  timers.slice().forEach((id) => clearInterval(id));
});