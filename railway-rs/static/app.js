/* app.js - shell: builds navigation from registered tabs (window.Tabs),
   mounts the active tab into #tab-root, fetches source-status for the badge. */

window.Tabs = window.Tabs || {};

(() => {
  const NAV = [
    { id: 'pnr', label: 'PNR Status', icon: '🎫' },
    { id: 'live_status', label: 'Spot Train', icon: '🚄' },
    { id: 'live_station', label: 'Live Station', icon: '⏱️' },
    { id: 'trains_between', label: 'Trains B/W', icon: '📍' },
    { id: 'station_timetable', label: 'Station TT', icon: '🗓️' },
    { id: 'average_delay', label: 'Avg Delay', icon: '⏰' },
    { id: 'heritage', label: 'Heritage', icon: '🚞' },
    { id: 'parcel', label: 'Parcel SPL', icon: '📦' },
    { id: 'journey_basis', label: 'Journey Basis', icon: '🚉' },
    { id: 'train_on_map', label: 'Train Map', icon: '🗺️' },
    { id: 'schedule', label: 'Schedule', icon: '🚉' },
    { id: 'exceptional', label: 'Exceptional', icon: '⚠️' },
    { id: 'stations', label: 'Stations', icon: '🗺️' },
    { id: 'settings', label: 'Settings', icon: '⚙️' },
    { id: 'observability', label: 'Observability', icon: '📊' },
  ];

  const state = { active: 'pnr', ctx: null };

  function ctx() {
    if (!state.ctx) {
      state.ctx = {
        api: window.Api,
        ui: window.UI,
        autocomplete: window.AutoComplete,
        captcha: { show: showCaptcha },
      };
    }
    return state.ctx;
  }

  function navFor(containerId) {
    const nav = document.getElementById(containerId);
    NAV.forEach((t) => {
      if (!window.Tabs[t.id]) return; // tab not built yet -> hide entry
      const btn = window.UI.el('button', {
        class: 'nav-item',
        onclick: () => activate(t.id),
      });
      btn.append(
        window.UI.el('span', { class: 'nav-icon', text: t.icon }),
        window.UI.el('span', { text: t.label }),
      );
      nav.append(btn);
    });
  }

  function activate(id) {
    state.active = id;
    document.querySelectorAll('.nav-item').forEach((n) => n.classList.remove('active'));
    const navBtns = [...document.querySelectorAll('.nav-item')];
    const idx = NAV.findIndex((t) => t.id === id);
    if (navBtns[idx]) navBtns[idx].classList.add('active');
    mount(id);
  }

  function mount(id) {
    const tab = window.Tabs[id];
    const root = document.getElementById('tab-root');
    if (!tab) {
      RailLog.warn('mount: tab not registered:', id);
      return;
    }
    window.UI.render(root, window.UI.el('div', { class: 'tab-header' }));
    RailLog.info('mounting tab:', id);
    try {
      tab.mount(root, ctx());
    } catch (err) {
      const msg = err && err.stack ? err.stack : (err && err.message ? err.message : String(err));
      RailLog.error('tab.mount threw for', id, '->', msg);
      window.UI.render(root, window.UI.errorBox(`Tab "${id}" failed to render: ${msg}`));
    }
  }

  /* Global search selection wiring: navigate to the most relevant tab and
     prefill its input. Public hooks: window.railwayTabs.{stations,trains};
     a 'railway:select' CustomEvent is dispatched for any additional listeners. */
  function prefill(id, value) {
    if (!window.Tabs[id]) return;
    activate(id);
    const input = document.querySelector('#tab-root input.input');
    if (input) input.value = value;
    const submit = document.querySelector('#tab-root .btn');
    if (submit) submit.click();
  }

  window.railwayTabs = {
    stations: {
      selectStation(code, name) {
        document.dispatchEvent(new CustomEvent('railway:select', { detail: { type: 'station', code, name } }));
        prefill('live_station', code);
      },
    },
    trains: {
      selectTrain(number, name) {
        document.dispatchEvent(new CustomEvent('railway:select', { detail: { type: 'train', number, name } }));
        prefill('live_status', number);
      },
    },
  };

  /* Header autocomplete: debounced IntelliSense over the pre-warmed local
     datasets via the combined suggest endpoint (stations + trains in one
     round trip), guarded against out-of-order responses with a request token. */
  function initShellSearch() {
    const wrap = document.getElementById('shell-search');
    const input = document.getElementById('shell-search-input');
    const menu = document.getElementById('shell-search-menu');
    if (!wrap || !input || !menu) return;

    let token = 0;
    let items = [];
    let hl = -1;

    function closeMenu() {
      menu.replaceChildren();
      menu.classList.add('hidden');
      items = [];
      hl = -1;
    }

    function updateHighlight() {
      [...menu.querySelectorAll('.ac-item')].forEach((row, i) => row.classList.toggle('hl', i === hl));
    }

    function renderMenu() {
      menu.replaceChildren();
      if (!items.length) {
        menu.append(window.UI.el('div', { class: 'ac-group', text: 'No results' }));
        menu.classList.remove('hidden');
        return;
      }
      const groups = [
        ['Stations', items.filter((it) => it.type === 'station')],
        ['Trains', items.filter((it) => it.type === 'train')],
      ];
      groups.forEach(([label, list]) => {
        if (!list.length) return;
        menu.append(window.UI.el('div', { class: 'ac-group', text: label }));
        list.forEach((it) => {
          const row = window.UI.el('div', {
            class: 'ac-item',
            onmousedown: (e) => { e.preventDefault(); select(it); },
          });
          row.append(
            window.UI.el('span', { class: 'ac-code', text: it.code || it.number }),
            window.UI.el('span', { class: 'ac-name', text: it.name }),
          );
          menu.append(row);
        });
      });
      menu.classList.remove('hidden');
      updateHighlight();
    }

    async function search() {
      const q = input.value.trim();
      if (!q) { closeMenu(); return; }
      const my = ++token;
      const suggestions = await window.Api.suggest(q);
      if (my !== token) return;
      items = [];
      if (Array.isArray(suggestions)) {
        suggestions.forEach((it) => items.push({
          type: it.type === 'train' ? 'train' : 'station',
          code: it.code,
          number: it.number,
          name: it.name,
        }));
      }
      hl = -1;
      renderMenu();
    }

    const debouncedSearch = window.UI.debounce(search, 250);

    function select(it) {
      if (it.type === 'station') window.railwayTabs.stations.selectStation(it.code, it.name);
      else window.railwayTabs.trains.selectTrain(it.number, it.name);
      input.value = '';
      closeMenu();
    }

    input.addEventListener('input', debouncedSearch);
    input.addEventListener('focus', () => { if (input.value.trim()) debouncedSearch(); });
    input.addEventListener('blur', () => setTimeout(closeMenu, 150));
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { closeMenu(); input.blur(); }
      else if (e.key === 'ArrowDown' && items.length) { e.preventDefault(); hl = (hl + 1) % items.length; updateHighlight(); }
      else if (e.key === 'ArrowUp' && items.length) { e.preventDefault(); hl = (hl - 1 + items.length) % items.length; updateHighlight(); }
      else if (e.key === 'Enter' && hl >= 0 && items[hl]) { e.preventDefault(); select(items[hl]); }
    });
    document.addEventListener('mousedown', (e) => { if (!wrap.contains(e.target)) closeMenu(); });
  }

  /* CAPTCHA flow: given an Api result with ok:false and status 428, present the
     image and let the user answer. Rendered as a fixed overlay on <body> so the
     underlying tab (and its detached render targets) stays intact; the promise
     resolves to the captcha params { session_id, source, text } or null when
     dismissed. */
  function showCaptcha(challenge) {
    return new Promise((resolve) => {
      const backdrop = window.UI.el('div', {
        class: 'card',
        style: 'position:fixed;inset:0;z-index:100;background:rgba(15,23,42,0.55);display:flex;align-items:center;justify-content:center;padding:16px;',
        onclick: (e) => e.stopPropagation(),
      });
      const panel = window.UI.el('div', {
        class: 'card',
        style: 'width:min(360px,100%);',
      });
      const input = window.UI.el('input', { class: 'input', autocomplete: 'off' });
      const close = () => backdrop.remove();
      panel.append(
        window.UI.el('h3', { text: `Captcha required (${challenge.source})` }),
        window.UI.el('img', { src: challenge.image, alt: 'captcha', style: 'border:1px solid #e2e8f0;border-radius:8px;margin:8px 0;' }),
        window.UI.el('label', { class: 'label', text: 'It is an arithmetic question (e.g. "4 + 40 = ?" → 44). Type the answer:' }),
        input,
        window.UI.el('div', { class: 'row mt-12' },
          window.UI.el('button', { class: 'btn', text: 'Submit', onclick: () => {
            const text = input.value.trim();
            if (!text) return;
            close();
            resolve({ session_id: challenge.session_id, source: challenge.source, text });
          } }),
          window.UI.el('button', { class: 'btn ghost', text: 'Cancel', onclick: () => {
            close();
            resolve(null);
          } }),
        ),
      );
      backdrop.append(panel);
      document.body.append(backdrop);
      input.focus();
    });
  }

  document.addEventListener('DOMContentLoaded', () => {
    RailLog.info('DOMContentLoaded: building nav');
    navFor('side-nav');
    navFor('mobile-nav');
    // mark the first available tab active
    const first = NAV.find((t) => window.Tabs[t.id]);
    RailLog.info('first available tab:', first ? first.id : '(none)', 'registered:', Object.keys(window.Tabs).sort().join(', '));
    activate(first ? first.id : NAV[0].id);

    window.Api.sourceStatus().then((s) => {
      if (!s || s.ok === false) {
        RailLog.warn('source-status:', s && s.ok === false ? `${s.status} ${s.error}` : 'no response');
        return;
      }
      const badge = document.getElementById('mode-badge');
      badge.textContent = s.mode || 'live';
      badge.classList.remove('hidden');
      RailLog.info('source-status ok: mode=' + s.mode + ' primary=' + (s.primary_source || '?') +
        ' sources=[' + (s.sources || []).map((x) => `${x.name}:${x.reachable ? 'up' : 'down'}`).join(', ') + ']');
    }).catch((err) => RailLog.error('source-status fetch threw:', err && err.message ? err.message : String(err)));

    initShellSearch();
    RailLog.info('app init complete');
  });
})();
