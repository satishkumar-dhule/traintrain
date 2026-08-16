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

  /* A train input wrapped in a .autocomplete container with IntelliSense.
     Returns { wrap, input }. */
  function trainInput(placeholder) {
    return wrapAutocomplete('train', placeholder || 'Train number or name');
  }

  /* A station input wrapped in a .autocomplete container with IntelliSense.
     Returns { wrap, input }. */
  function stationInput(placeholder) {
    return wrapAutocomplete('station', placeholder || 'Station code');
  }

  function wrapAutocomplete(type, placeholder) {
    const wrap = el('div', { class: 'autocomplete' });
    const input = el('input', { class: 'input', autocomplete: 'off', placeholder });
    wrap.append(input);
    window.AutoComplete.attach(input, { type });
    return { wrap, input };
  }

  /* Query form card from [label, control] field rows plus an optional submit
     button. Spacing between fields comes from `.field + .field`. */
  function queryCard(rows, submitBtn) {
    const c = el('div', { class: 'card' });
    rows.forEach(([text, control]) => c.append(el('div', { class: 'field' }, label(text), control)));
    if (submitBtn) c.append(el('div', { class: 'row mt-12' }, submitBtn));
    return c;
  }

  /* Loading-state + honest-error flow for a fetch. Shows a spinner, runs fn(),
     renders the error box on failure, and returns the response (or null on
     error) for the caller to render. `opts.button` is disabled while loading;
     `opts.failText` prefixes thrown-error messages. */
  function fetchFlow(resultsEl, fn, opts = {}) {
    const btn = opts.button || null;
    render(resultsEl, spinner());
    if (btn) btn.disabled = true;
    return Promise.resolve()
      .then(fn)
      .then((res) => {
        if (!res || res.ok === false) {
          render(resultsEl, errorBox(res && res.error ? res.error : (opts.failText || 'Request failed.')));
          return null;
        }
        return res;
      })
      .catch((err) => {
        const m = err && err.message ? err.message : String(err);
        render(resultsEl, errorBox((opts.failText ? opts.failText + ': ' : '') + m));
        return null;
      })
      .finally(() => { if (btn) btn.disabled = false; });
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

  /* Delay cell (HTML string for ui.table): "—" when on time, "N min" when late. */
  function delay(minutes) {
    if (!minutes || minutes <= 0) return '<span class="muted">-</span>';
    return `<span class="bold">${minutes} min</span>`;
  }

  /* Runs-on letters from 7 booleans (Mon..Sun), e.g. "M-TW-FS-" -> "M-TW-FS-". */
  function days(runsOn) {
    const letters = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
    if (!Array.isArray(runsOn)) return '—';
    return runsOn.map((on, i) => (on ? letters[i] : '-')).join('');
  }

  /* Status badge (HTML string for ui.table) for a station halt status. */
  function statusCell(status) {
    const kind = status === 'departed' ? 'slate' : status === 'expected' ? 'amber' : 'blue';
    return `<span class="badge badge-${kind}">${status || 'scheduled'}</span>`;
  }

  /* Escape a value for use as table cell HTML (ui.table injects innerHTML). */
  function esc(v) {
    return String(v == null || v === '' ? '—' : v)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  /* Today's date as YYYY-MM-DD (the backend's accepted date format). */
  function today() {
    const d = new Date();
    const mm = String(d.getMonth() + 1).padStart(2, '0');
    const dd = String(d.getDate()).padStart(2, '0');
    return `${d.getFullYear()}-${mm}-${dd}`;
  }

  /* Normalize + validate a station-code input, mirroring the backend's
     require_station rules. Returns { code } (uppercased 2-4 char) or { error }. */
  function stationCode(value) {
    const code = String(value || '').trim().toUpperCase();
    if (!code) return { error: 'Enter a station code.' };
    if (code.length < 2 || code.length > 4 || !/^[A-Z0-9]+$/.test(code)) {
      return { error: `Invalid station code: ${code}. Must be a 2-4 character code.` };
    }
    return { code };
  }

  return { el, card, badge, errorBox, successBox, notice, spinner, emptyState, table, label, render, withLoading, debounce, fmtTime, stationCode, trainInput, stationInput, queryCard, fetchFlow, delay, days, statusCell, esc, today };
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
