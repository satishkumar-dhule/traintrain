/* embed.test.mjs - unit tests for the semantic fallback tier's PURE vector
   core (l2normalize / cosine / pickByMargin) in embed.js. These run with zero
   network and never trigger the module's lazy loaders: the CDN model load
   sits behind initEmbedder() and the gate.js import behind getGate(), so
   importing the pure helpers under Node executes neither. The async tier
   (embedClassify) is browser-only by design and is exercised in-browser. */

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  l2normalize,
  cosine,
  pickByMargin,
  initEmbedder,
  embedTexts,
  getIntentVectors,
  embedClassify,
} from '../../frontend/src/lib/chat/embed.js';

test('module loads clean in Node: pure core exported, lazy loaders untouched', () => {
  for (const fn of [l2normalize, cosine, pickByMargin, initEmbedder, embedTexts, getIntentVectors, embedClassify]) {
    assert.equal(typeof fn, 'function');
  }
});

test('cosine: identity is 1, orthogonality is 0, opposition is -1', () => {
  const e1 = [1, 0, 0];
  const e2 = [0, 1, 0];
  assert.ok(Math.abs(cosine(e1, e1) - 1) < 1e-12);
  assert.equal(cosine(e1, e2), 0);
  assert.ok(Math.abs(cosine(e1, [-1, 0, 0]) + 1) < 1e-12);
});

test('cosine: order/scale invariant', () => {
  const a = [3, 4];
  const b = [-8, 6]; // unit-ish vectors scaled arbitrarily
  const direct = cosine(a, b);
  const scaled = cosine(a.map((x) => x * 7), b.map((x) => x * 0.01));
  assert.ok(Math.abs(direct - scaled) < 1e-12);
  assert.equal(cosine(b, a), direct);
});

test('l2normalize: output has unit L2 norm and preserves direction', () => {
  const v = [3, 4];
  const n = l2normalize(v);
  assert.ok(Math.abs(n[0] - 0.6) < 1e-12);
  assert.ok(Math.abs(n[1] - 0.8) < 1e-12);
  let sum = 0;
  for (const x of n) sum += x * x;
  assert.ok(Math.abs(sum - 1) < 1e-12);
  assert.ok(Math.abs(cosine(v, n) - 1) < 1e-12);
});

test('l2normalize: does not mutate input, handles zero vector without NaN', () => {
  const v = new Float32Array([3, 4]);
  l2normalize(v);
  assert.deepEqual(Array.from(v), [3, 4]);
  const zero = l2normalize([0, 0, 0]);
  assert.deepEqual(Array.from(zero), [0, 0, 0]);
  assert.ok(zero.every((x) => Number.isFinite(x)));
});

test('pickByMargin: accepts a clear winner above floor and margin', () => {
  const q = [1, 0, 0];
  const items = [
    { id: 'live_status', vec: [1, 0, 0] },
    { id: 'train_schedule', vec: [0, 1, 0] },
    { id: 'average_delay', vec: [0, 0, 1] },
  ];
  const hit = pickByMargin(q, items);
  assert.ok(hit);
  assert.equal(hit.id, 'live_status');
  assert.ok(Math.abs(hit.score - 1) < 1e-12);
  assert.ok(hit.gap >= 0.08);
});

test('pickByMargin: rejects when best score is below the floor', () => {
  const q = [1, 0];
  const orthogonalOnly = [{ id: 'a', vec: [0, 1] }, { id: 'b', vec: [-1, 0] }];
  assert.equal(pickByMargin(q, orthogonalOnly), null); // scores 0 and -1

  // Floor is enforced independently of margin: clear winner but high floor.
  const q2 = [1, 0];
  const near = [{ id: 'win', vec: [Math.sqrt(0.9), Math.sqrt(0.1)] }];
  assert.equal(pickByMargin(q2, near, { floor: 0.99 }), null);
  assert.ok(pickByMargin(q2, near)); // default floor passes
});

test('pickByMargin: rejects when margin over runner-up is too small', () => {
  // Both vectors normalized; cosines vs [1,0] are exactly .9 and .85.
  const q = [1, 0];
  const items = [
    { id: 'best', vec: [0.9, Math.sqrt(1 - 0.81)] },
    { id: 'second', vec: [0.85, Math.sqrt(1 - 0.7225)] },
  ];
  const hit = pickByMargin(q, items);
  assert.equal(hit, null); // gap = .05 < .08
  assert.ok(pickByMargin(q, items, { margin: 0.04 })); // relaxed margin accepts
});

test('pickByMargin: ties are rejected even at perfect score', () => {
  const q = [0, 1];
  const tied = [
    { id: 'a', vec: [0, 1] },
    { id: 'b', vec: [0, 1] },
  ];
  assert.equal(pickByMargin(q, tied), null); // gap = 0
});

test('pickByMargin: single item uses score itself as gap', () => {
  const single = [{ id: 'only', vec: [1, 0, 0] }];
  const hit = pickByMargin([1, 0, 0], single);
  assert.ok(hit);
  assert.equal(hit.id, 'only');
  assert.equal(hit.gap, hit.score);
  assert.equal(pickByMargin([0, 1, 0], single), null); // below floor AND margin
});

test('pickByMargin: empty candidate list yields null', () => {
  assert.equal(pickByMargin([1, 0], []), null);
});
