import { recordVisit } from './visit-trail.svelte.js'

function normalize(p) {
  if (!p || p === '/') return '/'
  return p.length > 1 ? p.replace(/\/+$/, '') : p
}

export const route = $state({ path: normalize(window.location.pathname) })

export function navigate(to) {
  window.history.pushState({}, '', to)
  route.path = normalize(window.location.pathname)
  recordVisit(route.path)
  window.scrollTo(0, 0)
}

window.addEventListener('popstate', () => {
  route.path = normalize(window.location.pathname)
  recordVisit(route.path)
})
