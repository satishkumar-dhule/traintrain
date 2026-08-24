/* memory.test.mjs - session replay cache contracts. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const memory = require('../../frontend/src/lib/chat/memory.js');

const answer = (content) => ({ content, cards: [], actions: [] });

test('exact normalized replay hits regardless of punctuation/case/filler', () => {
  const m = memory.createMemory();
  memory.remember(m, 'live status of 12951', answer('it is on time'));
  const hit = memory.findReplay(m, 'Live status of 12951!');
  assert.ok(hit);
  assert.equal(hit.exact, true);
  assert.equal(hit.entry.answer.content, 'it is on time');
});

test('similar question with same entities replays; different entities do not', () => {
  const m = memory.createMemory();
  memory.remember(m, 'what is the live running status of train 12951 right now', answer('on time'));

  const similar = memory.findReplay(m, 'tell me the live running status of the 12951 train please');
  assert.ok(similar, 'similar should hit');
  assert.equal(similar.exact, false);

  assert.equal(memory.findReplay(m, 'what is the live running status of train 22691 right now'), null,
    'different train number must not replay');
});

test('short or unseen queries never replay', () => {
  const m = memory.createMemory();
  memory.remember(m, 'trains from secunderabad to pune today', answer('many'));
  assert.equal(memory.findReplay(m, 'ok'), null);
  assert.equal(memory.findReplay(m, 'completely unrelated question about food menus'), null);
  assert.equal(memory.findReplay(m, ''), null);
});

test('expired TTL entries are not replayed (live data goes stale)', async () => {
  const m = memory.createMemory();
  memory.remember(m, 'live status of 12951', answer('on time'), { ttlMs: 50 });
  assert.ok(memory.findReplay(m, 'live status of 12951'), 'fresh within ttl');
  await new Promise((r) => setTimeout(r, 60));
  assert.equal(memory.findReplay(m, 'live status of 12951'), null, 'expired -> refetch');
});

test('memory is capped at max entries, newest first', () => {
  const m = memory.createMemory(3);
  for (let i = 0; i < 5; i++) memory.remember(m, `question number ${i}`, answer(`a${i}`));
  assert.equal(m.entries.length, 3);
  assert.equal(m.entries[0].answer.content, 'a4');
});
