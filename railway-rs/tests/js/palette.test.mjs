/* palette.test.mjs - smoke tests for static/palette.js (command palette).
   Uses the same fake-DOM harness as dom-smoke.test.mjs. Guards the v4
   palette contract: open/close lifecycle, grouped initial state, smart-query
   instant results, Escape handling, and the global Cmd/Ctrl+K toggle. */

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
  removeEventListener(type, fn) { (this._listeners[type] || []).filter((f) => f !== fn); }
  dispatch(type, evt) { (this._listeners[type] || []).forEach((f) => f(evt || {})); }
  dispatchEvent(type, evt) { return this.dispatch(type, evt); }
  click() { if (this._listeners.click) this.dispatch('click'); else if (this.onclick) this.onclick({}); }
  focus() { document.activeElement = this; }
  blur() { if (document.activeElement === this) document.activeElement = null; }
  scrollIntoView() {}
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

global.window = window;
global.document = document;
global.location = location;
global.localStorage = localStorage;
global.navigator = { userAgent: 'fake', clipboard: null, onLine: true };
global.Event = class Event { constructor(type, opts) { this.type = type; Object.assign(this, opts || {}); } };
global.requestAnimationFrame = (fn) => setTimeout(fn, 0);

global.fetch = async () => ({
  ok: false,
  status: 502,
  text: async () => JSON.stringify({ error: 'stub 502' }),
});

byId.set('tab-root', document.createElement('div'));

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

const tick = () => new Promise((r) => setTimeout(r, 15));

const lastBackdrop = () => document.body.children[document.body.children.length - 1];

/* ---------- tests ---------- */

test('open() renders the palette with grouped initial state and focuses the input', () => {
  window.Palette.open();
  const backdrop = lastBackdrop();
  assert.ok(document.body.children.includes(backdrop), 'backdrop should be appended to body');
  const panel = backdrop.querySelector('.palette');
  assert.ok(panel, 'panel should exist inside the backdrop');
  assert.equal(panel.getAttribute('role'), 'dialog', 'panel should be a dialog');
  assert.equal(document.activeElement.tagName, 'INPUT', 'the palette input should hold focus');
  assert.ok(panel.textContent.includes('Actions'), 'initial state should list quick actions');
});

test('smart query renders an instant Result item (train number)', () => {
  const backdrop = lastBackdrop();
  const input = backdrop.querySelector('.palette-input') || document.body.querySelector('input');
  input.value = '12559';
  input.dispatch('input', {});
  const panel = backdrop.querySelector('.palette');
  assert.ok(panel.textContent.includes('Result'), 'smart hit should render under a Result label');
  assert.ok(panel.textContent.includes('Train 12559'), `got: ${panel.textContent.slice(0, 200)}`);
});

test('Escape closes the palette', () => {
  const backdrop = lastBackdrop();
  const input = backdrop.querySelector('input');
  input.dispatch('keydown', { key: 'Escape', preventDefault() {} });
  assert.ok(!document.body.children.includes(backdrop), 'backdrop should be removed on Escape');
});

test('Cmd/Ctrl+K toggles the palette from anywhere', () => {
  document.dispatch('keydown', { key: 'k', metaKey: true, preventDefault() {} });
  const opened = lastBackdrop();
  assert.ok(opened && opened.querySelector('.palette'), 'Cmd+K should open the palette');
  document.dispatch('keydown', { key: 'k', ctrlKey: true, preventDefault() {} });
  const closed = lastBackdrop();
  assert.ok(!closed || !closed.querySelector('.palette'), 'Ctrl+K again should close it');
});

test('picking an item navigates via the hash router and closes', async () => {
  document.dispatch('keydown', { key: 'k', metaKey: true, preventDefault() {} });
  const backdrop = lastBackdrop();
  await tick();
  const items = backdrop.querySelectorAll('.palette-item');
  assert.ok(items.length > 3, `expected several initial items, got ${items.length}`);
  items[items.length - 1].click();
  await tick();
  assert.notEqual(location.hash, '#/', `hash should have changed, got ${location.hash}`);
  assert.ok(!backdrop._parent, 'palette should close after navigation');
});
