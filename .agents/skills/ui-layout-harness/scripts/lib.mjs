// Shared helpers for the UI layout harness.
import fs from 'node:fs'
import path from 'node:path'

export const HOME = process.env.UI_HARNESS_HOME || '/tmp/opencode/ui-harness'

export async function loadPlaywright() {
  return import(path.join(HOME, 'node_modules', 'playwright-core', 'index.mjs'))
}

export function findShell() {
  for (const d of fs.readdirSync(path.join(HOME, 'browsers'))) {
    const p = path.join(HOME, 'browsers', d, 'chrome-linux', 'headless_shell')
    if (fs.existsSync(p)) return p
  }
  throw new Error('no headless_shell — run setup.mjs first')
}

export const browserEnv = () => ({
  ...process.env,
  LD_LIBRARY_PATH: `${HOME}/debroot/usr/lib/x86_64-linux-gnu:${HOME}/debroot/lib/x86_64-linux-gnu`,
  FONTCONFIG_PATH: `${HOME}/debroot/etc/fonts`,
})

export async function launch(pw) {
  return pw.chromium.launch({
    executablePath: findShell(),
    env: browserEnv(),
    args: ['--no-sandbox', '--disable-gpu', '--force-device-scale-factor=1'],
  })
}

// Elements sticking out of the viewport are offenders UNLESS some ancestor
// other than html/body clips them (overflow != visible AND its box actually
// cuts the element off) — e.g. an internally-scrollable table container.
// We intentionally skip html/body because their overflow-x:clip is exactly
// the "unreachable zone" we're hunting for.
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
        // does this ancestor's box actually clip the element?
        if (r.right > pr.right + 1 || r.left < pr.left - 1) {
          contained = true
          break
        }
      }
      if (!contained && out.offenders.length < 12)
        out.offenders.push({
          tag: el.tagName.toLowerCase(),
          cls: String(el.className?.baseVal ?? el.className).slice(0, 90),
          left: Math.round(r.left),
          right: Math.round(r.right),
          w: Math.round(r.width),
          text: (el.textContent || '').trim().slice(0, 40),
        })
    }
    return out
  })
}

export function logDiag(label, d) {
  console.log(
    `== ${label} == viewport ${d.winW} | docScrollW ${d.docScrollW} | hOverflow ${d.hOverflow}`,
  )
  for (const o of d.offenders)
    console.log(`  OFFENDER <${o.tag}> [${o.cls}] L${o.left} R${o.right} w${o.w} "${o.text}"`)
}
