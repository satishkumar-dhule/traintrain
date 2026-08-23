import { clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs) {
  return twMerge(clsx(inputs))
}

/* ----- SPA href builders -----
   One place for cross-page jumps so route shapes never drift again
   (chat ToolCards used to point at removed /plan/... routes). */

const enc = encodeURIComponent

export function todayISO() {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

export const DATE_RE = /^\d{4}-\d{2}-\d{2}$/

export function trainHref(number, view = '') {
  return `/train/${enc(String(number ?? '').trim())}${view ? `/${view}` : ''}`
}

export function stationHref(code, view = 'live') {
  return `/station/${enc(String(code ?? '').trim().toUpperCase())}${view ? `/${view}` : ''}`
}

export function journeysHref(src, dst) {
  return `/journeys/${enc(String(src ?? '').trim().toUpperCase())}/${enc(String(dst ?? '').trim().toUpperCase())}`
}

/* Availability auto-searches only when src, dst AND an ISO date are all
   present, so callers that omit a date silently get today. */
export function availabilityHref(src, dst, date = todayISO()) {
  const dt = DATE_RE.test(String(date ?? '')) ? String(date) : todayISO()
  return (
    `/availability/${enc(String(src ?? '').trim().toUpperCase())}` +
    `/${enc(String(dst ?? '').trim().toUpperCase())}/${enc(dt)}`
  )
}
