const KEY = 'rc-visits'
const MAX_ITEMS = 10

const SECTIONS = [
  { seg: '',            label: 'Home' },
  { seg: 'train',       label: 'Live' },
  { seg: 'station',     label: 'Board' },
  { seg: 'journeys',    label: 'Journeys' },
  { seg: 'availability',label: 'Availability' },
  { seg: 'pnr',         label: 'PNR Status' },
  { seg: 'extras',      label: 'Heritage & Parcel' },
  { seg: 'assistant',   label: 'Ask Train Bro' },
  { seg: 'system',      label: 'System' },
  { seg: 'about',       label: 'About' }
]

function normalize(p) {
  if (!p || p === '/') return '/'
  return p.length > 1 ? p.replace(/\/+$/, '') : p
}

function describe(rawPath) {
  const path = normalize(rawPath)
  const segments = path.split('/').filter(Boolean)
  if (segments.length < 2) return null
  const seg = segments[0]
  const found = SECTIONS.find((s) => s.seg === seg)
  let label = found ? found.label : seg.charAt(0).toUpperCase() + seg.slice(1)
  const raw = segments[1]
  const entity = /^[a-z0-9]{3,5}$/i.test(raw) ? raw.toUpperCase() : raw
  label = `${label} · ${entity}`
  return { path, label }
}

export const visitTrail = $state({ entries: [] })

export function hydrate() {
  try {
    const raw = JSON.parse(sessionStorage.getItem(KEY) ?? '[]')
    if (!Array.isArray(raw)) return
    visitTrail.entries = raw
      .filter((e) => e && typeof e.path === 'string' && typeof e.label === 'string')
      .slice(0, MAX_ITEMS)
  } catch {}
}

export function persist() {
  try {
    sessionStorage.setItem(KEY, JSON.stringify(visitTrail.entries))
  } catch {}
}

export function recordVisit(rawPath) {
  const entry = describe(rawPath)
  if (!entry) return
  const last = visitTrail.entries[visitTrail.entries.length - 1]
  if (last && last.path === entry.path) return
  visitTrail.entries.push({ ...entry, ts: Date.now() })
  if (visitTrail.entries.length > MAX_ITEMS) visitTrail.entries.shift()
  persist()
}

export function clearTrail() {
  visitTrail.entries = []
  try {
    sessionStorage.removeItem(KEY)
  } catch {}
}

hydrate()
recordVisit(window.location.pathname)
