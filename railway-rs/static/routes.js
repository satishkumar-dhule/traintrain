/* routes.js - pure route table for the hash router. No DOM access; safe to
   import from Node for unit tests (module.exports guard below). The SPA
   sections are Train, Station, Plan, and System; each section has named
   sub-views reachable via deep links like #/train/12559/schedule.

   Two route families per section:
     - section-level:   #/train, #/station, #/plan, #/system  (no entity yet)
     - entity-level:    #/train/{num}[/view], #/station/{code}[/view],
                        #/plan/{src}/{dst}[/view], #/system/{view}
   A section route resolves to the section's default view with empty params. */

(function (global) {
  'use strict';

  const SECTIONS = {
    home:    { views: [], defaultView: null, params: [] },
    train:   { views: ['spot', 'schedule', 'map', 'delay', 'exceptions', 'journey'], defaultView: 'spot', params: ['train'] },
    station: { views: ['live', 'tt', 'heritage', 'parcel'], defaultView: 'live', params: ['station'] },
    plan:    { views: ['trains', 'availability', 'chart'], defaultView: 'trains', params: ['src', 'dst'] },
    system:  { views: ['observability', 'settings', 'debug'], defaultView: 'observability' },
  };

  const NAV_ORDER = ['home', 'train', 'station', 'plan', 'system'];

  const ROUTES = [
    { section: 'home', re: /^\/$/ },
    { section: 'train', re: /^\/train$/ },
    { section: 'train', re: /^\/track$/ },
    { section: 'station', re: /^\/station$/ },
    { section: 'plan', re: /^\/plan$/ },
    { section: 'system', re: /^\/system$/ },
    { section: 'train', re: /^\/train\/([0-9]{1,8})(?:\/([a-z0-9]+))?$/ },
    { section: 'station', re: /^\/station\/(heritage|parcel)$/i, viewOnly: true },
    { section: 'station', re: /^\/station\/([A-Z0-9]{2,4})(?:\/([a-z0-9]+))?$/i },
    { section: 'plan', re: /^\/plan\/([A-Z0-9]{2,4})\/([A-Z0-9]{2,4})(?:\/([a-z0-9]+))?(?:\/(\d{4}-\d{2}-\d{2}))?(?:\/class\/([A-Z0-9]{2,3}))?(?:\/(flex))?(?:\/(berth))?$/i },
    { section: 'system', re: /^\/system\/([a-z0-9]+)$/ },
  ];

  function viewsFor(section) {
    const spec = SECTIONS[section];
    return spec ? spec.views.slice() : [];
  }

  function isValidView(section, view) {
    return !!view && viewsFor(section).includes(view);
  }

  function isAllZeros(s) {
    return /^0+$/.test(s);
  }

  const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;

  /* A real calendar date (not just the right shape: 2026-13-99 is rejected). */
  function isValidDate(s) {
    const m = DATE_RE.exec(s);
    if (!m) return false;
    const d = new Date(Date.UTC(+m[1], +m[2] - 1, +m[3]));
    return d.getUTCFullYear() === +m[1] && d.getUTCMonth() === +m[2] - 1 && d.getUTCDate() === +m[3];
  }

  /* Parse a location hash ("#/train/12559/schedule") into a route object
     { section, view, params } or null when the hash is not a valid route.
     The default view for a section is filled in when the hash omits it. */
  function parse(hash) {
    const h = String(hash || '');
    const raw = h.charAt(0) === '#' ? h.slice(1) : h;
    if (raw === '' || raw === '/') return { section: 'home', view: null, params: {} };
    // backward compat: redirect old #/pnr/XXXXX to train section
    const pnrMatch = /^\/pnr\/(\d{10})$/.exec(raw);
    if (pnrMatch) return { section: 'train', view: null, params: { _pnr: pnrMatch[1] } };
    if (raw === '/pnr') return { section: 'train', view: null, params: {} };
    // backward compat: redirect old #/more/* paths
    const moreMatch = /^\/more\/([a-z0-9]+)$/.exec(raw);
    if (moreMatch) {
      const map = {
        observability: 'system/observability',
        debug: 'system/debug',
        system: 'system/settings',
        heritage: 'station/heritage',
        parcel: 'station/parcel',
        stations: 'station',
      };
      const target = map[moreMatch[1]];
      if (target) return parse('#/' + target);
      return null;
    }
    for (const r of ROUTES) {
      const m = r.re.exec(raw);
      if (!m) continue;
      const spec = SECTIONS[r.section];
      if (r.viewOnly) {
        const view = m[1] && spec.views.includes(m[1].toLowerCase()) ? m[1].toLowerCase() : spec.defaultView;
        return { section: r.section, view, params: {} };
      }
      const params = {};
      (spec.params || []).forEach((name, i) => {
        const val = m[i + 1];
        if (val !== undefined) params[name] = val.toUpperCase();
      });
      if (r.section === 'train' && isAllZeros(params.train)) return null;
      const view = spec.views.length ? (m[(spec.params || []).length + 1] || spec.defaultView) : null;
      if (spec.views.length && view != null && !spec.views.includes(view)) return null;
      if (r.section === 'plan' && m[4] !== undefined) {
        if (!isValidDate(m[4])) return null;
        params.date = m[4];
      }
      if (r.section === 'plan') {
        if (m[5] !== undefined) params.class = m[5].toUpperCase();
        if (m[6] !== undefined) params.flex = '1';
        if (m[7] !== undefined) params.berth = '1';
      }
      return { section: r.section, view, params };
    }
    return null;
  }

  /* Build the canonical hash for a route object. Entity parameters omitted
     collapse to the section-level route (#/train, #/plan, ...); the default
     view is dropped so deep links stay minimal (#/train/12559). */
  function href(route) {
    if (!route || !route.section) return '#/';
    const spec = SECTIONS[route.section];
    if (!spec) return '#/';
    const params = route.params || {};
    const get = (n) => {
      const raw = String(params[n] || '');
      return (n === 'station' || n === 'src' || n === 'dst') ? raw.toUpperCase() : raw;
    };
    const view = route.view && spec.views.includes(route.view) ? route.view : spec.defaultView;
    switch (route.section) {
      case 'home': return '#/';
      case 'train': return get('train')
        ? '#/train/' + encodeURIComponent(get('train')) + (view && view !== spec.defaultView ? '/' + view : '')
        : '#/train';
      case 'station': {
        const code = get('station');
        if (code) return '#/station/' + encodeURIComponent(code) + (view && view !== spec.defaultView ? '/' + view : '');
        if (view && view !== spec.defaultView && ['heritage', 'parcel'].includes(view)) return '#/station/' + view;
        return '#/station';
      }
      case 'plan': {
        if (!(get('src') && get('dst'))) return '#/plan';
        let hash = '#/plan/' + encodeURIComponent(get('src')) + '/' + encodeURIComponent(get('dst')) +
          (view && view !== spec.defaultView ? '/' + view : '');
        const date = String(params.date || '');
        if (isValidDate(date)) hash += '/' + encodeURIComponent(date);
        const cls = String(params.class || '').toUpperCase();
        if (cls && /^[A-Z0-9]{2,3}$/.test(cls)) hash += '/class/' + cls;
        if (params.flex) hash += '/flex';
        if (params.berth) hash += '/berth';
        return hash;
      }
      case 'system': return view && view !== spec.defaultView ? '#/system/' + view : '#/system';
    }
    return '#/';
  }

  /* Today's date as YYYY-MM-DD (local), matching ui.today() so the canonical
     plan hash carries the same date the search form would submit. */
  function today() {
    const d = new Date();
    const mm = String(d.getMonth() + 1).padStart(2, '0');
    const dd = String(d.getDate()).padStart(2, '0');
    return d.getFullYear() + '-' + mm + '-' + dd;
  }

  /* Canonical form of a hash: parse, normalize (uppercase stations, default
     views) and, for a plan route missing its journey date, inject today.
     Returns null when the hash is empty, invalid, or already canonical. */
  function canonical(hash, date) {
    const h = String(hash || '');
    const raw = h.charAt(0) === '#' ? h.slice(1) : h;
    if (raw === '') return null;
    const route = parse(h);
    if (!route) return null;
    if (route.params._pnr) return null;
    if (route.section === 'plan' && route.params.src && route.params.dst && !route.params.date) {
      const d = date || today();
      if (isValidDate(d)) route.params = Object.assign({}, route.params, { date: d });
    }
    const out = href(route);
    return out === h ? null : out;
  }

  /* ---------- DOM boot: canonical URL rewriting (browser only) ---------- */

  const bootedWindows = new WeakSet();

  /* Rewrite location.hash to its canonical form. The initial deep link is
     pushed (the original hash stays in history, so Back returns to it or the
     previous page); later hashchanges are replaced in place so Back/forward
     can never grow history or bounce. No-op when the hash is already
     canonical, so the push can never trigger a redirect loop. */
  function canonicalize(replace) {
    if (!window || !window.location) return;
    const canon = canonical(window.location.hash);
    if (!canon) return;
    const history = window.history;
    if (!history) return;
    const fn = replace ? history.replaceState : history.pushState;
    if (typeof fn !== 'function') return;
    fn.call(history, null, '', canon);
  }

  /* Boot the router: canonicalize the initial hash and canonicalize every
     later hashchange. Idempotent — booting again with an already-canonical
     hash adds no history entry. */
  function boot() {
    if (window && typeof window.addEventListener === 'function' && !bootedWindows.has(window)) {
      bootedWindows.add(window);
      window.addEventListener('hashchange', () => canonicalize(true));
    }
    canonicalize(false);
    return Promise.resolve();
  }

  const api = { SECTIONS, NAV_ORDER, parse, href, canonical, boot, viewsFor, isValidView };

  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else global.Routes = api;

  /* Auto-boot when loaded in a real DOM (hash canonicalization at startup).
     Skips Node imports and test fakes whose history lacks pushState. */
  if (typeof window !== 'undefined' && window.location && window.history &&
      typeof window.history.pushState === 'function' && typeof window.addEventListener === 'function') {
    boot();
  }
})(typeof window !== 'undefined' ? window : globalThis);
