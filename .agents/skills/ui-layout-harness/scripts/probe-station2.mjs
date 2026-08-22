#!/usr/bin/env node
// Probe the stn-date wrapper + strip ancestors precisely.
import { loadPlaywright, launch } from './lib.mjs'

const pw = await loadPlaywright()
const browser = await launch(pw)
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
await page.goto('http://localhost:3000/station/NDLS/live', { waitUntil: 'networkidle' })
await page.waitForTimeout(300)

const data = await page.evaluate(() => {
  const box = (el) => {
    if (!el) return null
    const r = el.getBoundingClientRect()
    const cs = getComputedStyle(el)
    return {
      tag: el.tagName, x: Math.round(r.x), w: Math.round(r.width), right: Math.round(r.right),
      display: cs.display, width: cs.width, maxW: cs.maxWidth, minW: cs.minWidth,
      flexShrink: cs.flexShrink, flexBasis: cs.flexBasis, gridCols: cs.gridTemplateColumns,
      cls: String(el.className).slice(0, 100),
    }
  }
  const label = document.querySelector('label[for="stn-date"]')
  if (!label) return { error: 'no label' }
  const wrap = label.parentElement
  const strip = label.nextElementSibling
  const chain = []
  let el = strip
  for (let i = 0; i < 5 && el; i++) {
    chain.push(box(el))
    el = el.parentElement
  }
  return { wrapper: box(wrap), ancestorChainFromStrip: chain }
})
console.log(JSON.stringify(data, null, 2))
await browser.close()
