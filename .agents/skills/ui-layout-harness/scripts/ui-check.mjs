#!/usr/bin/env node
// Screenshot + overflow diagnostics for pages served on localhost:3000.
//
//   node ui-check.mjs --path /availability --vp 1440x900,390x844 \
//     --step 'fill:#av-from=NDLS' --step 'click:text=Search' --step 'wait:4000' \
//     --step 'shot:results'
//
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { loadPlaywright, launch, diagnose, logDiag, HOME } from './lib.mjs'

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`)
  return i >= 0 ? process.argv[i + 1] : def
}
const BASE = arg('url', 'http://localhost:3000')
const PAGE_PATH = arg('path', '/')
const VPS = (arg('vp', '1440x900,1280x800,820x1180,390x844')
  .split(',')
  .map((s) => s.split('x').map(Number)))
const STEPS = process.argv.filter((a, i) => process.argv[i - 1] === '--step')
const FULL = process.argv.includes('--full')
const OUT = path.join(HOME, 'shots', PAGE_PATH.replace(/[^a-z0-9]+/gi, '_') || 'root')
fs.mkdirSync(OUT, { recursive: true })

const pw = await loadPlaywright()
const browser = await launch(pw)

async function clickButton(page, val) {
  // Prefer buttons inside <main> — global headers often carry same-named icons.
  const scoped = page.locator('main').getByRole('button', { name: val, exact: true }).first()
  const target = (await scoped.count()) ? scoped : page.getByRole('button', { name: val, exact: true }).first()
  await target
    .click()
    .catch(async () => {
      await target.click({ force: true })
      console.log(`  NOTE: '${val}' needed force:true (covered by a sticky/overlay element?)`)
    })
}

async function runStep(page, step, vpLabel) {
  const [op, ...rest] = step.split(':')
  const val = rest.join(':')
  if (op === 'fill') {
    const [sel, ...v] = val.split('=')
    await page.fill(sel.trim(), v.join('='))
  } else if (op === 'click') await clickButton(page, val)
  else if (op === 'clicktext')
    await page
      .getByText(val)
      .first()
      .click()
      .catch(async () => {
        await page.getByText(val).first().click({ force: true })
        console.log(`  NOTE: text '${val}' needed force:true (covered by a sticky/overlay element?)`)
      })
  else if (op === 'esc') await page.keyboard.press('Escape')
  else if (op === 'press') await page.keyboard.press(val)
  else if (op === 'wait') await page.waitForTimeout(+val || 500)
  else if (op === 'shot') await page.screenshot({ path: `${OUT}/${val}-${vpLabel}.png`, fullPage: true })
}

let failures = 0
for (const [w, h] of VPS) {
  const label = `${w}x${h}`
  const page = await browser.newPage({ viewport: { width: w, height: h } })
  try {
    await page.goto(BASE + PAGE_PATH, { waitUntil: 'networkidle' })
    await page.waitForTimeout(300)
    await page.screenshot({ path: `${OUT}/idle-${label}.png`, fullPage: FULL })
    for (const step of STEPS) await runStep(page, step, label)
    const d = await diagnose(page)
    logDiag(label, d)
    if (d.hOverflow || d.offenders.length) failures++
    const v = await page.evaluate(() => {
      window.scrollTo(0, 400)
      return { y: window.scrollY, max: document.documentElement.scrollHeight - window.innerHeight }
    })
    console.log(
      `  v-scroll: ${v.max > 0 ? `scrolled to ${v.y} of ${v.max}${v.y === 0 ? '  <-- BROKEN' : ''}` : 'page fits viewport'}`,
    )
  } catch (e) {
    failures++
    console.log(`== ${label} == FAILED: ${String(e.message).split('\n').slice(0, 5).join(' | ').slice(0, 400)}`)
  }
  await page.close()
}
await browser.close()
console.log(failures ? `\n${failures} viewport(s) with issues — shots in ${OUT}` : `\nall viewports clean — shots in ${OUT}`)
process.exit(failures ? 1 : 0)
