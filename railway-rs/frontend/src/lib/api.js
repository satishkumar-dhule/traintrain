const TIMEOUT_MS = 12000

const cache = new Map()
const inflight = new Set()
const warmed = new Set()

function slowConnection(conn) {
  if (!conn) return false
  return Boolean(conn.saveData) || conn.effectiveType === '2g' || conn.effectiveType === 'slow-2g'
}

export async function api(path, opts = {}) {
  const { cachePaint = false, onFresh } = opts
  if (cachePaint && cache.has(path)) {
    const hit = cache.get(path)
    revalidate(path, onFresh)
    return { ok: true, data: hit.data, stale: true }
  }
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS)
  try {
    const res = await fetch(path, { signal: controller.signal })
    const text = await res.text()
    let body = null
    if (text) {
      try {
        body = JSON.parse(text)
      } catch {
        body = text
      }
    }
    if (!res.ok) {
      const error =
        body && typeof body === 'object' && body.error ? body.error : `HTTP ${res.status}`
      return { ok: false, status: res.status, error, body }
    }
    cache.set(path, { data: body, ts: Date.now() })
    return { ok: true, data: body }
  } catch (err) {
    if (err && err.name === 'AbortError') {
      return { ok: false, status: 0, error: `Request timed out after ${TIMEOUT_MS}ms` }
    }
    return { ok: false, status: 0, error: err && err.message ? err.message : String(err) }
  } finally {
    clearTimeout(timer)
  }
}

function revalidate(path, onFresh) {
  if (inflight.has(path)) return
  inflight.add(path)
  api(path)
    .then((res) => {
      if (!res.ok) return
      cache.set(path, { data: res.data, ts: Date.now() })
      if (typeof onFresh === 'function') onFresh(res.data)
    })
    .catch(() => {})
    .finally(() => inflight.delete(path))
}

export function peekCached(path) {
  const hit = cache.get(path)
  return hit ? hit.data : null
}

export function cacheAgeSeconds(path) {
  const hit = cache.get(path)
  return hit ? Math.max(0, Math.round((Date.now() - hit.ts) / 1000)) : null
}

export function prefetch(path) {
  if (!path || warmed.has(path)) return
  const conn = typeof navigator !== 'undefined' ? navigator.connection ?? null : null
  if (slowConnection(conn)) return
  warmed.add(path)
  Promise.resolve(api(path)).catch(() => {})
}
