/* api.js - thin fetch layer for /rail-api/*. Every function returns a Promise
   resolving to the parsed JSON body. Non-2xx responses resolve to
   { ok:false, status, error, body } so tabs can render honest failures. */

window.Api = (() => {
  async function request(path, opts = {}) {
    const { timeout = 12000, signal: callerSignal, ...fetchOpts } = opts;
    const method = (opts.method || 'GET').toUpperCase();
    const t0 = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
    const reqBody = opts.body
      ? String(typeof opts.body === 'string' ? opts.body : JSON.stringify(opts.body)).slice(0, 400)
      : undefined;
    const controller = new AbortController();
    if (callerSignal) {
      if (callerSignal.aborted) controller.abort();
      else callerSignal.addEventListener('abort', () => controller.abort(), { once: true });
    }
    let timedOutFlag = false;
    const timeoutError = () => {
      const e = new Error(`Request timed out after ${timeout}ms`);
      e.code = 'TIMEOUT';
      return e;
    };
    const timer = setTimeout(() => { timedOutFlag = true; controller.abort(); }, timeout);
    const timedOut = new Promise((_, reject) => {
      controller.signal.addEventListener('abort', () => {
        if (timedOutFlag) reject(timeoutError());
      });
    });
    try {
      const res = await Promise.race([fetch(path, { ...fetchOpts, signal: controller.signal }), timedOut]);
      const text = await res.text();
      let body = null;
      if (text) {
        try { body = JSON.parse(text); } catch { body = text; }
      }
      const elapsed = Math.round(((typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now()) - t0);
      if (!res.ok) {
        const err = (body && typeof body === 'object' && body.error) ? body.error : `HTTP ${res.status}`;
        RailLog.warn(`api ${method} ${path} -> ${res.status} (${elapsed}ms) error: ${err}`);
        RailLog.api({
          method, url: path, status: res.status, latency_ms: elapsed, req_body: reqBody,
          error: err, body_snippet: text.slice(0, 400),
        });
        return { ok: false, status: res.status, error: err, body };
      }
      RailLog.info(`api ${method} ${path} -> ${res.status} (${elapsed}ms)`);
      RailLog.api({
        method, url: path, status: res.status, latency_ms: elapsed, req_body: reqBody,
        body_snippet: text.slice(0, 400),
      });
      return body;
    } catch (err) {
      const em = err && err.message ? err.message : String(err);
      const elapsed = Math.round(((typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now()) - t0);
      RailLog.api({
        method, url: path, status: 0, latency_ms: elapsed, req_body: reqBody,
        error: `network: ${em}`, thrown: true,
      });
      RailLog.error('api fetch threw:', method, path, em);
      if (err && (err.code === 'TIMEOUT' || (err.name === 'AbortError' && timedOutFlag))) throw timeoutError();
      throw err;
    } finally {
      clearTimeout(timer);
    }
  }

  const get = (path, opts = {}) => request(path, opts);

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
    stationTimetable: (station) => get(`/rail-api/ntes/station-timetable?station=${encodeURIComponent(station)}`),
    averageDelay: (train) => get(`/rail-api/ntes/average-delay?train=${encodeURIComponent(train)}`),
    heritage: (selection) => get(`/rail-api/ntes/heritage?selection=${encodeURIComponent(selection ?? 0)}`),
    parcel: () => get('/rail-api/ntes/parcel'),
    journeyStations: (train) => get(`/rail-api/ntes/journey-stations?train=${encodeURIComponent(train)}`),
    journeyBasis: (train, station) => get(`/rail-api/ntes/journey-basis?train=${encodeURIComponent(train)}&station=${encodeURIComponent(station)}`),
    trainOnMap: (train, station) => get(`/rail-api/ntes/train-on-map?train=${encodeURIComponent(train)}` + (station ? `&station=${encodeURIComponent(station)}` : '')),
    availability: (src, dst, date, source) =>
      get(`/rail-api/availability?src=${encodeURIComponent(src)}&dst=${encodeURIComponent(dst)}` + (date ? `&date=${encodeURIComponent(date)}` : '') + (source && source !== 'auto' ? `&source=${encodeURIComponent(source)}` : '')),
    chart: (train, date, station) =>
      get(`/rail-api/irctc/chart?train=${encodeURIComponent(train)}` + (date ? `&date=${encodeURIComponent(date)}` : '') + (station ? `&station=${encodeURIComponent(station)}` : '')),
    exceptional: (train, type) =>
      get(`/rail-api/ntes/exceptional?train=${encodeURIComponent(train)}` + (type ? `&type=${encodeURIComponent(type)}` : '')),
    stations: (q) => get(`/rail-api/stations?q=${encodeURIComponent(q)}`),
    searchTrains: (q) => get(`/rail-api/search/trains?q=${encodeURIComponent(q)}`),
    searchStations: (q) => get(`/rail-api/search/stations?q=${encodeURIComponent(q)}`),
    suggest: (q) => get(`/rail-api/search/suggest?q=${encodeURIComponent(q)}`),
    observability: () => get('/rail-api/observability'),
    logs: (limit = 100, level) =>
      get(`/rail-api/logs?limit=${limit}` + (level ? `&level=${encodeURIComponent(level)}` : '')),
  };
})();
