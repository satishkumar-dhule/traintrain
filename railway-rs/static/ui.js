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
    const c = el('div', { class: 'card-sm' });
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
    const c = el('div', { class: 'card-sm' });
    rows.forEach(([text, control]) => c.append(el('div', { class: 'field' }, label(text), control)));
    if (submitBtn) c.append(el('div', { class: 'row', style: 'gap:6px;margin-top:8px;' }, submitBtn));
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
        const timedOut = err && (err.code === 'TIMEOUT' || /timeout/i.test(m));
        render(resultsEl, errorBox(timedOut ? 'Upstream is not responding — try again shortly.' : (opts.failText ? opts.failText + ': ' : '') + m));
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

  function entityLink(type, code, label, navigate) {
    const hash = type === 'train'
      ? Routes.href({ section: 'train', params: { train: code } })
      : Routes.href({ section: 'station', params: { station: code } });
    return el('button', {
      class: 'entity-link',
      onclick: () => navigate(hash),
      text: label || code,
      'aria-label': 'Go to ' + type + ' ' + code,
    });
  }

  function skeleton(rows) {
    rows = rows || 3;
    const wrap = el('div', { class: 'col', style: 'gap:4px;' });
    for (let i = 0; i < rows; i++) wrap.append(el('div', { class: 'skeleton skeleton-row' }));
    return wrap;
  }

  /* ---------- Modern component system ---------- */

  /* Inline SVG icon from the /icons.svg sprite. */
  function icon(name, cls) {
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('class', 'ic' + (cls ? ' ' + cls : ''));
    svg.setAttribute('aria-hidden', 'true');
    const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
    use.setAttribute('href', '/icons.svg#i-' + name);
    svg.append(use);
    return svg;
  }

  /* Icon button (icon-only). */
  function iconBtn(name, label, onClick, cls) {
    const b = el('button', {
      class: 'btn icon-btn' + (cls ? ' ' + cls : ''),
      type: 'button',
      'aria-label': label,
      title: label,
      onclick: onClick,
    });
    b.append(icon(name));
    return b;
  }

  /* Toast notifications. */
  function toastHost() {
    let host = document.getElementById('toast-host');
    if (!host) {
      host = el('div', { id: 'toast-host', class: 'toast-host', 'aria-live': 'polite', 'aria-atomic': 'false' });
      document.body.append(host);
    }
    return host;
  }

  function toast(message, kind, opts) {
    kind = kind || 'info';
    opts = opts || {};
    const host = toastHost();
    const t = el('div', { class: 'toast toast-' + kind, role: 'status' });
    const iconName = kind === 'success' ? 'check' : kind === 'error' ? 'alert' : 'info';
    t.append(icon(iconName, 'toast-ic'), el('span', { class: 'toast-msg', text: message }));
    if (opts.action) t.append(opts.action);
    const dismiss = () => {
      t.classList.remove('show');
      setTimeout(() => t.remove(), 200);
    };
    const closeBtn = el('button', { class: 'toast-close', 'aria-label': 'Dismiss', onclick: dismiss });
    closeBtn.append(icon('close'));
    t.append(closeBtn);
    host.append(t);
    requestAnimationFrame(() => t.classList.add('show'));
    const timer = setTimeout(dismiss, opts.keep || 4000);
    t.addEventListener('mouseenter', () => clearTimeout(timer));
    t.addEventListener('mouseleave', () => setTimeout(dismiss, 1200));
    return { dismiss };
  }

  /* Error state with retry. */
  function errorState(title, hint, retryFn) {
    const box = el('div', { class: 'state-box state-error' });
    box.append(icon('alert', 'state-icon'));
    const body = el('div', { class: 'state-body' });
    body.append(el('p', { class: 'state-title', text: title || 'Something went wrong' }));
    if (hint) body.append(el('p', { class: 'state-hint', text: hint }));
    if (retryFn) {
      const retry = el('button', { class: 'btn secondary btn-sm mt-8', onclick: retryFn });
      retry.append(icon('refresh', 'btn-ic'), el('span', { text: 'Retry' }));
      body.append(retry);
    }
    box.append(body);
    return box;
  }

  /* Empty state (icon + title + hint). */
  function emptyState(iconName, title, hint) {
    const box = el('div', { class: 'state-box' });
    box.append(icon(iconName || 'search', 'state-icon'));
    const body = el('div', { class: 'state-body' });
    body.append(el('p', { class: 'state-title', text: title || 'Nothing here yet' }));
    if (hint) body.append(el('p', { class: 'state-hint', text: hint }));
    box.append(body);
    return box;
  }

  /* Skeleton table (rows x cols of shimmer bars). */
  function skeletonTable(rows, cols) {
    rows = rows || 5;
    cols = cols || 4;
    const wrap = el('div', { class: 'table-wrap' });
    const tbl = el('table', { class: 'tbl' });
    const tbody = el('tbody');
    for (let i = 0; i < rows; i++) {
      const tr = el('tr');
      for (let j = 0; j < cols; j++) {
        const td = el('td');
        const bar = el('div', { class: 'skeleton' });
        bar.style.width = (45 + ((i * 7 + j * 13) % 45)) + '%';
        td.append(bar);
        tr.append(td);
      }
      tbody.append(tr);
    }
    tbl.append(tbody);
    wrap.append(tbl);
    return wrap;
  }

  /* Skeleton card with a title line + body lines. */
  function skeletonCard(lines) {
    lines = lines || 3;
    const c = el('div', { class: 'card' });
    c.append(el('div', { class: 'skeleton', style: 'width:40%;height:16px;' }));
    for (let i = 0; i < lines; i++) c.append(el('div', { class: 'skeleton skeleton-row' }));
    return c;
  }

  function agoText(iso) {
    if (!iso) return '';
    const d = new Date(iso);
    if (isNaN(d.getTime())) return String(iso);
    const s = Math.max(0, Math.floor((Date.now() - d.getTime()) / 1000));
    if (s < 5) return 'just now';
    if (s < 60) return s + 's ago';
    const m = Math.floor(s / 60);
    if (m < 60) return m + 'm ago';
    return friendlyTime(iso);
  }

  /* "Updated Xs ago · Refresh [+ Auto]" row for live views. */
  function refreshRow(opts) {
    opts = opts || {};
    const row = el('div', { class: 'refresh-row' });
    const ago = el('span', { class: 'refresh-ago muted', text: opts.updatedAt ? 'Updated ' + agoText(opts.updatedAt) : ' ' });
    const refreshBtn = el('button', { class: 'btn ghost btn-sm', 'aria-label': 'Refresh', onclick: () => opts.onRefresh && opts.onRefresh() });
    refreshBtn.append(icon('refresh', 'btn-ic'), el('span', { text: 'Refresh' }));
    row.append(ago, refreshBtn);
    if (opts.autoKey !== undefined) {
      const toggle = el('button', {
        class: 'btn ghost btn-sm auto-toggle',
        type: 'button',
        role: 'switch',
        'aria-pressed': 'false',
        title: 'Auto-refresh every ' + (opts.autoMs || 30000) + 'ms',
      });
      toggle.append(icon('refresh', 'btn-ic'), el('span', { text: 'Auto' }));
      toggle.addEventListener('click', () => {
        const on = toggle.getAttribute('aria-pressed') === 'true';
        toggle.setAttribute('aria-pressed', String(!on));
        toggle.classList.toggle('on', !on);
        if (opts.onAuto) opts.onAuto(!on);
      });
      row.append(toggle);
    }
    return {
      row,
      setUpdated(iso) { ago.textContent = iso ? 'Updated ' + agoText(iso) : ' '; },
    };
  }

  /* Pulsing LIVE badge. */
  function liveDot(label) {
    return el('span', { class: 'live-badge' }, el('span', { class: 'live-dot' }), el('span', { text: label || 'LIVE' }));
  }

  /* KPI tile. */
  function statTile(label, value, sub, kind) {
    const t = el('div', { class: 'stat-tile' + (kind ? ' stat-' + kind : '') });
    t.append(
      el('p', { class: 'stat-label', text: label }),
      el('p', { class: 'stat-value', text: value === null || value === undefined ? '—' : value }),
    );
    if (sub) t.append(el('p', { class: 'stat-sub', text: sub }));
    return t;
  }

  /* Segmented control. options: ['value'] or [value, label]. */
  function seg(options, active, onSelect) {
    const bar = el('div', { class: 'seg', role: 'tablist' });
    options.forEach((o) => {
      const value = Array.isArray(o) ? o[0] : o;
      const label = Array.isArray(o) ? o[1] : o;
      const btn = el('button', {
        class: 'seg-item' + (value === active ? ' active' : ''),
        role: 'tab',
        'aria-selected': String(value === active),
        onclick: () => onSelect && onSelect(value),
      }, label);
      bar.append(btn);
    });
    return bar;
  }

  /* Entity hero header: avatar + title + badges + facts + actions. */
  function entityHero(opts) {
    opts = opts || {};
    const hero = el('div', { class: 'hero' });
    const avatar = el('div', { class: 'hero-avatar' });
    avatar.append(icon(opts.icon || 'train'));
    hero.append(avatar);
    const head = el('div', { class: 'hero-head' });
    head.append(el('h1', { class: 'hero-title', text: opts.title || '' }));
    if (opts.subtitle) head.append(el('p', { class: 'hero-subtitle', text: opts.subtitle }));
    if (opts.badges && opts.badges.length) head.append(el('div', { class: 'hero-badges' }, ...opts.badges));
    hero.append(head);
    if (opts.facts && opts.facts.length) {
      const factsRow = el('div', { class: 'hero-facts' });
      opts.facts.forEach(([l, v]) => {
        factsRow.append(el('div', { class: 'hero-fact' },
          el('span', { class: 'hero-fact-label', text: l }),
          el('span', { class: 'hero-fact-value', text: v === null || v === undefined ? '—' : v }),
        ));
      });
      hero.append(factsRow);
    }
    if (opts.actions && opts.actions.length) hero.append(el('div', { class: 'hero-actions' }, ...opts.actions));
    return hero;
  }

  /* Copy the current (or given) deep link. */
  function copyLink(hash) {
    const url = location.origin + location.pathname + (hash || location.hash);
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(url)
        .then(() => { toast('Link copied', 'success'); return true; })
        .catch(() => { fallbackCopy(url); return true; });
    }
    fallbackCopy(url);
    return Promise.resolve(true);
  }

  function fallbackCopy(text) {
    const ta = el('textarea', { style: 'position:fixed;opacity:0;pointer-events:none;' });
    ta.value = text;
    document.body.append(ta);
    ta.select();
    try { document.execCommand('copy'); toast('Link copied', 'success'); }
    catch (e) { toast('Copy failed', 'error'); }
    ta.remove();
  }

  /* Share via Web Share API, falling back to copy-link. */
  function share(hash) {
    const url = location.origin + location.pathname + (hash || location.hash);
    if (navigator.share) {
      return navigator.share({ title: document.title, url }).catch(() => false);
    }
    return copyLink(hash);
  }

  /* Promise-based dialog (focus trap + Esc). Resolves with the value of the
     dismissed action, or null when cancelled. Accessible: role=dialog +
     aria-modal on the root, aria-labelledby the title, focus moves in on
     open, Tab is trapped inside, and focus returns to the opener on close. */
  function dialog(opts) {
    opts = opts || {};
    return new Promise((resolve) => {
      const opener = opts.trigger || document.activeElement;
      const backdrop = el('div', { class: 'dialog-backdrop' });
      const panel = el('div', { class: 'dialog', role: 'dialog', 'aria-modal': 'true' });
      const titleId = 'dialog-title-' + Date.now() + '-' + Math.floor(Math.random() * 1e9).toString(36);
      if (opts.title) {
        panel.setAttribute('aria-labelledby', titleId);
        panel.append(el('h3', { class: 'dialog-title', id: titleId, text: opts.title }));
      } else if (opts.label) {
        panel.setAttribute('aria-label', opts.label);
      }
      if (opts.body) panel.append(el('div', { class: 'dialog-body' }, opts.body));
      const actRow = el('div', { class: 'dialog-actions' });
      panel.append(actRow);
      let settled = false;
      const close = (value) => {
        if (settled) return;
        settled = true;
        backdrop.remove();
        if (opener && opener.focus) opener.focus();
        resolve(value);
      };
      backdrop.addEventListener('mousedown', (e) => { if (e.target === backdrop) close(null); });
      backdrop.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') { e.preventDefault(); close(null); return; }
        if (e.key !== 'Tab') return;
        if (!focusables.length) { e.preventDefault(); return; }
        if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
        else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      });
      (opts.actions || [{ label: 'OK', value: true, primary: true }]).forEach((a) => {
        actRow.append(el('button', {
          class: a.primary === false ? 'btn ghost' : 'btn',
          onclick: () => close(a.value === undefined ? true : a.value),
        }, a.label));
      });
      backdrop.append(panel);
      document.body.append(backdrop);
      const focusables = panel.querySelectorAll('input, button, select, [tabindex]');
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      setTimeout(() => { (first || panel).focus(); }, 0);
    });
  }

  /* Journey progress track: origin → current → destination with fill line. */
  function journeyProgress(stops, currentIdx) {
    const wrap = el('div', { class: 'journey' });
    if (!Array.isArray(stops) || stops.length < 2) return wrap;
    const origin = stops[0];
    const dest = stops[stops.length - 1];
    const cur = (currentIdx >= 0 && currentIdx < stops.length) ? stops[currentIdx] : null;
    const pct = stops.length > 1 ? Math.round((currentIdx / (stops.length - 1)) * 100) : 0;
    const line = el('div', { class: 'journey-line' });
    line.append(el('div', { class: 'journey-fill', style: 'width:' + Math.max(3, Math.min(100, pct)) + '%' }));
    const node = (s, kind, label) => {
      const n = el('div', { class: 'journey-node ' + kind });
      n.append(el('span', { class: 'journey-dot' }));
      n.append(el('span', { class: 'journey-node-label', text: label || s.name || s.code || '' }));
      if (s.code) n.append(el('span', { class: 'journey-node-code', text: s.code }));
      return n;
    };
    wrap.append(
      line,
      el('div', { class: 'journey-track' },
        node(origin, 'origin', origin.name || origin.code),
        cur && currentIdx > 0 && currentIdx < stops.length - 1 ? node(cur, 'current', cur.name || cur.code) : null,
        node(dest, 'dest', dest.name || dest.code),
      ),
    );
    return wrap;
  }

  function friendlyDate(dateStr) {
    if (!dateStr) return '—';
    var d = new Date(dateStr + (String(dateStr).indexOf('T') >= 0 ? '' : 'T00:00:00'));
    var now = new Date();
    var today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    var target = new Date(d.getFullYear(), d.getMonth(), d.getDate());
    var diff = Math.round((target - today) / 86400000);
    if (diff === 0) return 'Today';
    if (diff === 1) return 'Tomorrow';
    if (diff === 2) return 'Day after tomorrow';
    if (diff === -1) return 'Yesterday';
    if (diff >= -6 && diff <= 6) {
      return d.toLocaleDateString('en-US', { weekday: 'long' });
    }
    return d.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric', month: 'short' });
  }

  function friendlyTime(isoStr) {
    if (!isoStr) return '—';
    var d = new Date(isoStr);
    if (isNaN(d.getTime())) return String(isoStr);
    var now = new Date();
    var diffMs = now - d;
    if (diffMs < 0) return friendlyDate(isoStr.slice(0, 10));
    var diffMin = Math.floor(diffMs / 60000);
    if (diffMin < 1) return 'just now';
    if (diffMin < 60) return diffMin + ' min ago';
    var diffHr = Math.floor(diffMin / 60);
    if (diffHr < 24) return diffHr + ' hr ago';
    return friendlyDate(isoStr.slice(0, 10));
  }

  function dateQuickPick(onSelect) {
    var today = new Date();
    var bar = el('div', { class: 'date-quick-pick' });
    var labels = ['Today', 'Tmrw', '+2d'];
    for (var i = 0; i < 3; i++) {
      var d = new Date(today);
      d.setDate(d.getDate() + i);
      var iso = d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
      (function(label, dateIso) {
        bar.append(el('button', {
          class: 'btn btn-sm',
          text: label,
          onclick: function(e) { e.preventDefault(); onSelect(dateIso, label); },
        }));
      })(labels[i], iso);
    }
    return bar;
  }

  /* ---------- Calendar picker (IRCTC-style) ---------- */

  function parseIso(s) {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(s || ''));
    if (!m) return null;
    const d = new Date(+m[1], +m[2] - 1, +m[3]);
    return isNaN(d.getTime()) ? null : d;
  }

  function toIso(d) {
    return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
  }

  /* A journey-date picker in the IRCTC style: a trigger button showing the
     date in friendly form ("Today", "Wed, 20 Aug") that opens a month-grid
     popover. Dates before today are disabled (the IRCTC booking window);
     navigation is capped at `maxOffsetDays` ahead (default 120).
     Returns { wrap, getDate(), setDate(iso, quiet), open(), close() }.
     Keyboard: Tab walks the day buttons, Enter picks, Esc closes and returns
     focus to the trigger. */
  function calendarPicker(opts) {
    opts = opts || {};
    const maxOffset = opts.maxOffsetDays || 120;
    const todayD = new Date();
    todayD.setHours(0, 0, 0, 0);
    const todayIso = toIso(todayD);
    const maxD = new Date(todayD);
    maxD.setDate(maxD.getDate() + maxOffset);
    const state = { iso: null, month: null, pop: null };

    const wrap = el('div', { class: 'cal-wrap' });
    const trigger = el('button', { class: 'btn cal-trigger', type: 'button', 'aria-haspopup': 'dialog' });
    trigger.addEventListener('click', open);
    wrap.append(trigger);

    function setTrigger() {
      trigger.replaceChildren(icon('calendar', 'btn-ic'), el('span', { text: friendlyDate(state.iso) }));
      trigger.setAttribute('aria-label', 'Journey date ' + state.iso + ' (' + friendlyDate(state.iso) + '). Open the calendar.');
    }

    function setDate(iso, quiet) {
      const d = parseIso(iso);
      if (!d) return false;
      state.iso = toIso(d);
      if (!state.month) state.month = new Date(d.getFullYear(), d.getMonth(), 1);
      setTrigger();
      if (!quiet && opts.onSelect) opts.onSelect(state.iso);
      return true;
    }

    function pick(iso) {
      state.iso = iso;
      setTrigger();
      close();
      if (opts.onSelect) opts.onSelect(iso);
    }

    function shiftMonth(delta) {
      const next = new Date(state.month.getFullYear(), state.month.getMonth() + delta, 1);
      const minMonth = new Date(todayD.getFullYear(), todayD.getMonth(), 1);
      const maxMonth = new Date(maxD.getFullYear(), maxD.getMonth(), 1);
      if (next < minMonth || next > maxMonth) return;
      state.month = next;
      if (state.pop) renderMonth(state.pop);
    }

    function renderMonth(pop) {
      pop.title.textContent = state.month.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
      const cells = [];
      ['M', 'T', 'W', 'T', 'F', 'S', 'S'].forEach((w) => cells.push(el('span', { class: 'cal-wd', text: w, 'aria-hidden': 'true' })));
      const offset = (new Date(state.month.getFullYear(), state.month.getMonth(), 1).getDay() + 6) % 7;
      for (let i = 0; i < offset; i++) cells.push(el('span', { class: 'cal-void', 'aria-hidden': 'true' }));
      const daysInMonth = new Date(state.month.getFullYear(), state.month.getMonth() + 1, 0).getDate();
      for (let d = 1; d <= daysInMonth; d++) {
        const date = new Date(state.month.getFullYear(), state.month.getMonth(), d);
        const iso = toIso(date);
        const attrs = {
          class: 'cal-day' + (iso === todayIso ? ' today' : '') + (iso === state.iso ? ' sel' : ''),
          type: 'button',
          'aria-label': friendlyDate(iso) + ', ' + iso,
          'aria-pressed': String(iso === state.iso),
          onclick: () => pick(iso),
          text: String(d),
        };
        if (date < todayD || date > maxD) attrs.disabled = true;
        cells.push(el('button', attrs));
      }
      pop.grid.replaceChildren(...cells);
      return cells.find((c) => !c.disabled) || null;
    }

    function buildPopover() {
      const pop = el('div', { class: 'cal-pop', role: 'dialog', 'aria-label': 'Choose a journey date' });
      const head = el('div', { class: 'cal-head' });
      const title = el('span', { class: 'cal-month' });
      head.append(iconBtn('chevron-l', 'Previous month', () => shiftMonth(-1)), title, iconBtn('chevron-r', 'Next month', () => shiftMonth(1)));
      pop.append(head);

      const quick = el('div', { class: 'cal-quick' });
      ['Today', 'Tomorrow', '+2d', '+7d'].forEach((label, i) => {
        const d = new Date(todayD);
        d.setDate(d.getDate() + i);
        quick.append(el('button', { class: 'btn btn-sm', type: 'button', text: label, onclick: () => pick(toIso(d)) }));
      });
      pop.append(quick);

      const grid = el('div', { class: 'cal-grid' });
      pop.append(grid);
      const built = { pop, grid, title };
      renderMonth(built);
      return built;
    }

    function open() {
      if (state.pop) { close(); return; }
      const built = buildPopover();
      state.pop = built;
      wrap.append(built.pop);
      const target = built.grid.querySelector('.cal-day.sel') || built.grid.querySelector('.cal-day:not(:disabled)');
      if (target) target.focus();
      const onDocDown = (e) => { if (!wrap.contains(e.target)) close(); };
      document.addEventListener('mousedown', onDocDown);
      built.pop.addEventListener('keydown', (e) => { if (e.key === 'Escape') { e.preventDefault(); close(); } });
      built.cleanup = () => document.removeEventListener('mousedown', onDocDown);
    }

    function close() {
      if (!state.pop) return;
      if (state.pop.cleanup) state.pop.cleanup();
      state.pop.pop.remove();
      state.pop = null;
      trigger.focus();
    }

    setDate(opts.initial || todayIso, true);
    return { wrap, trigger, getDate: () => state.iso, setDate, open, close };
  }

  function collapsibleTable(headers, rows, maxVisible) {
    maxVisible = maxVisible || 10;
    var wrap = el('div');
    if (rows.length <= maxVisible) {
      wrap.append(table(headers, rows));
      return wrap;
    }
    var visible = rows.slice(0, maxVisible);
    var hidden = rows.slice(maxVisible);
    wrap.append(table(headers, visible));
    var toggle = el('button', {
      class: 'btn ghost btn-sm',
      text: 'Show all ' + rows.length + ' rows',
      onclick: function() {
        wrap.replaceChildren(table(headers, rows));
      },
    });
    wrap.append(el('div', { class: 'row', style: 'margin-top:4px;' }, toggle));
    return wrap;
  }

  function contextualActions(entity, navigate) {
    var bar = el('div', { class: 'contextual-actions' });
    if (!entity || !entity.type) return bar;
    var actions = [];
    if (entity.type === 'train' && entity.code) {
      actions = [
        { label: 'Delay', hash: Routes.href({ section: 'train', view: 'delay', params: { train: entity.code } }) },
        { label: 'Map', hash: Routes.href({ section: 'train', view: 'map', params: { train: entity.code } }) },
        { label: 'Schedule', hash: Routes.href({ section: 'train', view: 'schedule', params: { train: entity.code } }) },
      ];
    } else if (entity.type === 'station' && entity.code) {
      actions = [
        { label: 'Plan from here', hash: Routes.href({ section: 'plan', params: { src: entity.code, dst: '' } }) },
        { label: 'Timetable', hash: Routes.href({ section: 'station', view: 'tt', params: { station: entity.code } }) },
      ];
    }
    actions.forEach(function(a) {
      bar.append(el('button', {
        class: 'contextual-action',
        text: a.label,
        onclick: function() { navigate(a.hash); },
      }));
    });
    return bar;
  }

  function pillBar(views, labels, activeView, onSwitch) {
    var bar = el('div', { class: 'pill-bar' });
    views.forEach(function(v) {
      var btn = el('button', {
        class: 'pill' + (v === activeView ? ' active' : ''),
        text: labels[v] || v,
        'data-view': v,
        onclick: function() { onSwitch(v); },
      });
      bar.append(btn);
    });
    return bar;
  }

  /* ---------- IRCTC-style components ---------- */

  /* Booking console card: navy tab strip (IRCTC: PNR STATUS | CHARTS/VACANCY |
     BOOK TICKET) + body. opts: { tabs: [[id,label,icon]...], active, onTab(id)
     -> body nodes }. Switching a tab re-renders the body from onTab(). */
  function console(opts) {
    opts = opts || {};
    const card = el('div', { class: 'console' });
    const tabs = el('div', { class: 'console-tabs', role: 'tablist' });
    const body = el('div', { class: 'console-body' });
    const buttons = {};
    const renderTab = (id) => {
      Object.entries(buttons).forEach(([k, b]) => {
        b.classList.toggle('active', k === id);
        b.setAttribute('aria-selected', String(k === id));
      });
      if (opts.onTab) render(body, opts.onTab(id));
    };
    (opts.tabs || []).forEach(([id, label, iconName]) => {
      const b = el('button', {
        class: 'console-tab',
        role: 'tab',
        'aria-selected': String(id === opts.active),
        onclick: () => renderTab(id),
      });
      if (iconName) b.append(icon(iconName));
      b.append(el('span', { text: label }));
      buttons[id] = b;
      tabs.append(b);
    });
    card.append(tabs, body);
    renderTab(opts.active);
    return { card, body, renderTab };
  }

  /* Merged From/To station inputs (route box) with a swap button pinned to the
     right edge between them. Each input gets the app's station autocomplete.
     Returns { wrap, from, to } (from/to are the raw <input> elements). */
  function routeBox(opts) {
    opts = opts || {};
    const wrap = el('div', { class: 'route-box' });
    const from = routeField('From', 'from', opts.from || '');
    const to = routeField('To', 'to', opts.to || '');
    const swap = el('button', {
      class: 'swap-btn', type: 'button', 'aria-label': 'Swap stations',
      title: 'Swap stations',
      onclick: () => { const v = from.input.value; from.input.value = to.input.value; to.input.value = v; },
    });
    swap.append(icon('swap'));
    wrap.append(from.wrap, to.wrap, swap);
    return { wrap, from: from.input, to: to.input, swap };
  }

  function routeField(label, kind, value) {
    const wrap = el('div', { class: 'route-field route-' + kind });
    const input = el('input', {
      class: 'route-input', autocomplete: 'off', spellcheck: 'false',
      placeholder: ' ', 'aria-label': label + ' station',
    });
    if (value) input.value = value;
    const dot = el('span', { class: 'route-dot ' + kind, 'aria-hidden': 'true' });
    const lbl = el('span', { class: 'route-label', text: label.toUpperCase() });
    wrap.append(input, dot, lbl);
    window.AutoComplete.attach(input, { type: 'station' });
    return { wrap, input };
  }

  /* Floating-label field: caption pinned to the top + value below, optional
     left icon. Returns { wrap, control }. */
  function flField(opts) {
    opts = opts || {};
    const wrap = el('div', { class: 'fl-field' });
    if (opts.cls) wrap.classList.add(opts.cls);
    wrap.append(el('span', { class: 'fl-caption', text: opts.label || 'Input' }));
    if (opts.icon) wrap.append(icon(opts.icon, 'fl-ic'));
    return wrap;
  }

  /* Text input with floating label + integrated icon (e.g. PNR number). */
  function flInput(opts) {
    opts = opts || {};
    const wrap = flField(opts);
    const input = el('input', {
      class: 'fl-control fl-text',
      autocomplete: opts.autocomplete || 'off',
      spellcheck: 'false',
      inputmode: opts.inputmode || 'text',
      'aria-label': opts.label || 'Input',
    });
    if (opts.placeholder) input.placeholder = opts.placeholder;
    if (opts.value) input.value = opts.value;
    wrap.append(input);
    return { wrap, input };
  }

  /* Select with floating caption + chevron (IRCTC class dropdown). */
  function flSelect(opts) {
    opts = opts || {};
    const wrap = flField(opts);
    const sel = el('select', { class: 'fl-control fl-select', 'aria-label': opts.label || 'Select' });
    (opts.options || []).forEach((o) => {
      const value = Array.isArray(o) ? o[0] : o;
      const text = Array.isArray(o) ? o[1] : o;
      sel.append(el('option', { value, text }));
    });
    if (opts.value !== undefined) sel.value = opts.value;
    sel.addEventListener('change', () => { if (opts.onChange) opts.onChange(sel.value); });
    wrap.append(sel, icon('chevron-d', 'fl-chev'));
    return { wrap, select: sel, get: () => sel.value, set: (v) => { sel.value = v; } };
  }

  /* Journey-date field: floating caption + calendar icon + the IRCTC-style
     month-grid picker (see calendarPicker). Returns { wrap, getDate,
     setDate, open, close }. */
  function flDate(opts) {
    opts = opts || {};
    const wrap = flField(opts);
    const picker = calendarPicker({
      initial: opts.initial,
      onSelect: opts.onSelect,
      maxOffsetDays: opts.maxOffsetDays,
    });
    picker.trigger.classList.add('fl-control');
    wrap.append(picker.wrap);
    return {
      wrap, trigger: picker.trigger,
      getDate: () => picker.getDate(), setDate: picker.setDate,
      open: picker.open, close: picker.close,
    };
  }

  /* Dense checkbox row for the console's preference grid. */
  function checkRow(opts) {
    const row = el('label', { class: 'check-row' });
    const cb = el('input', { type: 'checkbox', checked: opts.checked ? 'checked' : undefined });
    row.append(cb, el('span', { text: opts.label }));
    if (opts.onChange) cb.addEventListener('change', () => opts.onChange(cb.checked));
    return { row, cb, get: () => cb.checked, set: (on) => { cb.checked = !!on; } };
  }

  /* Big orange SEARCH CTA with press physics. */
  function searchBtn(opts) {
    opts = opts || {};
    const b = el('button', {
      class: 'search-btn', type: 'button',
      onclick: opts.onclick,
      'aria-label': opts.label || 'Search',
    });
    if (opts.icon !== false) b.append(icon('search'));
    b.append(el('span', { text: opts.label || 'Search' }));
    if (opts.cls) b.classList.add(opts.cls);
    if (opts.disabled) b.disabled = true;
    return b;
  }

  /* Coach-chart view (shared by Plan -> Chart and the Home Charts/Vacancy
     tab). Renders the train header + coach legend + berth grid for a chart
     API response. Returns an array of nodes to render. */
  function chartView(res, ui, ctx) {
    const headerRow = card('Train',
      el('div', { class: 'row align-center mt-8' },
        entityLink('train', res.train_number || '', res.train_number || '', ctx.navigate),
        el('span', { class: 'bold', text: res.train_name || '' }),
        badge('Journey ' + (res.journey_date || ''), 'slate'),
        res.boarding_station ? badge('Boarding ' + res.boarding_station, 'slate') : null,
      ),
    );
    if (res.notice) headerRow.append(el('p', { class: 'notice' }, res.notice));

    const coaches = res.coaches || [];
    if (!Array.isArray(coaches) || !coaches.length) {
      return [headerRow, notice('No coach data returned.')];
    }

    const codes = coaches.map((c) => c.code);
    const legend = el('div', { class: 'row align-center mt-8', style: 'gap:12px;flex-wrap:wrap;' },
      el('span', { class: 'text-sm muted', text: 'Berths' }),
      badge('vacant', 'green'),
      badge('occupied', 'red'),
      badge('not reserved', 'slate'),
    );

    const berthWrap = el('div', { class: 'mt-8' });
    const showCoach = (code) => {
      const c = coaches.find((x) => x.code === code) || coaches[0];
      render(berthWrap,
        el('div', { class: 'col' },
          el('span', { class: 'text-sm muted', text: 'Berths' }),
          berthCell(c.berths, ui),
        ),
      );
    };

    const coachSeg = seg(codes, codes[0], showCoach);
    showCoach(codes[0]);

    const list = card('Coaches', coachSeg, legend, berthWrap);
    return [headerRow, list];
  }

  function berthCell(berths, ui) {
    const row = el('div', { class: 'row', style: 'gap:4px;flex-wrap:wrap;' });
    (Array.isArray(berths) ? berths : []).forEach((b) => {
      const cls = b.status === 'vacant' ? 'berth vacant' : b.status === 'occupied' ? 'berth occupied' : 'berth not-reserved';
      row.append(el('span', {
        class: cls,
        title: `${b.number}: ${b.status}`,
        text: String(b.number),
      }));
    });
    if (!row.children.length) row.append(el('span', { class: 'text-sm muted', text: '—' }));
    return row;
  }

  return { el, card, badge, errorBox, successBox, notice, spinner, emptyState, table, label, render, withLoading, debounce, fmtTime, stationCode, trainInput, stationInput, queryCard, fetchFlow, delay, days, statusCell, esc, today, entityLink, skeleton, friendlyDate, friendlyTime, dateQuickPick, calendarPicker, collapsibleTable, contextualActions, pillBar, icon, iconBtn, toast, errorState, skeletonTable, skeletonCard, refreshRow, liveDot, statTile, seg, entityHero, copyLink, share, dialog, journeyProgress, agoText, console, routeBox, flInput, flSelect, flDate, checkRow, searchBtn, chartView };
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
