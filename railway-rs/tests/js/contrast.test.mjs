/* contrast.test.mjs - unit tests for static/styles.css dark-mode contrast.
   Guards the --primary-strong token (dark blue in both modes), pins
   --on-primary-strong to pure white (so color: var(--on-primary-strong)
   counts as white text), and keeps the rule that white text must never sit
   on a plain var(--primary) background.
   Runs with the built-in Node test runner: `node --test tests/js/`. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const fs = require('node:fs');

const CSS = fs.readFileSync(require.resolve('../../static/styles.css'), 'utf8');

function srgbToLinear(c) {
  const s = c / 255;
  return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

function luminance(hex) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  assert.ok(m, `invalid hex color: ${hex}`);
  const r = parseInt(m[1], 16);
  const g = parseInt(m[2], 16);
  const b = parseInt(m[3], 16);
  return 0.2126 * srgbToLinear(r) + 0.7152 * srgbToLinear(g) + 0.0722 * srgbToLinear(b);
}

function contrast(fg, bg) {
  const l1 = luminance(fg);
  const l2 = luminance(bg);
  const hi = Math.max(l1, l2);
  const lo = Math.min(l1, l2);
  return (hi + 0.05) / (lo + 0.05);
}

/* Split into rule blocks (up to the next `}`) for per-block scans. */
function blocks() {
  return CSS.split('}');
}

/* A block must declare a white-ish text color (color: #fff / #ffffff / white,
   or the pinned-white --on-primary-strong token). */
const WHITE_COLOR = /(?:^|[{;])\s*color:\s*(?:(?:#fff|#ffffff|white)\b|var\(--on-primary-strong\))/;
const PRIMARY_BG = /background:\s*var\(--primary\)/;
const PRIMARY_STRONG_BG = /background:\s*var\(--primary-strong\)/;

test('dark-mode token --primary-strong exists and stays dark blue', () => {
  assert.match(CSS, /--primary-strong:\s*light-dark\(#1e40af,\s*#1d4ed8\);/);
});

test('--on-primary-strong is pinned to pure white', () => {
  assert.match(CSS, /--on-primary-strong:\s*#ffffff;/);
});

test('white text passes WCAG AA on both --primary-strong shades', () => {
  const ratios = {
    '#fff vs #1e40af (light)': contrast('#ffffff', '#1e40af'),
    '#fff vs #1d4ed8 (dark)': contrast('#ffffff', '#1d4ed8'),
  };
  for (const [label, ratio] of Object.entries(ratios)) {
    assert.ok(ratio >= 4.5, `${label} ratio ${ratio.toFixed(2)}:1 is below 4.5`);
  }
});

test('every --primary-strong background pairs with white text', () => {
  const offenders = blocks().filter((b) => PRIMARY_STRONG_BG.test(b) && !WHITE_COLOR.test(b));
  assert.deepEqual(offenders, [], 'rule blocks with var(--primary-strong) background must also declare white text');
});

test('no rule block pairs a plain --primary background with white text', () => {
  const offenders = blocks().filter((b) => PRIMARY_BG.test(b) && WHITE_COLOR.test(b));
  assert.deepEqual(offenders, [], 'white text must not sit on a plain var(--primary) background');
});