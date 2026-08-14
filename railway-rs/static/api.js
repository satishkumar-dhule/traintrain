/* api.js - thin fetch layer for /rail-api/*. Every function returns a Promise
   resolving to the parsed JSON body. Non-2xx responses resolve to
   { ok:false, status, error, body } so tabs can render honest failures. */

window.Api = (() => {
  async function request(path, opts = {}) {
    const method = (opts.method || 'GET').toUpperCase();
    const t0 = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
    let res;
    try {
      res = await fetch(path, opts);
    } catch (err) {
      RailLog.error('api fetch threw:', method, path, err && err.message ? err.message : String(err));
      throw err;
    }
    let body = null;
    const text = await res.text();
    if (text) {
      try { body = JSON.parse(text); } catch { body = text; }
    }
    const elapsed = Math.round(((typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now()) - t0);
    if (!res.ok) {
      const err = (body && typeof body === 'object' && body.error) ? body.error : `HTTP ${res.status}`;
      RailLog.warn(`api ${method} ${path} -> ${res.status} (${elapsed}ms) error: ${err}`);
      return { ok: false, status: res.status, error: err, body };
    }
    RailLog.info(`api ${method} ${path} -> ${res.status} (${elapsed}ms)`);
    return body;
  }

  const get = (path) => request(path);

  return {
    request,
    sourceStatus: () => get('/rail-api/source-status'),

    pnr: (pnr, captcha) => {
      let q = `/rail-api/pnr?pnr=${encodeURIComponent(pnr)}`;
      if (captcha) {
        q += `&captcha_session=${encodeURIComponent(captcha.session_id)}` +
             `&captcha_text=${encodeURIComponent(captcha.text)}` +
             `&captcha_source=${encodeURIComponent(captcha.source)}`;
      }
      return get(q);
    },

    schedule: (train) => get(`/rail-api/schedule?train=${encodeURIComponent(train)}`),
    liveStatus: (train, date) =>
      get(`/rail-api/live-status?train=${encodeURIComponent(train)}` + (date ? `&date=${encodeURIComponent(date)}` : '')),
    liveStation: (station, hours) =>
      get(`/rail-api/ntes/live-station?station=${encodeURIComponent(station)}&hours=${hours || 2}`),
    trainsBetween: (src, dst) =>
      get(`/rail-api/ntes/trains-between?src=${encodeURIComponent(src)}&dst=${encodeURIComponent(dst)}`),
    exceptional: (type) => get(`/rail-api/ntes/exceptional?type=${encodeURIComponent(type)}`),
    stations: (q) => get(`/rail-api/stations?q=${encodeURIComponent(q)}`),
    searchTrains: (q) => get(`/rail-api/search/trains?q=${encodeURIComponent(q)}`),
    searchStations: (q) => get(`/rail-api/search/stations?q=${encodeURIComponent(q)}`),
    observability: () => get('/rail-api/observability'),
  };
})();
