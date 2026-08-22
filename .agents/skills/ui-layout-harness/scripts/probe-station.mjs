#!/usr/bin/env node
// Probe geometry of the station-board form row to find the overflow culprit.
import { loadPlaywright, launch, HOME } from './lib.mjs'

const BASE = 'http://localhost:3000'
const pw = await loadPlaywright()
const browser = await launch(pw)
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
await page.goto(BASE + '/station/NDLS/live', { waitUntil: 'networkidle' })
await page.waitForTimeout(300)

const data = await page.evaluate(() => {
  const pick = (el) => {
    if (!el) return null
    const r = el.getBoundingClientRect()
    const cs = getComputedStyle(el)
    return {
      x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height),
      right: Math.round(r.right), display: cs.display, maxW: cs.maxWidth, width: cs.width,
      minW: cs.minWidth, overflow: cs.overflowX, cls: el.className?.slice?.(0, 120) || el.tagName,
    }
  }
  const out = {}
  // the wrapper around the DateStrip (label "Date (timetable)")
  out.cardContent = [...document.querySelectorAll('main [class*="flex flex-wrap"]')]
    .map((el, i) => ({ i, ...pick(el), kids: el.children.length }))
  // find the label then walk up
  const label = [...document.querySelectorAll('label')].find((l) => /timetable/i.test(l.textContent))
  out.dateWrapper = label ? pick(label.parentElement.parentElement || label.parentElement) : null
  if (label) {
    let strip = label.nextElementSibling
    out.stripRoot = pick(strip)
    if (strip) {
      out.chipsScroller = pick(strip.querySelector('.overflow-x-auto'))
      const chips = strip.querySelectorAll('.overflow-x-auto > button')
      out.chipCount = chips.length
      out.firstChip = pick(chips[0])
      out.lastChip = chips.length ? pick(chips[chips.length - 1]) : null
      out.calInput = pick(strip.querySelector('input[type="date"]'))
      out.chevrons = [...strip.querySelectorAll('button[aria-label*="day"]')].map(pick)
    }
    // show board button box for overlap check
    const btn = [...document.querySelectorAll('button')].find((b) => /show board/i.test(b.textContent))
    out.showBoard = pick(btn)
  }
  return out
})
console.log(JSON.stringify(data, null, 2))
await browser.close()
