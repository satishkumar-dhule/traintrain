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
      cells.forEach((c) => row.append(el('td', { html: c })));
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

/* Autocomplete for train / station inputs. Usage:
   AutoComplete.attach(input, { type: 'train'|'station', onSelect(value), minChars })
   onSelect receives { code, name, number?, name? } or null on clear. */
window.AutoComplete = (() => {
  let active = null;

  function close() {
    if (active && active.menu.parentNode) active.menu.remove();
    active = null;
  }

  function attach(input, { type, onSelect, minChars = 1 }) {
    input.addEventListener('focus', () => {
      if (input.value.trim().length >= minChars) search(input, type, onSelect);
    });
    input.addEventListener('input', debounceInput(input, () => search(input, type, onSelect), minChars));
    input.addEventListener('blur', () => setTimeout(close, 150));
  }

  function debounceInput(input, fn, minChars) {
    let t;
    return () => {
      clearTimeout(t);
      if (input.value.trim().length < minChars) { close(); return; }
      t = setTimeout(fn, 250);
    };
  }

  async function search(input, type, onSelect) {
    const q = input.value.trim();
    if (q.length < 1) { close(); return; }
    const apiFn = type === 'train' ? window.Api.searchTrains : window.Api.searchStations;
    const res = await apiFn(q);
    if (!res || res.ok === false || !Array.isArray(res)) { close(); return; }
    const items = res.slice(0, 8);
    close();
    if (!items.length) return;

    const wrap = input.closest('.autocomplete') || input.parentElement;
    const menu = window.UI.el('div', { class: 'ac-menu' });
    items.forEach((item) => {
      const row = window.UI.el('div', {
        class: 'ac-item',
        onmousedown: (e) => { e.preventDefault(); input.value = item.code || item.number; close(); onSelect(item); },
      });
      row.append(
        window.UI.el('span', { class: 'ac-code', text: item.code || item.number }),
        window.UI.el('span', { class: 'ac-name', text: item.name }),
      );
      menu.append(row);
    });
    wrap.appendChild(menu);
    active = { menu };
  }

  return { attach };
})();
