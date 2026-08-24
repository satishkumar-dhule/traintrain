// env.mjs - where the harness lives, which app to test, viewport matrix.
// Every knob is an env override so CI and local runs share one code path.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

/* railway-rs root (tests/ui/_lib -> up 3) */
export const ROOT = fileURLToPath(new URL('../../..', import.meta.url))

/* playwright-core + chromium-headless-shell home (zero host installs) */
export const HARNESS_HOME = process.env.UI_HARNESS_HOME || '/tmp/opencode/ui-harness'

/* Base URL of the app under test. Default: the standard :3000 dev server. */
const PORT = process.env.UI_PORT || '3000'
export const BASE_URL = process.env.UI_BASE_URL || `http://localhost:${PORT}`

/* Viewport matrix. Keep it small enough to stay fast, wide enough to catch
   mobile blowouts: desktop + a narrow phone. Override with UI_VIEWPORTS
   ("1440x900:desktop,390x844:mobile"). */
export const VIEWPORTS = (
  process.env.UI_VIEWPORTS || '1440x900:desktop,390x844:mobile'
)
  .split(',')
  .map((s) => {
    const [wh, label] = s.trim().split(':')
    const [width, height] = wh.split('x').map(Number)
    return { width, height, label }
  })

/* Every primary route the SPA must render without errors or overflow.
   Mirrors the nav table in frontend/src/lib/Layout.svelte. */
export const ROUTES = (process.env.UI_ROUTES ||
  '/,/train,/station,/journeys,/availability,/pnr,/exceptions,/extras,/assistant,/system,/about'
).split(',')

export function loadPlaywright() {
  return import(path.join(HARNESS_HOME, 'node_modules', 'playwright-core', 'index.mjs'))
}

export function findShell() {
  try {
    for (const d of fs.readdirSync(path.join(HARNESS_HOME, 'browsers'))) {
      const p = path.join(HARNESS_HOME, 'browsers', d, 'chrome-linux', 'headless_shell')
      if (fs.existsSync(p)) return p
    }
  } catch {
    /* harness home missing entirely */
  }
  return null
}

export function browserEnv() {
  return {
    ...process.env,
    LD_LIBRARY_PATH: `${HARNESS_HOME}/debroot/usr/lib/x86_64-linux-gnu:${HARNESS_HOME}/debroot/lib/x86_64-linux-gnu`,
    FONTCONFIG_PATH: `${HARNESS_HOME}/debroot/etc/fonts`,
  }
}

/* True when the private browser tree exists. When false the suite skips with
   setup instructions instead of failing machines that never bootstrapped. */
export function browserReady() {
  return (
    fs.existsSync(path.join(HARNESS_HOME, 'node_modules', 'playwright-core')) &&
    findShell() !== null
  )
}
