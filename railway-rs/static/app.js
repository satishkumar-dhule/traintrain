/* app.js - shell: hash router over the Routes table, section nav (5 primary
   sections + a "More" menu), and section mounting. Sections live in
   static/sections/*.js (home, track, station, plan, pnr, more) and expose
   window.Sections.<id> with mount(container, ctx, route). */

window.Tabs = window.Tabs || {};

(() => {
  const SECTIONS = [
    { id: 'home', label: 'Home', icon: '🏠' },
    { id: 'track', label: 'Track', icon: '🚄' },
    { id: 'station', label: 'Station', icon: '🚉' },
    { id: 'plan', label: 'Plan', icon: '📍' },
    { id: 'pnr', label: 'PNR', icon: '🎫' },
  ];

  const MORE = [
    { id: 'heritage', label: 'Heritage', icon: '🚞' },
    { id: 'parcel', label: 'Parcel SPL', icon: '📦' },
    { id: 'stations', label: 'Stations', icon: '🗺️' },
    { id: 'system', label: 'System', icon: '⚙️' },
    { id: 'observability', label: 'Observability', icon: '📊' },
    { id: 'debug', label: 'Debug', icon: '🐞' },
  ];

  const VIEW_LABELS = {
    spot: 'Spot', schedule: 'Schedule', map: 'Map', delay: 'Delay',
    exceptions: 'Exceptions', journey: 'Journey',
    live: 'Live', tt: 'Timetable',
    trains: 'Trains', availability: 'Availability', chart: 'Chart',
  };

  const RECENT_KEY = 'rc.recent';
  const RECENT_MAX = 8;

  const state = { ctx: null, recent: loadRecent() };

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

  function recordRecent(route) {
    const p = route.params || {};
    if (!p.train && !p.station && !(p.src && p.dst) && !p.pnr) return;
    const label = entityLabel(route);
    const hash = Routes.href(route);
    state.recent = [
      { label, hash, ts: Date.now() },
      ...state.recent.filter((r) => r.hash !== hash),
    ].slice(0, RECENT_MAX);
    saveRecent();
  }

  function ctx() {
    if (!state.ctx) {
      state.ctx = {
        api: window.Api,
        ui: window.UI,
        autocomplete: window.AutoComplete,
        captcha: { show: showCaptcha },
        navigate,
        recent: {
          list: () => state.recent.slice(),
          clear: () => { state.recent = []; saveRecent(); },
        },
      };
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
    render(route);
  }

  function buildNav(containerId) {
    const nav = document.getElementById(containerId);
    if (!nav) return;
    const items = containerId === 'side-nav'
      ? SECTIONS
      : [...SECTIONS, { id: 'more', label: 'More', icon: '⋯' }];
    items.forEach((s) => {
      const btn = window.UI.el('button', {
        class: 'nav-item',
        'data-section': s.id,
        onclick: () => navigate(Routes.href({ section: s.id })),
      });
      btn.append(
        window.UI.el('span', { class: 'nav-icon', text: s.icon }),
        window.UI.el('span', { text: s.label }),
      );
      nav.append(btn);
    });
    if (containerId === 'side-nav') {
      nav.append(window.UI.el('div', { class: 'nav-group', text: 'More' }));
      MORE.forEach((m) => {
        const btn = window.UI.el('button', {
          class: 'nav-item',
          'data-more': m.id,
          onclick: () => navigate('#/more/' + m.id),
        });
        btn.append(
          window.UI.el('span', { class: 'nav-icon', text: m.icon }),
          window.UI.el('span', { text: m.label }),
        );
        nav.append(btn);
      });
    }
  }

  function updateNav(route) {
    document.querySelectorAll('.nav-item').forEach((n) => n.classList.remove('active'));
    const section = route.section;
    const hit = document.querySelector(`[data-section="${section}"]`);
    if (hit) hit.classList.add('active');
    if (section === 'more' && route.view) {
      const m = document.querySelector(`[data-more="${route.view}"]`);
      if (m) m.classList.add('active');
    }
  }

  /* ---------- Rendering ---------- */

  function render(route) {
    const root = document.getElementById('tab-root');
    if (!root) return;
    updateNav(route);
    RailLog.info('route:', location.hash || '#/', '->', route.section, route.view || '', route.params || {});

    const section = window.Sections[route.section];
    if (!section) {
      window.UI.render(root, window.UI.errorBox(`Section "${route.section}" is not wired up yet.`));
      return;
    }

    if (route.section === 'home' || (route.section === 'more' && !route.view)) {
      section.mount(root, ctx(), route);
      recordRecent(route);
      return;
    }

    const content = window.UI.el('div', { class: 'tab-content' });
    window.UI.render(root, buildSectionHeader(route), content);
    section.mount(content, ctx(), route);
    recordRecent(route);
  }

  function buildSectionHeader(route) {
    const ui = window.UI;
    const bar = ui.el('div', { class: 'section-bar' });
    if (route.section === 'pnr') return bar;
    const views = Routes.viewsFor(route.section);
    const params = route.params || {};
    const hasEntity = !!(params.train || params.station || (params.src && params.dst));
    if (!views.length || !hasEntity) return bar;
    bar.append(ui.el('span', { class: 'section-title', text: entityLabel(route) }));
    const pills = ui.el('div', { class: 'section-pills' });
    views.forEach((v) => {
      pills.append(ui.el('button', {
        class: 'section-pill' + (v === route.view ? ' active' : ''),
        text: VIEW_LABELS[v] || v,
        onclick: () => navigate(Routes.href({ section: route.section, view: v, params })),
      }));
    });
    bar.append(pills);
    return bar;
  }

  function entityLabel(route) {
    const p = route.params || {};
    if (route.section === 'track') return 'Train ' + (p.train || '');
    if (route.section === 'station') return 'Station ' + (p.station || '');
    if (route.section === 'plan') return p.src + ' → ' + p.dst;
    return '';
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
      if (it.type === 'station') navigate(Routes.href({ section: 'station', params: { station: it.code } }));
      else navigate(Routes.href({ section: 'track', params: { train: it.number } }));
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

  /* ---------- CAPTCHA ---------- */

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

  /* ---------- Boot ---------- */

  document.addEventListener('DOMContentLoaded', () => {
    buildNav('side-nav');
    buildNav('mobile-nav');

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
