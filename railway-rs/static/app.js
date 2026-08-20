/* app.js - shell: hash router over the Routes table, section nav (5 primary
   sections: home, train, station, plan, system), and section mounting. Sections
   live in static/sections/*.js and expose window.Sections.<id> with
   mount(container, ctx, route). */

window.Tabs = window.Tabs || {};

(() => {
  const UI = window.UI;

  const SECTIONS = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'train', label: 'Train', icon: 'train' },
    { id: 'station', label: 'Station', icon: 'station' },
    { id: 'plan', label: 'Plan', icon: 'plan' },
    { id: 'system', label: 'System', icon: 'system' },
  ];

  const VIEW_LABELS = {
    spot: 'Spot', schedule: 'Schedule', map: 'Map', delay: 'Delay',
    exceptions: 'Exceptions', journey: 'Journey',
    live: 'Live', tt: 'Timetable', heritage: 'Heritage', parcel: 'Parcel',
    trains: 'Trains', availability: 'Availability', chart: 'Chart',
    observability: 'Observability', settings: 'Settings', debug: 'Debug',
  };

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
      renderSidebarFavs();
      favListeners.forEach((fn) => fn(added, type, code));
      return added;
    },
    onchange(fn) { favListeners.push(fn); },
    update(type, code, label) {
      const hit = state.favs.findIndex((f) => favKey(f.type, f.code) === favKey(type, code));
      if (hit < 0 || !label) return;
      state.favs[hit].label = label;
      saveFavs();
      renderSidebarFavs();
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
        renderSidebarFavs();
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

  function buildNav(containerId) {
    const nav = document.getElementById(containerId);
    if (!nav) return;
    SECTIONS.forEach((s) => {
      const btn = UI.el('button', {
        class: 'nav-item',
        'data-section': s.id,
        onclick: () => navigate(Routes.href({ section: s.id })),
      });
      btn.append(
        UI.el('span', { class: 'nav-icon', 'aria-hidden': 'true' }, UI.icon(s.icon)),
        UI.el('span', { text: s.label }),
      );
      nav.append(btn);
    });
  }

  function updateNav(route) {
    document.querySelectorAll('.nav-item').forEach((n) => n.classList.remove('active'));
    const hit = document.querySelector(`[data-section="${route.section}"]`);
    if (hit) hit.classList.add('active');
  }

  /* ---------- Rendering ---------- */

  function render(route) {
    const root = document.getElementById('tab-root');
    if (!root) return;
    const main = document.getElementById('main');
    updateNav(route);

    if (state.lastSection && state.lastSection !== route.section && main) {
      state.scrollMemo = {};
      main.scrollTop = 0;
    }
    state.lastSection = route.section;

    RailLog.info('route:', location.hash || '#/', '->', route.section, route.view || '', route.params || {});

    const section = window.Sections[route.section];
    if (!section) {
      UI.render(root, UI.errorState(`Section "${route.section}" is not wired up yet.`));
      return;
    }

    const content = UI.el('div', { class: 'tab-content' });
    UI.render(root, content);
    section.mount(content, ctx(), route);
    recordRecent(route);

    requestAnimationFrame(() => {
      if (main) main.scrollTop = state.scrollMemo[location.hash] || 0;
    });
  }

  function entityLabel(route) {
    const p = route.params || {};
    if (route.section === 'train') return 'Train ' + (p.train || '');
    if (route.section === 'station') return 'Station ' + (p.station || '');
    if (route.section === 'plan') return p.src + ' → ' + p.dst;
    return '';
  }

  /* ---------- Sidebar favorites ---------- */

  function renderSidebarFavs() {
    const box = document.getElementById('side-favs');
    if (!box) return;
    const list = state.favs.slice(0, 12);
    if (!list.length) { box.classList.add('hidden'); box.replaceChildren(); return; }
    box.classList.remove('hidden');
    box.replaceChildren(UI.el('div', { class: 'side-favs-label', text: 'Favorites' }));
    const rows = UI.el('div', { class: 'col', style: 'gap:3px;' });
    list.forEach((f) => {
      rows.append(UI.el('button', {
        class: 'recent-item',
        onclick: () => navigate(favHash(f)),
        'aria-label': 'Open ' + f.label,
      },
        UI.el('span', { class: 'recent-label' },
          UI.icon(f.type === 'train' ? 'train' : 'station'),
          ' ' + f.label),
        UI.icon('star-fill', 'fav-star'),
      ));
    });
    box.append(rows);
  }

  function favHash(f) {
    if (f.type === 'train') return Routes.href({ section: 'train', params: { train: f.code } });
    if (f.type === 'station') return Routes.href({ section: 'station', params: { station: f.code } });
    return '#/';
  }

  /* ---------- Shell search ---------- */

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
        menu.append(UI.el('div', { class: 'ac-group', text: 'No results' }));
        menu.classList.remove('hidden');
        return;
      }
      const groups = [
        ['Stations', items.filter((it) => it.type === 'station')],
        ['Trains', items.filter((it) => it.type === 'train')],
      ];
      groups.forEach(([label, list]) => {
        if (!list.length) return;
        menu.append(UI.el('div', { class: 'ac-group', text: label }));
        list.forEach((it) => {
          const row = UI.el('div', {
            class: 'ac-item',
            onmousedown: (e) => { e.preventDefault(); select(it); },
          });
          row.append(
            UI.el('span', { class: 'ac-code', text: it.code || it.number }),
            UI.el('span', { class: 'ac-name', text: it.name }),
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

    const debouncedSearch = UI.debounce(search, 250);

    function select(it) {
      if (it.type === 'station') navigate(Routes.href({ section: 'station', params: { station: it.code } }));
      else navigate(Routes.href({ section: 'train', params: { train: it.number } }));
      input.value = '';
      closeMenu();
    }

    input.addEventListener('input', debouncedSearch);
    input.addEventListener('focus', () => {
      if (input.value.trim()) debouncedSearch();
    });
    input.addEventListener('blur', () => setTimeout(closeMenu, 150));
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') { closeMenu(); input.blur(); }
      else if (e.key === 'ArrowDown' && items.length) { e.preventDefault(); hl = (hl + 1) % items.length; updateHighlight(); }
      else if (e.key === 'ArrowUp' && items.length) { e.preventDefault(); hl = (hl - 1 + items.length) % items.length; updateHighlight(); }
      else if (e.key === 'Enter' && hl >= 0 && items[hl]) { e.preventDefault(); select(items[hl]); }
    });
    document.addEventListener('mousedown', (e) => { if (!wrap.contains(e.target)) closeMenu(); });
  }

  /* ---------- Theme ---------- */

  function syncThemeIcons() {
    const theme = window.AppTheme;
    if (!theme) return;
    const btn = document.getElementById('theme-toggle');
    if (btn) btn.replaceChildren(UI.icon(theme.icon()));
  }

  function initTheme() {
    const t = document.getElementById('theme-toggle');
    if (t) t.addEventListener('click', () => { window.AppTheme.toggle(); syncThemeIcons(); });
    syncThemeIcons();
  }

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

  /* ---------- Keyboard shortcuts ---------- */

  function initKeyboard() {
    document.addEventListener('keydown', (e) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const tag = e.target && e.target.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (e.key === '/') {
        e.preventDefault();
        const input = document.getElementById('shell-search-input');
        if (input) input.focus();
      }
    });
  }

  /* ---------- Boot ---------- */

  document.addEventListener('DOMContentLoaded', () => {
    buildNav('side-nav');
    buildNav('mobile-nav');
    renderSidebarFavs();
    initTheme();
    initKeyboard();

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