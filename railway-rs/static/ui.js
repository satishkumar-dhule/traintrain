/* ui.js - DOM helpers shared by every tab. Loaded before the tab scripts. */

window.UI = (() => {
  function el(tag, attrs = {}, ...children) {
    const node = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs || {})) {
      if (k === 'class') node.className = v;
      else if (k === 'text') node.textContent = v;
      else if (k === 'html') node.innerHTML = v;
      else if (k === 'onclick') node.onclick = v;
      else if (k.startsWith('on') && typeof v === 'function') node.addEventListener(k.slice(2), v);
      else if (v !== undefined && v !== null) node.setAttribute(k, v);
    }
    for (const c of children.flat()) {
      if (c === null || c === undefined || c === false) continue;
      node.append(c.nodeType ? c : document.createTextNode(c));
    }
    return node;
  }

  function card(title, ...children) {
    const c = el('div', { class: 'card' });
    if (title) c.append(el('h2', { text: title }));
    c.append(...children.flat());
    return c;
  }

  function badge(text, kind = 'slate') {
    return el('span', { class: `badge badge-${kind}`, text });
  }

  function errorBox(message) {
    return el('div', { class: 'error-box' }, message);
  }

  function successBox(message) {
    return el('div', { class: 'success-box' }, message);
  }

  function notice(text) {
    return el('p', { class: 'notice' }, text);
  }

  function spinner() {
    return el('div', { class: 'spinner' });
  }

  function emptyState(text) {
    return el('p', { class: 'text-sm muted' }, text);
  }

  function table(headers, rows) {
    const t = el('div', { class: 'table-wrap' });
    const tbl = el('table', { class: 'tbl' });
    const thead = el('thead');
    const tr = el('tr');
    headers.forEach((h) => tr.append(el('th', { text: h })));
    thead.append(tr);
    tbl.append(thead);
    const tbody = el('tbody');
    rows.forEach((cells) => {
      const row = el('tr');
      cells.forEach((c) => {
        const td = el('td');
        if (c && c.nodeType) {
          td.append(c);            // DOM node -> append, never stringify
        } else {
          td.innerHTML = c === null || c === undefined ? '' : c; // HTML string
        }
        row.append(td);
      });
      tbody.append(row);
    });
    tbl.append(tbody);
    t.append(tbl);
    return t;
  }

  function label(text) {
    return el('label', { class: 'label', text });
  }

  /* Clear a container and optionally append children. */
  function render(root, ...children) {
    root.replaceChildren();
    root.append(...children.flat());
  }

  /* Generic loading state for async actions. Returns [setLoading, setError]. */
  function withLoading(btn, loadingText = 'Loading…') {
    const original = btn.textContent;
    const setLoading = (on) => {
      btn.disabled = on;
      btn.textContent = on ? loadingText : original;
    };
    return setLoading;
  }

  function debounce(fn, ms = 250) {
    let t;
    return (...args) => { clearTimeout(t); t = setTimeout(() => fn(...args), ms); };
  }

  function fmtTime(hhmm) { return hhmm || '--:--'; }

  return { el, card, badge, errorBox, successBox, notice, spinner, emptyState, table, label, render, withLoading, debounce, fmtTime };
})();

/* Autocomplete / IntelliSense for train and station inputs. Usage:
   AutoComplete.attach(input, {
     type: 'train' | 'station' | 'both',  // 'both' uses the combined suggest endpoint
     onSelect(item),                       // { code, number, name } or null on clear
     minChars,
   })
   Trains match by number and name; stations by code and name. The server keeps
   both lists pre-warmed, so every keystroke hits the local dataset only. */
window.AutoComplete = (() => {
  /* One state object per input, keyed through a WeakMap so attach/events and
     the search closure always share the same menu/highlight/request token. */
  const states = new WeakMap();
  function stateOf(input) {
    let s = states.get(input);
    if (!s) { s = { token: 0, menu: null, items: [], hl: -1 }; states.set(input, s); }
    return s;
  }

  function close(state) {
    if (state.menu && state.menu.parentNode) state.menu.remove();
    state.menu = null;
    state.items = [];
    state.hl = -1;
  }

  function attach(input, { type, onSelect, minChars = 1 }) {
    const state = stateOf(input);
    const runSearch = debounceInput(input, () => search(input, type, onSelect), minChars);

    input.addEventListener('focus', () => {
      if (input.value.trim().length >= minChars) runSearch();
    });
    input.addEventListener('input', runSearch);
    input.addEventListener('blur', () => setTimeout(() => close(state), 150));

    input.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { close(state); input.blur(); }
      else if (e.key === 'ArrowDown' && state.items.length) {
        e.preventDefault();
        state.hl = (state.hl + 1) % state.items.length;
        updateHighlight(state);
      } else if (e.key === 'ArrowUp' && state.items.length) {
        e.preventDefault();
        state.hl = (state.hl - 1 + state.items.length) % state.items.length;
        updateHighlight(state);
      } else if (e.key === 'Enter' && state.hl >= 0 && state.items[state.hl]) {
        e.preventDefault();
        e.stopImmediatePropagation();
        pick(input, state, state.items[state.hl], onSelect);
      } else if (e.key === 'Tab' && state.hl >= 0 && state.items[state.hl]) {
        e.preventDefault();
        e.stopImmediatePropagation();
        state.hl = 0;
        pick(input, state, state.items[state.hl], onSelect);
      }
    });
  }

  function debounceInput(input, fn, minChars) {
    let t;
    return () => {
      clearTimeout(t);
      if (input.value.trim().length < minChars) { close(stateOf(input)); return; }
      t = setTimeout(fn, 220);
    };
  }

  async function search(input, type, onSelect) {
    const state = stateOf(input);
    const q = input.value.trim();
    if (!q) { close(state); return; }
    const my = ++state.token;
    const res = type === 'both'
      ? await window.Api.suggest(q)
      : type === 'station' ? await window.Api.searchStations(q) : await window.Api.searchTrains(q);
    if (my !== state.token) return;
    if (!res || res.ok === false || !Array.isArray(res)) { close(state); return; }
    close(state);
    state.items = res.slice(0, 8);
    state.hl = -1;
    if (!state.items.length) return;

    const wrap = input.closest('.autocomplete') || input.parentElement;
    const menu = window.UI.el('div', { class: 'ac-menu' });
    state.items.forEach((item) => {
      const row = window.UI.el('div', {
        class: 'ac-item',
        onmousedown: (e) => { e.preventDefault(); pick(input, state, item, onSelect); },
      });
      row.append(
        window.UI.el('span', { class: 'ac-code', text: item.code || item.number }),
        window.UI.el('span', { class: 'ac-name', text: item.name }),
      );
      menu.append(row);
    });
    wrap.appendChild(menu);
    state.menu = menu;
  }

  function pick(input, state, item, onSelect) {
    input.value = item.code || item.number;
    close(state);
    if (onSelect) onSelect(item);
  }

  function updateHighlight(state) {
    if (!state.menu) return;
    [...state.menu.querySelectorAll('.ac-item')].forEach((r, i) => r.classList.toggle('hl', i === state.hl));
  }

  return { attach };
})();
