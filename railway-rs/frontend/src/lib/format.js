// Shared formatting helpers - super-optimized, memoized, single source of truth
// Fan-out across Train, Station, Availability, Pnr, Exceptions, Extras, Home, System, About

export const DATE_RE = /^\d{4}-\d{2}-\d{2}$/
export const RUN_MONTHS = ['jan','feb','mar','apr','may','jun','jul','aug','sep','oct','nov','dec']
export const MONTHS_UC = ['JAN','FEB','MAR','APR','MAY','JUN','JUL','AUG','SEP','OCT','NOV','DEC']

export function norm(v) {
  return String(v ?? '').trim().toUpperCase()
}

export function asText(v) {
  return String(v ?? '').trim()
}

export function fmtDash(v) {
  const s = String(v ?? '').trim()
  return s && s !== '-' && s !== '--' ? s : '—'
}
export const fmt = fmtDash // alias for legacy imports
export const fmtTime = fmtDash

export function numOrNull(v) {
  const s = String(v ?? '').trim()
  if (!s) return null
  const n = Number(s)
  return Number.isFinite(n) ? n : null
}

// ISO "2026-08-25" -> "25-AUG-2026" (NTES style)
const fmtExcCache = new Map()
export function fmtExcDate(iso) {
  const raw = String(iso ?? '').trim()
  if (!raw) return '—'
  if (fmtExcCache.has(raw)) return fmtExcCache.get(raw)
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(raw)
  let out = raw
  if (m) {
    const mo = (RUN_MONTHS[Number(m[2]) - 1] ?? '').toUpperCase()
    out = mo ? `${m[3]}-${mo}-${m[1]}` : raw
  }
  fmtExcCache.set(raw, out)
  return out
}

// "DD-MMM-YYYY" / ISO -> "YYYY-MM-DD"
const normDayCache = new Map()
export function normDay(s) {
  const str = String(s ?? '').trim()
  if (normDayCache.has(str)) return normDayCache.get(str)
  let out = str.slice(0,10)
  const m = /^(\d{1,2})-([A-Za-z]{3})-(\d{4})$/.exec(str)
  if (m) {
    const mo = RUN_MONTHS.indexOf(m[2].toLowerCase())
    out = mo >= 0 ? `${m[3]}-${String(mo+1).padStart(2,'0')}-${m[1].padStart(2,'0')}` : ''
  }
  normDayCache.set(str, out)
  return out
}

export function ntesDate(iso) {
  if (!iso) return null
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso).trim())
  if (!m) return null
  const mi = Number(m[2]) - 1
  if (mi < 0 || mi > 11) return null
  return `${m[3]}-${MONTHS_UC[mi]}-${m[1]}`
}

export function todayISO() {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`
}

export function fmtUptime(s) {
  const n = Number(s)
  if (!Number.isFinite(n) || n < 0 || n <= 0) return '—'
  const d = Math.floor(n/86400)
  const h = Math.floor((n%86400)/3600)
  const m = Math.floor((n%3600)/60)
  const sec = Math.floor(n%60)
  if (d) return `${d}d ${h}h${m ? ` ${m}m` : ''}`
  if (h) return `${h}h ${m}m`
  if (m) return sec ? `${m}m ${sec}s` : `${m}m`
  return `${sec}s`
}

export function fmtInt(n) {
  const v = Number(n)
  return Number.isFinite(v) ? Math.round(v).toLocaleString('en-IN') : '—'
}

const compactFmt = new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 })
export function fmtCompact(n) {
  const v = Number(n)
  if (!Number.isFinite(v)) return '—'
  return compactFmt.format(v)
}

export function humanBytes(b) {
  const n = Number(b)
  if (!Number.isFinite(n) || n < 0) return '—'
  if (n < 1024) return `${n} B`
  const kb = n / 1024
  if (kb < 1024) return `${kb.toFixed(1)} KB`
  const mb = kb / 1024
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  return `${(mb/1024).toFixed(2)} GB`
}

export function delayLabelFromMins(mins) {
  if (mins == null) return '—'
  if (mins > 0) return `${mins} min late`
  return 'on time'
}
