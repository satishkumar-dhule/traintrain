/* tokens.test.mjs - TDD gate for the "Signal & Steel" palette.
   Pins the identity tokens in frontend/src/app.css, verifies the font
   wiring, and computes WCAG contrast from oklch literals so the palette
   cannot drift below AA. Runs with: node --test tests/js/ */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const fs = require('node:fs');
const path = require('node:path');

const FRONTEND = path.resolve(path.dirname(require.resolve('../../package.json')), 'frontend');

const CSS = fs.readFileSync(path.join(FRONTEND, 'src/app.css'), 'utf8');
const MAIN = fs.readFileSync(path.join(FRONTEND, 'src/main.js'), 'utf8');

/* ---------- oklch -> sRGB -> WCAG contrast ---------- */

function oklchToLinearSrgb(L, C, Hdeg) {
  const h = (Hdeg * Math.PI) / 180;
  const a = C * Math.cos(h);
  const b = C * Math.sin(h);
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ ** 3,
    m = m_ ** 3,
    s = s_ ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s
  ];
}

function oklchLuminance(L, C, H) {
  /* WCAG relative luminance weights the LINEAR-light channels directly
     (same as tests/js/contrast.test.mjs does after its hex decode). */
  const [r, g, b] = oklchToLinearSrgb(L, C, H).map((c) => Math.min(1, Math.max(0, c)));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Parse the first `oklch(L C H)` literal out of a token declaration. */
function tokenOklch(scope, name) {
  const block = scope === 'root' ? rootBlock() : darkBlock();
  const m = new RegExp(`--${name}:\\s*oklch\\(\\s*([\\d.]+)\\s+([\\d.]+)\\s+([\\d.]+)\\s*\\)`).exec(block);
  assert.ok(m, `${scope} --${name} must be a plain oklch(L C H) literal`);
  return [+m[1], +m[2], +m[3]];
}

let _blocks = null;
function themeBlocks() {
  if (!_blocks) {
    _blocks = { root: /:root\s*{([^}]*)}/.exec(CSS)?.[1] ?? '', dark: /\.dark\s*{([^}]*)}/.exec(CSS)?.[1] ?? '' };
  }
  return _blocks;
}
const rootBlock = () => themeBlocks().root;
const darkBlock = () => themeBlocks().dark;

function contrastPair(fg, bg) {
  const l1 = oklchLuminance(...fg);
  const l2 = oklchLuminance(...bg);
  const hi = Math.max(l1, l2);
  const lo = Math.min(l1, l2);
  return (hi + 0.05) / (lo + 0.05);
}

/* ---------- identity pins ---------- */

test('coach indigo primary is pinned in both themes', () => {
  assert.match(rootBlock(), /--primary:\s*oklch\(0\.40 0\.13 272\)/);
  assert.match(darkBlock(), /--primary:\s*oklch\(0\.68 0\.135 272\)/);
});

test('background leaves the default zinc axis for blue steel', () => {
  assert.match(rootBlock(), /--background:\s*oklch\(0\.975 0\.005 255\)/);
  assert.match(darkBlock(), /--background:\s*oklch\(0\.165 0\.02 265\)/);
});

test('signal lamp trio exists in :root AND .dark', () => {
  for (const name of ['signal-go', 'signal-hold', 'signal-stop']) {
    assert.ok(tokenOklch('root', name), `:root --${name}`);
    assert.ok(tokenOklch('dark', name), `.dark --${name}`);
  }
});

test('saffron brand accent exists in both themes', () => {
  assert.ok(tokenOklch('root', 'saffron'));
  assert.ok(tokenOklch('dark', 'saffron'));
});

/* ---------- typography wiring ---------- */

test('fontsource packages are imported in main.js', () => {
  assert.match(MAIN, /@fontsource-variable\/archivo\/wdth\.css/);
  assert.match(MAIN, /@fontsource-variable\/jetbrains-mono\/index\.css/);
});

test('@theme wires --font-display and --font-mono to the variable faces', () => {
  assert.match(CSS, /--font-display:\s*'Archivo Variable'/);
  assert.match(CSS, /--font-mono:\s*'JetBrains Mono Variable'/);
});

test('signature utilities exist: signage, track-rule, data-num', () => {
  assert.match(CSS, /@utility signage/);
  assert.match(CSS, /@utility track-rule/);
  assert.match(CSS, /@utility data-num/);
});

test('lamp-pulse animation respects prefers-reduced-motion', () => {
  assert.match(CSS, /\.lamp-pulse\s*\{[^}]*animation: lamp-pulse/, 'lamp-pulse must exist');
  const reduceBlocks = CSS.match(/@media \(prefers-reduced-motion: reduce\) \{[\s\S]*?\n\}/g) ?? [];
  const killsLamp = reduceBlocks.some((b) => b.includes('.lamp-pulse') && b.includes('animation: none'));
  assert.ok(killsLamp, 'reduced motion must kill lamp-pulse');
});

/* ---------- WCAG AA on computed pairs ---------- */

test('primary carries its foreground at AA (both themes)', () => {
  for (const scope of ['root', 'dark']) {
    const ratio = contrastPair(tokenOklch(scope, 'primary-foreground'), tokenOklch(scope, 'primary'));
    assert.ok(ratio >= 4.5, `${scope} primary ratio ${ratio.toFixed(2)} < 4.5`);
  }
});

test('muted-foreground text passes AA on background (both themes)', () => {
  for (const scope of ['root', 'dark']) {
    const ratio = contrastPair(tokenOklch(scope, 'muted-foreground'), tokenOklch(scope, 'background'));
    assert.ok(ratio >= 4.5, `${scope} muted-fg ratio ${ratio.toFixed(2)} < 4.5`);
  }
});

test('foreground passes AA on background and card (both themes)', () => {
  for (const scope of ['root', 'dark']) {
    for (const surface of ['background', 'card']) {
      const ratio = contrastPair(tokenOklch(scope, 'foreground'), tokenOklch(scope, surface));
      assert.ok(ratio >= 7, `${scope} fg/${surface} ratio ${ratio.toFixed(2)} < 7 (AAA body)`);
    }
  }
});
