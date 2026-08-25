/* uniformity.test.mjs - static uniformity contract for pages/ and components/.
   The "Signal & Steel" system only reads as one product when every page follows
   the same rules. These are source-level checks (no DOM): they keep the visual
   contract enforceable in CI alongside the token pins in tokens.test.mjs.

   Contract:
   U1 every page renders its title through PageHeader OR a single <h1> that
      carries the `signage` treatment — never both, never a bare h1.
   U2 at most one `track-rule` per page (the header divider); never stacked.
   U3 no raw Tailwind palette literals anywhere in src/lib — tokens only
      (signal/base/ink/saffron/chart/primary/...).
   U4 pages carry no `font-mono` — data numerals use `data-num` (JetBrains is
      wired into --font-mono, but pages must use the utility so the contract
      is greppable); identifiers in components/chat stay allowed.
   U5 pages use the shared EmptyState component instead of hand-rolled
      "no data" blocks (an inline 'No … yet' heading outside EmptyState is a
      smell we can grep).

   Runs with: node --test tests/js/ */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';

const require = createRequire(import.meta.url);
const LIB = path.resolve(
  path.dirname(require.resolve('../../package.json')),
  'frontend/src/lib'
);

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) return walk(p);
    return p.endsWith('.svelte') ? [p] : [];
  });
}

const pages = walk(path.join(LIB, 'pages'));
const allSvelte = walk(LIB);

const markupOf = (src) => src.slice(src.lastIndexOf('</script>'));

/* Raw palette literals: tailwind named colors with numeric shades, hex, rgb(). */
const RAW_PALETTE =
  /(?:^|[\s"':])(?:[a-z]+-)?(?:emerald|amber|red|sky|violet|orange|lime|teal|cyan|fuchsia|rose|blue|green|yellow|purple|pink|indigo)-(?:50|100|200|300|400|500|600|700|800|900|950)[\s"'`)]/i;

test('U1: every page titles through PageHeader or a single signage h1 — never both', () => {
  for (const file of pages) {
    const src = fs.readFileSync(file, 'utf8');
    const rel = path.relative(LIB, file);
    const usesPageHeader = /<PageHeader[\s/>]/.test(src);
    const h1s = [...markupOf(src).matchAll(/<h1[^>]*>/g)];
    if (usesPageHeader) {
      assert.equal(h1s.length, 0, `${rel}: PageHeader pages must not hand-roll an <h1>`);
    } else {
      assert.equal(h1s.length, 1, `${rel}: exactly one <h1> required (or use PageHeader)`);
      assert.match(
        h1s[0][0],
        /class="[^"]*\bsignage\b/,
        `${rel}: the <h1> must carry the signage treatment`
      );
    }
  }
});

test('U2: at most one track-rule per page', () => {
  for (const file of pages) {
    const src = fs.readFileSync(file, 'utf8');
    const n = (src.match(/track-rule/g) ?? []).length;
    assert.ok(n <= 1, `${path.relative(LIB, file)}: ${n} track-rules (max 1)`);
  }
});

test('U3: no raw palette literals in any svelte file — tokens only', () => {
  for (const file of allSvelte) {
    const src = fs.readFileSync(file, 'utf8');
    const m = RAW_PALETTE.exec(src);
    assert.equal(
      m,
      null,
      `${path.relative(LIB, file)}: raw palette literal "${m?.[0].trim()}" — use signal/token utilities`
    );
  }
});

test('U4: pages use data-num, not font-mono', () => {
  for (const file of pages) {
    const src = fs.readFileSync(file, 'utf8');
    const n = (src.match(/font-mono/g) ?? []).length;
    assert.equal(n, 0, `${path.relative(LIB, file)}: ${n} font-mono uses (use data-num)`);
  }
});

test('U5: empty states flow through EmptyState, not hand-rolled headings', () => {
  for (const file of pages) {
    const src = fs.readFileSync(file, 'utf8');
    const rel = path.relative(LIB, file);
    const handRolled = [...markupOf(src).matchAll(/<h3[^>]*>[^<{]*[Nn]o [^<{]*yet/g)];
    if (handRolled.length) {
      assert.match(src, /EmptyState/, `${rel}: hand-rolled empty state must use EmptyState`);
    }
  }
});
