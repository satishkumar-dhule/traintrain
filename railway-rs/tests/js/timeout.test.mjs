/* timeout.test.mjs - client-side fetch timeout behaviour for static/api.js and
   static/ui.js fetchFlow. Uses the same fake-DOM harness and window.fetch
   stub pattern as dom-smoke.test.mjs. A hung upstream must settle quickly
   with a TIMEOUT marker (api) and an honest error box (ui), while the normal
   200 path keeps working. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

/* ---------- minimal fake DOM (same pattern as dom-smoke.test.mjs) ---------- */

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

/* Default fetch stub: hangs forever BUT records/aborts when the caller's
   AbortSignal fires, so tests prove the timeout actually reaches fetch. */
const abortedCalls = [];
global.fetch = (url, opts) => new Promise((resolve, reject) => {
  const sig = opts && opts.signal;
  if (!sig || !sig.addEventListener) { reject(new Error('no signal passed to fetch')); return; }
  sig.addEventListener('abort', () => { abortedCalls.push(String(url)); reject(new Error('Aborted')); });
});

/* ---------- load the real scripts (boot -> ui -> api, index.html order) ---------- */

global.RailLog = {
  info() {}, warn() {}, error() {}, lifecycle() {}, action() {}, api() {},
  syserr() {}, entries: () => [], raw: () => '', clear() {},
};
require('../../static/boot.js');
global.RailLog = window.RailLog;
require('../../static/ui.js');
require('../../static/api.js');

const ui = window.UI;
const Api = window.Api;

/* Keep the event loop alive while a hung-fetch test is pending so the runner
   does not cancel the remaining tests; cleared in teardown. */
const keepAlive = setInterval(() => {}, 1000);

const watchdog = (ms = 1000) => new Promise((r) => setTimeout(() => r('watchdog'), ms));

test('api request rejects with a TIMEOUT marker when upstream hangs', async () => {
  const winner = await Promise.race([
    Api.request('/rail-api/schedule?train=12559', { timeout: 50 })
      .then(() => 'settled', (err) => ({ err })),
    watchdog(),
  ]);
  assert.notEqual(winner, 'watchdog', 'request never settled within 1s');
  const marker = winner.err && (winner.err.code === 'TIMEOUT' || /timeout/i.test(String(winner.err.message)));
  assert.ok(marker, `expected a TIMEOUT marker, got: ${JSON.stringify(winner.err && winner.err.message)}`);
  assert.equal(abortedCalls.length, 1, 'the abort must have reached the real fetch');
});

test('fetchFlow renders an honest error box after a timeout', async () => {
  const host = document.createElement('div');
  const winner = await Promise.race([
    ui.fetchFlow(host, () => Api.request('/rail-api/schedule?train=12559', { timeout: 50 }))
      .then(() => 'settled', () => 'settled'),
    watchdog(),
  ]);
  assert.notEqual(winner, 'watchdog', 'fetchFlow never settled within 1s');
  const box = host.querySelector('.error-box');
  assert.ok(box, 'an error box should be rendered after a timeout');
  assert.ok(
    String(box.textContent).includes('Upstream is not responding'),
    `error box text was: ${box.textContent}`,
  );
});

test('a normal 200 response still resolves and renders without an error box', async () => {
  global.fetch = async () => ({
    ok: true,
    status: 200,
    text: async () => JSON.stringify({ ok: true, train: '12559', train_name: 'Shiv Ganga Express' }),
  });
  const host = document.createElement('div');
  const res = await ui.fetchFlow(host, () => Api.request('/rail-api/schedule?train=12559', { timeout: 500 }));
  assert.equal(res.train, '12559');
  assert.equal(res.train_name, 'Shiv Ganga Express');
  assert.equal(host.querySelector('.error-box'), null, 'no error box on success');
});

test('teardown: stop the keep-alive so the runner can exit', () => {
  clearInterval(keepAlive);
});
