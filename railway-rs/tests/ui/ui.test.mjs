/* ui.test.mjs - the app's always-on real-browser UI suite.
   Runs against the actual SPA served by the axum binary (default :3000) via
   playwright-core + a private chromium-headless-shell (no host installs).

   Contract pinned here (TDD: this file defines the intended behavior):
     1. /healthz answers the service identity the tooling relies on
     2. every nav route renders exactly one h1 and real content
     3. nothing overflows horizontally at desktop or phone widths
     4. vertical scrolling works wherever content exceeds the viewport
     5. form controls and icon-only buttons carry accessible names
     6. the live-status flow always resolves to data or an honest error
     7. theme choice persists across reloads (rc-theme -> html.light/dark)
     8. zero uncaught JS exceptions and no app-level console errors

   Run:  npm run test:ui        (or: make test-ui)
*/
import test from 'node:test'
import assert from 'node:assert/strict'
import * as ui from './_lib/index.mjs'

const READY = ui.browserReady()

if (!READY && !process.env.UI_STRICT) {
  console.warn(
    `\n[ui-suite] browser harness missing at ${ui.HARNESS_HOME}\n` +
      `[ui-suite] one-time setup: node .agents/skills/ui-layout-harness/scripts/setup.mjs\n` +
      `[ui-suite] skipping UI specs (set UI_STRICT=1 to fail instead)\n`,
  )
}
const t = READY ? test : test.skip

test.before(async () => {
  if (!READY) return
  await ui.ensureApp()
})

test.after(async () => {
  await ui.closeBrowser()
  await ui.shutdown()
})

/* ---- 1. server contract ------------------------------------------------ */

t('healthz reports ok / railway-rs', { timeout: 15000 }, async () => {
  const r = await fetch(`${ui.BASE_URL}/healthz`)
  assert.equal(r.status, 200)
  const j = await r.json()
  assert.equal(j.status, 'ok')
  assert.equal(j.service, 'railway-rs')
})

/* ---- 2-4 + 8. route tour: content, outline, layout, hygiene ------------ */
/* One test per route keeps failures localized; each test tours every
   viewport so mobile blowouts are caught next to their desktop twins. */

for (const route of ui.ROUTES) {
  t(`route ${route}: renders, no overflow, scrolls clean, never throws`, { timeout: 90000 }, async () => {
    for (const vp of ui.VIEWPORTS) {
      const s = await ui.openPage({ path: route, viewport: vp })
      try {
        const body = await s.text()
        const controls = await s.page.evaluate(
          () =>
            document.querySelectorAll(
              'main a[href], main button, main input, main select, main textarea',
            ).length,
        )
        // Sparse-but-functional pages (e.g. /extras on phones hide the
        // description line) are valid: mounted = text OR live controls.
        assert.ok(
          body.trim().length > 40 || controls > 0,
          `[${vp.label}] ${route} rendered neither text nor controls`,
        )

        const h1 = await s.page.evaluate(() => document.querySelectorAll('main h1').length)
        assert.equal(h1, 1, `[${vp.label}] ${route} must have exactly one <h1>, found ${h1}`)

        await ui.assertNoHorizontalOverflow(s)
        await ui.assertVerticalScrollWorks(s)
        await ui.assertNoPageErrors(s)
        await ui.assertNoConsoleErrors(s)
      } finally {
        await s.close()
      }
    }
  })
}

/* ---- 5. accessibility basics on the form-heavy routes ------------------ */

t('form controls and buttons have accessible names', { timeout: 60000 }, async () => {
  for (const route of ['/train', '/station', '/availability', '/pnr']) {
    const s = await ui.openPage({ path: route, viewport: ui.VIEWPORTS[0] })
    try {
      await ui.assertControlsAreLabelled(s)
      await ui.assertButtonsAreNamed(s)
    } finally {
      await s.close()
    }
  }
})

/* ---- 6. the core live-status flow -------------------------------------- */
/* The app's promise: submit a train number and you get EITHER live data OR
   an explicit honest error - never a silent blank, never fabricated rows. */

t('train flow: Track 12951 ends in data or an honest error', { timeout: 60000 }, async () => {
  const s = await ui.openPage({ path: '/train', viewport: ui.VIEWPORTS[0] })
  try {
    await s.page.fill('#train-no', '12951')
    await s.page.getByRole('button', { name: 'Track', exact: true }).click()

    // Resolve = data card with the train number badge, or role=alert error.
    await s.page.waitForSelector('[role="alert"], main >> text=12951', { timeout: 40000 })
    const state = await s.page.evaluate(() => ({
      alert: document.querySelector('main [role="alert"]')?.textContent ?? null,
      busy: !!document.querySelector('main [aria-busy="true"]'),
      text: document.querySelector('main').innerText.length,
    }))
    assert.ok(
      state.alert !== null || state.text > 120,
      `flow stalled: neither data nor error rendered (busy=${state.busy}, len=${state.text})`,
    )
    await ui.assertNoPageErrors(s)
  } finally {
    await s.close()
  }
})

/* ---- 7. theme persistence ---------------------------------------------- */

t('theme choice applies html class and survives reload', { timeout: 45000 }, async () => {
  const s = await ui.openPage({ path: '/', viewport: ui.VIEWPORTS[0] })
  try {
    await s.page.evaluate(() => localStorage.setItem('rc-theme', 'dark'))
    await s.goto('/')
    const cls = await s.page.evaluate(() => document.documentElement.className)
    assert.equal(cls, 'dark', `html should carry the dark class, got "${cls}"`)
    const stored = await s.page.evaluate(() => localStorage.getItem('rc-theme'))
    assert.equal(stored, 'dark', 'rc-theme should persist')
    // restore default for other sessions on this profile-free context
    await s.page.evaluate(() => localStorage.removeItem('rc-theme'))
  } finally {
    await s.close()
  }
})
