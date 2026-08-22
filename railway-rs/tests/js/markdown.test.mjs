/* markdown.test.mjs - unit tests for frontend/src/lib/markdown.js.
   The renderer is security-critical ({@html} injection target): these tests
   pin the escaping model as well as the supported Markdown subset. */

import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const md = require('../../frontend/src/lib/markdown.js');

test('bold, italic and inline code render with classes', () => {
  const html = md.renderMarkdown('**late** by *12 min* at `BCT`');
  assert.match(html, /<strong>late<\/strong>/);
  assert.match(html, /<em>12 min<\/em>/);
  assert.match(html, /<code class="md-code">BCT<\/code>/);
});

test('fenced code blocks escape their body verbatim', () => {
  const html = md.renderMarkdown('```\n<script>alert(1)</script>\n```');
  assert.match(html, /<pre class="md-pre"><code>/);
  assert.doesNotMatch(html, /<script>/);
  assert.match(html, /&lt;script&gt;/);
});

test('tables render thead/tbody from pipe rows', () => {
  const html = md.renderMarkdown(
    '| Train | Dep |\n|---|---|\n| 12951 | 17:40 |'
  );
  assert.match(html, /<table class="md-table">/);
  assert.match(html, /<th>Train<\/th>/);
  assert.match(html, /<td>12951<\/td>/);
});

test('bullet and numbered lists group consecutive lines', () => {
  const html = md.renderMarkdown('- one\n- two\n\n1. first\n2. second');
  assert.match(html, /<ul class="md-ul"><li>one<\/li><li>two<\/li><\/ul>/);
  assert.match(html, /<ol class="md-ol"><li>first<\/li><li>second<\/li><\/ol>/);
});

test('headings map to bubble-safe levels', () => {
  const html = md.renderMarkdown('# Title\n## Sub');
  assert.match(html, /<h3 class="md-h">Title<\/h3>/);
  assert.match(html, /<h4 class="md-h">Sub<\/h4>/);
});

test('raw html is escaped everywhere outside code too', () => {
  const html = md.renderMarkdown('<img src=x onerror=alert(1)> **hi**');
  assert.doesNotMatch(html, /<img/);
  assert.match(html, /&lt;img src=x onerror=alert\(1\)&gt;/);
  assert.match(html, /<strong>hi<\/strong>/);
});

test('javascript: links are stripped to plain text; https links pass', () => {
  const bad = md.renderMarkdown('[click](javascript:alert(1))');
  assert.doesNotMatch(bad, /href="javascript/);

  const good = md.renderMarkdown('[site](https://example.com/a)');
  assert.match(good, /<a href="https:\/\/example.com\/a" target="_blank" rel="noopener noreferrer">site<\/a>/);
});

test('empty and null input yield empty string without throwing', () => {
  assert.equal(md.renderMarkdown(''), '');
  assert.equal(md.renderMarkdown(null), '');
});
