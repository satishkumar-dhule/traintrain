/* Shared reactive viewport flag (client-only SPA).
   "narrow" = phone/tablet layout below the lg breakpoint: cards instead of
   tables, bottom nav instead of sidebar. One matchMedia listener for all. */
const mq =
  typeof window !== 'undefined' && window.matchMedia
    ? window.matchMedia('(max-width: 1023.98px)')
    : null

export const viewport = $state({ narrow: mq ? mq.matches : false })

if (mq) {
  const on = (e) => {
    viewport.narrow = e.matches
  }
  if (mq.addEventListener) mq.addEventListener('change', on)
  else mq.addListener(on)
}
