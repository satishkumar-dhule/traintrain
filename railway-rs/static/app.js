/* app.js - application shell for Train Bro v5 ("Transit").
   Responsibilities:
     - hash router over the Routes table (sections: home/train/station/plan/system)
     - section mounting with view transitions, scroll memory and focus management
   Sections live in static/sections/*.js and expose window.Sections.<id> with
   mount(container, ctx, route). v5 adds persistent chrome — a topbar (desktop
   nav + search + theme toggle) and a mobile tabbar — alongside the command
   palette (palette.js), which remains the primary search/command surface.
   app.js syncs active states on [data-nav] links and wires the topbar buttons;
   the keyboard shortcuts below are unchanged. */

window.Tabs = window.Tabs || {};

(() => {
  const UI = window.UI;

  const SECTIONS = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'train', label: 'Trains', icon: 'train' },
    { id: 'station', label: 'Stations', icon: 'station' },
    { id: 'plan', label: 'Journeys', icon: 'plan' },
    { id: 'system', label: 'System', icon: 'system' },
  ];

  const RECENT_KEY = 'rc.recent';
  const RECENT_MAX = 8;
  const FAV_KEY = 'rc.favs';

  const state = { ctx: null, recent: loadRecent(), scrollMemo: {}, lastSection: null };

  /* ---------- localStorage helpers ---------- */

  function loadRecent() {
    try {
      const raw = localStorage.getItem(RECENT_KEY);
      const arr = raw ? JSON.parse(raw) : [];
      return Array.isArray(arr) ? arr : [];
    } catch { return []; }
  }

  function saveRecent() {
    try { localStorage.setItem(RECENT_KEY, JSON.stringify(state.recent.slice(0, RECENT_MAX))); } catch { /* private mode */ }
  }

  /* Favorites: [{ type: 'train'|'station', code, label, ts }] */
  function loadFavs() {
    try {
      const raw = localStorage.getItem(FAV_KEY);
      const arr = raw ? JSON.parse(raw) : [];
      return Array.isArray(arr) ? arr : [];
    } catch { return []; }
  }

  function saveFavs() {
    try { localStorage.setItem(FAV_KEY, JSON.stringify(state.favs)); } catch { /* private mode */ }
  }

  function favKey(type, code) { return type + ':' + String(code).toUpperCase(); }

  const favListeners = [];
  const fav = {
    list: () => state.favs.slice(),
    has: (type, code) => state.favs.some((f) => f.type === type && f.code.toUpperCase() === String(code).toUpperCase()),
    toggle(type, code, label) {
      const key = favKey(type, code);
      const hit = state.favs.findIndex((f) => favKey(f.type, f.code) === key);
      let added = false;
      if (hit >= 0) {
        state.favs.splice(hit, 1);
      } else {
        state.favs.unshift({ type, code: String(code).toUpperCase(), label: label || `${type} ${code}`, ts: Date.now() });
        added = true;
      }
      state.favs = state.favs.slice(0, 40);
      saveFavs();
      favListeners.forEach((fn) => fn(added, type, code));
      return added;
    },
    onchange(fn) { favListeners.push(fn); },
    update(type, code, label) {
      const hit = state.favs.findIndex((f) => favKey(f.type, f.code) === favKey(type, code));
      if (hit < 0 || !label) return;
      state.favs[hit].label = label;
      saveFavs();
    },
  };

  state.favs = loadFavs();

  /* In-flight guard so we never fetch the same schedule twice. */
  const enriching = {};

  /* Background-enrich a train's recent/favorite label with
     "Train N · NAME (FROM → TO)" once schedule data is available. Fires on
     navigation and again on the Home page so stored bare labels self-heal. */
  function enrichTrain(num, hash, onDone) {
    const code = String(num).trim();
    if (!code || !/^\d+$/.test(code) || enriching[code]) return;
    enriching[code] = true;
    window.Api.schedule(code)
      .then((res) => {
        if (!res || res.ok === false) return;
        const stops = Array.isArray(res.stops) ? res.stops : [];
        const from = stops[0];
        const to = stops[stops.length - 1];
        const label = 'Train ' + code
          + (res.train_name ? ' \u00b7 ' + res.train_name : '')
          + (from && to ? ' (' + from.code + ' \u2192 ' + to.code + ')' : '');
        const h = hash || Routes.href({ section: 'train', params: { train: code } });
        const recHit = state.recent.findIndex((r) => r.hash === h);
        if (recHit >= 0) state.recent[recHit].label = label;
        const favHit = state.favs.findIndex((f) => favKey(f.type, f.code) === favKey('train', code));
        if (favHit >= 0) state.favs[favHit].label = label;
        saveRecent();
        saveFavs();
        favListeners.forEach((fn) => fn(null, 'train', code));
        if (onDone) onDone();
      })
      .catch(() => {})
      .finally(() => { delete enriching[code]; });
  }

  function recordRecent(route) {
    const p = route.params || {};
    if (!p.train && !p.station && !(p.src && p.dst)) return;
    const label = entityLabel(route);
    const hash = Routes.href(route);
    state.recent = [
      { label, hash, ts: Date.now() },
      ...state.recent.filter((r) => r.hash !== hash),
    ].slice(0, RECENT_MAX);
    saveRecent();
    if (p.train) enrichTrain(p.train, hash);
  }

  function ctx() {
    if (!state.ctx) {
      state.ctx = {
        api: window.Api,
        ui: UI,
        autocomplete: window.AutoComplete,
        captcha: { show: showCaptcha },
        navigate,
        recent: {
          list: () => state.recent.slice(),
          clear: () => { state.recent = []; saveRecent(); },
          update: (hash, label) => {
            const hit = state.recent.findIndex((r) => r.hash === hash);
            if (hit < 0 || !label) return;
            state.recent[hit].label = label;
            saveRecent();
          },
        },
        fav,
        copyLink: UI.copyLink,
        share: UI.share,
        enrichTrain,
        openSearch: () => window.Palette && window.Palette.open(),
        theme: window.AppTheme || { current: () => 'system', set: () => {}, toggle: () => {} },
      };
      window._appCtx = () => state.ctx;
    }
    return state.ctx;
  }

  /* ---------- Navigation ---------- */

  function navigate(hash) {
    const target = String(hash || '#/');
    if (location.hash === target) render(Routes.parse(target));
    else location.hash = target;
  }

  function onHashChange() {
    const route = Routes.parse(location.hash);
    if (!route) {
      const prev = location.hash;
      location.hash = '#/';
      if (location.hash === prev) render(Routes.parse('#/'));
      return;
    }
    rememberScroll();
    render(route);
  }

  function rememberScroll() {
    const main = document.getElementById('main');
    if (main && location.hash) state.scrollMemo[location.hash] = main.scrollTop;
  }

  /* ---------- Rendering ---------- */

  /* Keep topnav/tabbar chrome in sync with the active section. */
  function syncChrome(route) {
    document.querySelectorAll('[data-nav]').forEach((el) => {
      const active = el.dataset.nav === route.section;
      el.classList.toggle('active', active);
      if (active) el.setAttribute('aria-current', 'page');
      else el.removeAttribute('aria-current');
    });
  }

  function reducedMotion() {
    try {
      return !!(window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches);
    } catch { return false; }
  }

  function swapContent(route) {
    const root = document.getElementById('tab-root');
    if (!root) return;
    const content = UI.el('div', { class: 'tab-content reveal' });
    UI.render(root, content);
    const section = window.Sections[route.section];
    if (!section) {
      UI.render(content, UI.errorState(`Section "${route.section}" is not wired up yet.`));
      return;
    }
    section.mount(content, ctx(), route);
  }

  function render(route) {
    const main = document.getElementById('main');

    if (state.lastSection && state.lastSection !== route.section && main) {
      state.scrollMemo = {};
    }
    state.lastSection = route.section;
    syncChrome(route);

    RailLog.info('route:', location.hash || '#/', '->', route.section, route.view || '', route.params || {});

    const canTransition = typeof document.startViewTransition === 'function' && !reducedMotion();
    if (canTransition) {
      document.startViewTransition(() => swapContent(route));
    } else {
      swapContent(route);
    }
    recordRecent(route);

    requestAnimationFrame(() => {
      if (main) main.scrollTop = state.scrollMemo[location.hash] || 0;
      /* Focus management for screen readers: move focus to the content region
         unless a section already placed focus (e.g. an autofocused input). */
      if (main && (!document.activeElement || document.activeElement === document.body)) {
        try { main.focus({ preventScroll: true }); } catch { main.focus(); }
      }
    });
  }

  function entityLabel(route) {
    const p = route.params || {};
    if (route.section === 'train') return 'Train ' + (p.train || '');
    if (route.section === 'station') return 'Station ' + (p.station || '');
    if (route.section === 'plan') return p.src + ' → ' + p.dst;
    return '';
  }

  /* ---------- Theme ---------- */

  /* ---------- CAPTCHA (dialog-based) ---------- */

  function showCaptcha(challenge) {
    const input = UI.el('input', { class: 'input', autocomplete: 'off', placeholder: 'Answer (e.g. 44)' });
    return UI.dialog({
      title: 'Captcha required (' + challenge.source + ')',
      body: [
        UI.el('img', { src: challenge.image, alt: 'captcha' }),
        UI.el('p', { class: 'text-sm muted', text: 'It is an arithmetic question (e.g. "4 + 40 = ?" → 44). Type the answer:' }),
        input,
      ],
      actions: [
        { label: 'Submit', primary: true, value: '__submit' },
        { label: 'Cancel', primary: false, value: null },
      ],
    }).then((v) => {
      if (v !== '__submit') return null;
      const text = String(input.value || '').trim();
      if (!text) return null;
      return { session_id: challenge.session_id, source: challenge.source, text };
    });
  }

  /* ---------- Keyboard shortcuts ----------
     ⌘K / Ctrl+K or /  → command palette (wired in palette.js too)
     1..5              → jump to section
     t                 → toggle theme
     ?                 → shortcut help                                    */

  function initKeyboard() {
    document.addEventListener('keydown', (e) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const tag = e.target && e.target.tagName;
      const typing = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
      if (typing) return;

      if (e.key === '/') {
        e.preventDefault();
        if (window.Palette) window.Palette.open();
        return;
      }
      if (e.key === '?') {
        e.preventDefault();
        showShortcuts();
        return;
      }
      if (e.key === 't' || e.key === 'T') {
        if (window.AppTheme) window.AppTheme.toggle();
        return;
      }
      const n = parseInt(e.key, 10);
      if (n >= 1 && n <= SECTIONS.length) {
        navigate(Routes.href({ section: SECTIONS[n - 1].id }));
      }
    });
  }

  function showShortcuts() {
    const kbd = (k) => UI.el('kbd', { class: 'pi-kbd', text: k });
    const row = (keys, desc) => UI.el('div', { class: 'row justify-between', style: 'padding:4px 0;' },
      UI.el('span', { class: 'text-sm', text: desc }),
      UI.el('span', { class: 'row', style: 'gap:4px;' }, ...keys.map(kbd)));
    UI.dialog({
      title: 'Keyboard shortcuts',
      body: [
        row(['⌘K', '/'], 'Search & commands'),
        row(['1', '…', '5'], 'Jump to section'),
        row(['t'], 'Toggle theme'),
        row(['?'], 'This help'),
      ],
      actions: [{ label: 'Got it', primary: true, value: true }],
    });
  }

  /* ---------- Boot ---------- */

  document.addEventListener('DOMContentLoaded', () => {
    initKeyboard();

    const searchBtn = document.getElementById('topbar-search');
    if (searchBtn) searchBtn.addEventListener('click', () => { if (window.Palette) window.Palette.open(); });

    const themeBtn = document.getElementById('theme-toggle');
    function syncThemeIcon() {
      const use = document.getElementById('theme-toggle-use');
      const t = window.AppTheme;
      if (use && t) use.setAttribute('href', '/icons.svg#' + (t.icon() === 'sun' ? 'i-sun' : 'i-moon'));
    }
    if (themeBtn) themeBtn.addEventListener('click', () => { if (window.AppTheme) window.AppTheme.toggle(); });
    if (window.AppTheme && window.AppTheme.onChange) window.AppTheme.onChange(syncThemeIcon);
    syncThemeIcon();

    const initial = Routes.parse(location.hash);
    if (!initial) {
      location.hash = '#/';
      render(Routes.parse('#/'));
    } else {
      render(initial);
    }
    window.addEventListener('hashchange', onHashChange);

    window.Api.sourceStatus().then((s) => {
      if (!s || s.ok === false) {
        RailLog.warn('source-status:', s && s.ok === false ? `${s.status} ${s.error}` : 'no response');
        return;
      }
      RailLog.info('source-status ok: mode=' + s.mode + ' primary=' + (s.primary_source || '?') +
        ' sources=[' + (s.sources || []).map((x) => `${x.name}:${x.reachable ? 'up' : 'down'}`).join(', ') + ']');
    }).catch((err) => RailLog.error('source-status fetch threw:', err && err.message ? err.message : String(err)));

    RailLog.info('app init complete');
  });
})();
