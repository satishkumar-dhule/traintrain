const TIMEOUT_MS = 12000

export async function api(path) {
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
