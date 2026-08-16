/* routes.js - pure route table for the hash router. No DOM access; safe to
   import from Node for unit tests (module.exports guard below). The SPA
   sections are Home, Track, Station, Plan, PNR plus a "More" menu; each
   section has named sub-views reachable via deep links like #/train/12559/map.

   Two route families per section:
     - section-level:   #/track, #/station, #/plan, #/pnr, #/more  (no entity yet)
     - entity-level:    #/train/{num}[/view], #/station/{code}[/view],
                        #/plan/{src}/{dst}[/view], #/pnr/{pnr}
   A section route resolves to the section's default view with empty params. */

(function (global) {
  'use strict';

  const SECTIONS = {
    home:    { views: [] },
    track:   { views: ['spot', 'schedule', 'map', 'delay', 'exceptions', 'journey'], defaultView: 'spot', params: ['train'] },
    station: { views: ['live', 'tt'], defaultView: 'live', params: ['station'] },
    plan:    { views: ['trains', 'availability', 'chart'], defaultView: 'trains', params: ['src', 'dst'] },
    pnr:     { views: [], params: ['pnr'] },
    more:    { views: ['heritage', 'parcel', 'stations', 'system', 'observability', 'debug'], defaultView: null },
  };

  const NAV_ORDER = ['home', 'track', 'station', 'plan', 'pnr'];

  const ROUTES = [
    { section: 'home', re: /^\/$/ },
    { section: 'pnr', re: /^\/pnr$/ },
    { section: 'track', re: /^\/track$/ },
    { section: 'station', re: /^\/station$/ },
    { section: 'plan', re: /^\/plan$/ },
    { section: 'more', re: /^\/more$/ },
    { section: 'pnr', re: /^\/pnr\/(\d{10})$/ },
    { section: 'track', re: /^\/train\/([0-9]{1,8})(?:\/([a-z0-9]+))?$/ },
    { section: 'station', re: /^\/station\/([A-Z0-9]{2,4})(?:\/([a-z0-9]+))?$/i },
    { section: 'plan', re: /^\/plan\/([A-Z0-9]{2,4})\/([A-Z0-9]{2,4})(?:\/([a-z0-9]+))?$/i },
    { section: 'more', re: /^\/more\/([a-z0-9]+)$/ },
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

  /* Parse a location hash ("#/train/12559/schedule") into a route object
     { section, view, params } or null when the hash is not a valid route.
     The default view for a section is filled in when the hash omits it. */
  function parse(hash) {
    const h = String(hash || '');
    const raw = h.charAt(0) === '#' ? h.slice(1) : h;
    if (raw === '' || raw === '/') return { section: 'home', view: null, params: {} };
    for (const r of ROUTES) {
      const m = r.re.exec(raw);
      if (!m) continue;
      const spec = SECTIONS[r.section];
      const params = {};
      (spec.params || []).forEach((name, i) => {
        const val = m[i + 1];
        if (val !== undefined) params[name] = val.toUpperCase();
      });
      if (r.section === 'track' && isAllZeros(params.train)) return null;
      const view = spec.views.length ? (m[(spec.params || []).length + 1] || spec.defaultView) : null;
      if (spec.views.length && view != null && !spec.views.includes(view)) return null;
      return { section: r.section, view, params };
    }
    return null;
  }

  /* Build the canonical hash for a route object. Entity parameters omitted
     collapse to the section-level route (#/track, #/plan, ...); the default
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
      case 'pnr': return get('pnr') ? '#/pnr/' + encodeURIComponent(get('pnr')) : '#/pnr';
      case 'track': return get('train')
        ? '#/train/' + encodeURIComponent(get('train')) + (view && view !== spec.defaultView ? '/' + view : '')
        : '#/track';
      case 'station': return get('station')
        ? '#/station/' + encodeURIComponent(get('station')) + (view && view !== spec.defaultView ? '/' + view : '')
        : '#/station';
      case 'plan': return (get('src') && get('dst'))
        ? '#/plan/' + encodeURIComponent(get('src')) + '/' + encodeURIComponent(get('dst')) +
          (view && view !== spec.defaultView ? '/' + view : '')
        : '#/plan';
      case 'more': return view ? '#/more/' + view : '#/more';
    }
    return '#/';
  }

  const api = { SECTIONS, NAV_ORDER, parse, href, viewsFor, isValidView };

  if (typeof module !== 'undefined' && module.exports) module.exports = api;
  else global.Routes = api;
})(typeof window !== 'undefined' ? window : globalThis);
