// offload.js — headless sandbox behind the scenes to derive API data
// 100% offloaded to user's device: Dedicated Worker + p-limit fan-out, user IP.
// No UI. No server relay. Used internally by pages to fan out n=2 and merge.
//
// Usage (behind the scenes, no Lab UI):
//   import { fetchAllOffloaded, mapLimitOffloaded, deriveOffloaded, n2Derive } from '$lib/offload.js'
//   const boards = await fetchAllOffloaded(['/rail-api/ntes/live-station?station=NDLS', ...], {concurrency:3})
//   const derived = await deriveOffloaded(`return data.map(d=>d.trains?.length)`, {data: boards})
//   const tree = await n2Derive({ items: ['NDLS','AGC'], l1Concurrency:2, l2Concurrency:2, l1Code: `...`, l2Code: `...` })

const TIMEOUT_DEFAULT = 9000

const WORKER_SRC = `
  function pLimit(n){
    let active=0; const q=[];
    const next=()=>{
      if(active>=n || q.length===0) return;
      active++;
      const {fn, resolve, reject} = q.shift();
      Promise.resolve().then(fn).then(v=>{active--; resolve(v); next();}, e=>{active--; reject(e); next();});
    };
    return (fn)=> new Promise((resolve,reject)=>{ q.push({fn,resolve,reject}); next(); });
  }

  async function fetchWithTimeout(url, opts={}, ms=7000){
    const ctrl = new AbortController();
    const tid = setTimeout(()=> ctrl.abort(new DOMException('Timeout '+ms+'ms '+url, 'TimeoutError')), ms);
    try{
      const res = await fetch(url, { ...opts, signal: opts.signal ? AbortSignal.any([opts.signal, ctrl.signal]) : ctrl.signal });
      const text = await res.text();
      let data=null;
      if(text){ try{ data=JSON.parse(text);}catch{ data=text; } }
      if(!res.ok){
        const msg = data && typeof data==='object' && data.error ? data.error : 'HTTP '+res.status;
        const err=new Error(msg); err.status=res.status; err.body=data; throw err;
      }
      return data;
    } finally { clearTimeout(tid); }
  }

  async function fetchAllOffloadedInner(urls, {concurrency=4, timeoutMs=7000}={}){
    const limit=pLimit(concurrency);
    const withTimeout=(p,ms)=>{ let tid; const t=new Promise((_,rej)=>{tid=setTimeout(()=>rej(new DOMException('Timeout','TimeoutError')),ms)}); return Promise.race([p,t]).finally(()=>clearTimeout(tid)); };
    return Promise.allSettled(urls.map(u=> limit(()=> withTimeout(fetchWithTimeout(u,{},timeoutMs), timeoutMs+500))));
  }

  async function mapLimitOffloadedInner(items, concurrency, fn){
    const limit=pLimit(concurrency);
    return Promise.allSettled(items.map(it=> limit(()=> fn(it))));
  }

  // runDAG — tiny topological executor for n=2 orchestration inside worker
  async function runDAGInner(nodes, {concurrency=4}={}){
    const done=new Map(); const byId=new Map(nodes.map(n=>[n.id,n])); const rem=new Set(nodes.map(n=>n.id)); const limit=pLimit(concurrency);
    while(rem.size){
      const ready=[...rem].filter(id=> (byId.get(id).deps||[]).every(d=>done.has(d)));
      if(ready.length===0) throw new Error('DAG cycle: '+[...rem].join(','));
      const settled=await Promise.allSettled(ready.map(id=> limit(async()=>{ const v=await byId.get(id).run(done); done.set(id,v); return v; })));
      for(let i=0;i<ready.length;i++) if(settled[i].status==='rejected') throw settled[i].reason;
      ready.forEach(id=> rem.delete(id));
    }
    return done;
  }

  self.onmessage = async (e)=>{
    const msg=e.data; if(!msg||!msg.id) return;
    const {id, type, payload} = msg;
    try{
      if(type==='FETCH_ALL'){
        const {urls, opts} = payload;
        const out = await fetchAllOffloadedInner(urls, opts);
        // Normalize: {status, value/reason} -> plain JSON (reason as string)
        const norm = out.map(r=> r.status==='fulfilled' ? {status:'fulfilled', value:r.value} : {status:'rejected', reason: r.reason?.message||String(r.reason), statusCode:r.reason?.status});
        postMessage({id, ok:true, data:norm});
      } else if(type==='MAP_LIMIT'){
        const {items, concurrency, fnCode} = payload;
        // fnCode is string body: e.g., "async (item, {fetchJson}) => { return await fetchJson('/rail-api/...'+item) }"
        const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
        const userFn = new AsyncFunction('fetchJson','fetchAll','pLimit','runDAG', 'return ('+fnCode+')') (fetchWithTimeout, fetchAllOffloadedInner, pLimit, runDAGInner);
        const settled = await mapLimitOffloadedInner(items, concurrency, userFn);
        const norm = settled.map(r=> r.status==='fulfilled' ? {status:'fulfilled', value:r.value} : {status:'rejected', reason: r.reason?.message||String(r.reason)});
        postMessage({id, ok:true, data:norm});
      } else if(type==='DERIVE'){
        const {code, context} = payload;
        // code is string body that can use context + helpers. Example: "return context.data.map(...)"
        const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
        const fn = new AsyncFunction('context','fetchJson','fetchAll','mapLimit','pLimit','runDAG', code);
        const out = await fn(context, fetchWithTimeout, fetchAllOffloadedInner, mapLimitOffloadedInner, pLimit, runDAGInner);
        postMessage({id, ok:true, data:out});
      } else if(type==='N2'){
        const {items, l1Concurrency, l2Concurrency, l1Code} = payload;
        // l1Code is string for L1 handler: async (item, {fetchJson, fetchAll}) => { ... }
        const AsyncFunction = Object.getPrototypeOf(async function(){}).constructor;
        const l1Fn = new AsyncFunction('fetchJson','fetchAll','pLimit','runDAG', 'return ('+l1Code+')')(fetchWithTimeout, fetchAllOffloadedInner, pLimit, runDAGInner);
        // wrap l1Fn to inject correct fetchAll concurrency
        const wrapped = async (it)=>{
          // l1Fn may call fetchAll internally with its own concurrency; we just call it
          return await l1Fn(it);
        };
        const settled = await mapLimitOffloadedInner(items, l1Concurrency||2, wrapped);
        const norm = settled.map(r=> r.status==='fulfilled' ? {status:'fulfilled', value:r.value} : {status:'rejected', reason: r.reason?.message||String(r.reason)});
        postMessage({id, ok:true, data:norm});
      } else {
        postMessage({id, ok:false, error:'unknown type '+type});
      }
    } catch(err){
      postMessage({id, ok:false, error: err?.message||String(err), stack: err?.stack||''});
    }
  };
`

let worker = null
let seq = 0
const pending = new Map()
let workerUrl = null

function getWorker() {
  if (worker) return worker
  const blob = new Blob([WORKER_SRC], { type: 'text/javascript' })
  workerUrl = URL.createObjectURL(blob)
  worker = new Worker(workerUrl, { type: 'module' })
  worker.onmessage = (e) => {
    const { id } = e.data || {}
    const p = pending.get(id)
    if (p) {
      pending.delete(id)
      clearTimeout(p.timer)
      p.resolve(e.data)
    }
  }
  worker.onerror = (e) => {
    // reject all pending on worker error (offloaded error)
    for (const [id, p] of pending.entries()) {
      clearTimeout(p.timer)
      p.reject(new Error(e.message || 'Worker error'))
    }
    pending.clear()
  }
  return worker
}

function callWorker(type, payload, timeoutMs = TIMEOUT_DEFAULT) {
  const w = getWorker()
  const id = String(++seq)
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id)
      // hard kill and respawn worker (offloaded kill)
      try { w.terminate() } catch {}
      worker = null
      if (workerUrl) { try { URL.revokeObjectURL(workerUrl) } catch {} workerUrl = null }
      reject(new Error(`offload timeout after ${timeoutMs}ms (Worker terminated)`))
    }, timeoutMs + 800)
    pending.set(id, { resolve, reject, timer })
    w.postMessage({ id, type, payload })
  }).then((res) => {
    if (!res.ok) throw new Error(res.error || 'offload failed')
    return res.data
  })
}

// Public API — all offloaded to user's Worker (user IP, user cores), no server, no UI

/** Fan-out N fetches in Worker, concurrency-limited. Returns array of {status, value/reason}. */
export function fetchAllOffloaded(urls, opts = {}) {
  const { concurrency = 4, timeoutMs = 7000 } = opts
  return callWorker('FETCH_ALL', { urls, opts: { concurrency, timeoutMs } }, timeoutMs + 2000)
}

/** Map with concurrency limit inside Worker. fnCode is stringified async function, e.g. `async (code)=> await fetchJson('/rail-api/ntes/live-station?station='+code)` */
export function mapLimitOffloaded(items, concurrency, fnCode) {
  return callWorker('MAP_LIMIT', { items, concurrency, fnCode }, TIMEOUT_DEFAULT)
}

/** Run arbitrary derive code in sandboxed Worker with context. Code sees: context, fetchJson, fetchAll, mapLimit, pLimit, runDAG */
export function deriveOffloaded(code, context = {}) {
  // code example: `return context.boards.map(b=> b.trains?.length)`
  // or full async: `const out = await fetchAll(context.urls); return out;`
  return callWorker('DERIVE', { code, context }, TIMEOUT_DEFAULT)
}

/** n=2 deep delegation: L1 fans to N items, each L1 handler may fan to M (via fetchAll inside l1Code). */
export function n2Derive({ items, l1Concurrency = 2, l1Code, timeoutMs = TIMEOUT_DEFAULT }) {
  // l1Code: stringified async function, e.g. `async (station) => { const b=await fetchJson('/rail-api/ntes/live-station?station='+station); const trains=(b.trains||[]).slice(0,2); const det=await fetchAll(trains.map(t=>'/rail-api/schedule?train='+t.number),{concurrency:2}); return {station, count: trains.length, details: det}; }`
  return callWorker('N2', { items, l1Concurrency, l1Code }, timeoutMs)
}

/** Convenience: terminate and discard offload worker (e.g., on page hide). */
export function terminateOffload() {
  if (worker) {
    try { worker.terminate() } catch {}
    worker = null
  }
  if (workerUrl) { try { URL.revokeObjectURL(workerUrl) } catch {} workerUrl = null }
  for (const [, p] of pending.entries()) clearTimeout(p.timer)
  pending.clear()
}

// Re-export for behind-the-scenes auto-derive in pages (example helper)
/** Derive live boards for multiple stations behind the scenes (no UI Lab). */
export async function deriveLiveBoards(stationCodes, { boardConcurrency = 3, detailConcurrency = 2 } = {}) {
  // This is a behind-the-scenes n=2 example that callers can use without any UI.
  // It offloads all fetches + merging to Worker (user device).
  const l1Code = `
    async (code) => {
      const board = await fetchJson('/rail-api/ntes/live-station?station=' + encodeURIComponent(code) + '&hours=2');
      const trains = Array.isArray(board?.trains) ? board.trains.slice(0, 3) : [];
      // L2 fan-out per station: fetch schedule for each train
      const schedules = await fetchAll(
        trains.map(t => '/rail-api/schedule?train=' + encodeURIComponent(t.number || t.train_number || '')),
        { concurrency: ${detailConcurrency}, timeoutMs: 7000 }
      );
      return {
        station: code,
        boardTrains: trains.length,
        data_source: board?.data_source || null,
        schedules: schedules.map((r,i)=> r.status==='fulfilled' ? { ok:true, train: trains[i]?.number, data: r.value } : { ok:false, train: trains[i]?.number, error: r.reason })
      };
    }
  `
  return n2Derive({ items: stationCodes, l1Concurrency: boardConcurrency, l1Code })
}
