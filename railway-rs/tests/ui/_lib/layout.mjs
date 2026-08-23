// layout.mjs - in-page diagnostics + assertion helpers shared by every spec.
// The overflow model matches the app's CSS contract: html/body clip x-overflow
// (overflow-x: clip), so an offender is an element that escapes the viewport
// WITHOUT any intermediate scrollable/clip ancestor - i.e. genuinely
// unreachable content on real devices.
import assert from 'node:assert/strict'

/* Port of the harness diagnose(): find unreachable overflow + measure scroll. */
export function diagnose(page) {
  return page.evaluate(() => {
    const doc = document.documentElement
    const out = {
      winW: window.innerWidth,
      docScrollW: doc.scrollWidth,
      hOverflow: doc.scrollWidth > window.innerWidth + 1,
      offenders: [],
    }
    for (const el of document.querySelectorAll('body *')) {
      const r = el.getBoundingClientRect()
      if ((!r.width && !r.height) || !(r.right > window.innerWidth + 1 || r.left < -1)) continue
      let contained = false
      for (
        let p = el.parentElement;
        p && p !== document.body && p !== document.documentElement;
        p = p.parentElement
      ) {
        const cs = getComputedStyle(p)
        if (cs.overflowX === 'visible' && cs.overflowY === 'visible') continue
        const pr = p.getBoundingClientRect()
        if (r.right > pr.right + 1 || r.left < pr.left - 1) {
          contained = true
          break
        }
      }
      if (!contained && out.offenders.length < 8)
        out.offenders.push({
          tag: el.tagName.toLowerCase(),
          cls: String(el.className?.baseVal ?? el.className).slice(0, 90),
          left: Math.round(r.left),
          right: Math.round(r.right),
          text: (el.textContent || '').trim().slice(0, 40),
        })
    }
    return out
  })
}

export function fmtOffender(o) {
  return `<${o.tag}> [${o.cls}] L${o.left} R${o.right} "${o.text}"`
}

/* No content may escape the viewport without a clipping ancestor. */
export async function assertNoHorizontalOverflow(session) {
  const d = await diagnose(session.page)
  assert.equal(
    d.hOverflow,
    false,
    `document scrolls horizontally (${d.docScrollW} > ${d.winW}) on ${session.page.url()}`,
  )
  assert.deepEqual(
    d.offenders.map(fmtOffender),
    [],
    `unreachable overflow offenders on ${session.page.url()}`,
  )
}

/* If the page is taller than the viewport, scrolling must actually move it. */
export async function assertVerticalScrollWorks(session) {
  const v = await session.page.evaluate(() => {
    window.scrollTo(0, 400)
    return { y: window.scrollY, max: document.documentElement.scrollHeight - window.innerHeight }
  })
  if (v.max <= 0) return // page fits; nothing to scroll
  assert.ok(
    v.y > 0,
    `vertical scroll is broken on ${session.page.url()} (scrolled to ${v.y} of ${v.max})`,
  )
}

/* Zero uncaught exceptions anywhere in a session. Network failures are NOT
   errors here: honest upstream unavailability is the product's contract. */
export async function assertNoPageErrors(session) {
  const msgs = session.errors.pageerror.map((e) => `${e.message}\n${e.stack?.split('\n')[1] ?? ''}`)
  assert.deepEqual(msgs, [], `uncaught JS exceptions on ${session.page.url()}`)
}

/* Console hygiene: allow network/resource noise, forbid app-level errors.
   Add ignore patterns here (never in specs) with a reason comment. */
const CONSOLE_IGNORE = [
  /Failed to load resource/i, // browser net-log for unreachable upstreams
  /ERR_(CONNECTION|NAME|INTERNET|TIMED|PROXY|SSL|HTTP2)/i,
]

export async function assertNoConsoleErrors(session) {
  const bad = session.errors.console.filter((m) => !CONSOLE_IGNORE.some((re) => re.test(m)))
  assert.deepEqual(bad, [], `console.error on ${session.page.url()}`)
}

/* Every form control inside <main> must have an accessible name:
   <label for>, wrapping <label>, aria-label, aria-labelledby or title. */
export async function assertControlsAreLabelled(session) {
  const bad = await session.page.evaluate(() => {
    const named = (el) =>
      (el.id && document.querySelector(`label[for="${CSS.escape(el.id)}"]`)) ||
      el.closest('label') ||
      el.getAttribute('aria-label') ||
      el.getAttribute('aria-labelledby') ||
      el.getAttribute('title')
    const sel =
      'main input:not([type=hidden]), main select, main textarea'
    return [...document.querySelectorAll(sel)]
      .filter((el) => {
        const off = el.offsetWidth === 0 && el.offsetHeight === 0
        return !off && !named(el)
      })
      .map((el) => `<${el.tagName.toLowerCase()} type=${el.type}> placeholder="${el.placeholder ?? ''}"`)
  })
  assert.deepEqual(bad, [], `form controls without accessible names on ${session.page.url()}`)
}

/* Icon-only buttons must expose a name too; text buttons get it free. */
export async function assertButtonsAreNamed(session) {
  const bad = await session.page.evaluate(() =>
    [...document.querySelectorAll('main button')]
      .filter((b) => b.offsetWidth > 0)
      .filter((b) => !(b.innerText.trim() || b.getAttribute('aria-label') || b.getAttribute('title')))
      .map((b) => `button [${String(b.className).slice(0, 60)}]`),
  )
  assert.deepEqual(bad, [], `icon-only buttons without accessible names on ${session.page.url()}`)
}

/* Exactly one h1 per route keeps the document outline sane. */
export async function assertSingleH1(session) {
  const n = await session.page.evaluate(
    () => document.querySelectorAll('main h1').length,
  )
  assert.equal(n, 1, `expected exactly one <h1> in main on ${session.page.url()}, found ${n}`)
}
