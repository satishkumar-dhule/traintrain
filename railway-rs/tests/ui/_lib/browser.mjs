// browser.mjs - one browser for the suite, one isolated page per test.
// Each session captures console errors and uncaught page errors so specs can
// assert "the app never throws" alongside behavioral expectations.
import { loadPlaywright, findShell, browserEnv } from './env.mjs'

let _pw = null
let _browser = null

export async function sharedBrowser() {
  if (_browser) return _browser
  _pw ||= await loadPlaywright()
  _browser = await _pw.chromium.launch({
    executablePath: findShell(),
    env: browserEnv(),
    args: ['--no-sandbox', '--disable-gpu', '--force-device-scale-factor=1'],
  })
  return _browser
}

export async function closeBrowser() {
  if (_browser) await _browser.close()
  _browser = null
}

/* Open a fresh page at `path` in `viewport`, wait until <main> has content,
   and return a session handle:
     { page, errors, goto, text, close }
   - errors: { console: string[], pageerror: Error[] } captured so far
   - goto(p): navigate + auto-wait for main (reuses the same capture arrays)
   Network noise (failed fetches to unreachable upstreams) is expected by
   design — the app surfaces honest errors instead of throwing. */
export async function openPage({ path = '/', viewport }) {
  const browser = await sharedBrowser()
  const context = await browser.newContext({ viewport })
  const page = await context.newPage()
  const errors = { console: [], pageerror: [] }

  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.console.push(msg.text())
  })
  page.on('pageerror', (err) => errors.pageerror.push(err))

  const goto = async (p, { waitMain = true } = {}) => {
    await page.goto(`${origin()}${p}`, { waitUntil: 'domcontentloaded', timeout: 20000 })
    if (waitMain) await waitForMain(page)
    return page
  }

  await goto(path)

  return {
    page,
    errors,
    goto,
    viewportName: `${viewport.width}x${viewport.height}`,
    text: () => page.locator('main').innerText(),
    close: async () => {
      await context.close()
    },
  }
}

function origin() {
  return process.env.UI_BASE_URL || `http://localhost:${process.env.UI_PORT || '3000'}`
}

/* The SPA mounts into #app; give Svelte up to 10s on cold loads. */
async function waitForMain(page) {
  try {
    await page.waitForSelector('main', { timeout: 10000 })
    await page.waitForFunction(
      () => {
        const m = document.querySelector('main')
        return m && m.innerText.trim().length > 20
      },
      { timeout: 10000 },
    )
  } catch {
    /* let individual specs assert what they need; empty-main failures are
       diagnosed by assert helpers with a DOM snapshot */
  }
}
