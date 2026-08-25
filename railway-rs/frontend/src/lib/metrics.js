// metrics.js — shared observability + runtime helpers (super fan-out, single source)
// Extracted from System.svelte / About.svelte duplications (B11/C15 in dry plan).
// Re-uses format.js primitives (humanBytes) — no second implementation.

import { humanBytes } from './format.js'

export function num(v) {
  const n = Number(v)
  return Number.isFinite(n) ? n : null
}

export function memMb(n) {
  const b = num(n)
  if (b === null) return '—'
  return (b / (1024 * 1024)).toFixed(1)
}

export function pctFromFrac(v) {
  const f = num(v)
  if (f === null) return '—'
  return `${(f * 100).toFixed(1)}%`
}

export function latest(arr) {
  if (!Array.isArray(arr)) return null
  for (let i = arr.length - 1; i >= 0; i--) {
    const n = num(arr[i])
    if (n !== null) return n
  }
  return null
}

export function seriesRange(arr) {
  if (!Array.isArray(arr)) return null
  const vals = arr.slice(-60).map(num).filter((v) => v !== null)
  if (!vals.length) return null
  return { min: Math.min(...vals), max: Math.max(...vals) }
}

export function sparkPoints(arr) {
  if (!Array.isArray(arr)) return []
  const tail = arr.slice(-60)
  const vals = tail.map((v) => num(v)).map((v) => (v === null ? 0 : Math.max(v, 0)))
  const max = Math.max(...vals)
  return vals.map((v, i) => ({
    pct: max > 0 ? Math.min(100, (v / max) * 100) : 0,
    op: vals.length > 1 ? 0.2 + 0.8 * (i / (vals.length - 1)) : 1,
    val: tail[i]
  }))
}

export function logLine(l) {
  const f = l && typeof l === 'object' ? l.fields ?? {} : {}
  const msg = l?.message != null ? String(l.message) : ''
  const bits = []
  if (f.method) bits.push(f.method)
  if (f.path) bits.push(f.path)
  if (f.status_code != null) bits.push(`→ ${f.status_code}`)
  if (f.latency_ms != null) bits.push(`${f.latency_ms}ms`)
  return bits.length ? `${msg} · ${bits.join(' ')}` : msg
}

export function tsTime(t) {
  const ms = num(t)
  if (ms === null) return '—'
  const d = new Date(ms)
  if (Number.isNaN(d.getTime())) return '—'
  return d.toLocaleTimeString('en-GB', { hour12: false })
}

export function sortedLogs(raw) {
  const arr = Array.isArray(raw) ? raw.slice() : []
  arr.sort((a, b) => {
    const ta = num(a?.ts) ?? 0
    const tb = num(b?.ts) ?? 0
    return tb - ta
  })
  return arr
}

export function codeClass(code) {
  if (code >= 200 && code < 300) return 'bg-signal-go'
  if (code >= 300 && code < 400) return 'bg-chart-3'
  if (code >= 400 && code < 500) return 'bg-signal-hold'
  if (code >= 500 && code < 600) return 'bg-signal-stop'
  return 'bg-muted-foreground/40'
}

export function cacheValue(key, v) {
  if (/bytes/i.test(String(key))) {
    const b = num(v)
    if (b !== null) return humanBytes(b)
  }
  if (v === null || v === undefined || v === '') return '—'
  if (typeof v === 'object') {
    try {
      return JSON.stringify(v)
    } catch {
      return String(v)
    }
  }
  return String(v)
}

export function updatedTime(iso) {
  if (!iso) return ''
  const d = new Date(iso)
  return Number.isNaN(d.getTime()) ? '' : d.toLocaleTimeString('en-GB', { hour12: false })
}

export function hitRateFromCache(cache) {
  if (!cache || typeof cache !== 'object') return null
  const hits = num(cache.hits) ?? 0
  const misses = num(cache.misses) ?? 0
  const lookups = hits + misses
  if (lookups <= 0) return null
  return (hits / lookups) * 100
}
