/* embed.js - OPTIONAL semantic fallback tier for the chat intent router.
 *
 * Flow: gate.classify() always runs first (lexical, offline, free). Only when
 * it answers { kind: 'help' } - i.e. NO lexical match fired - may the UI call
 * embedClassify(text). We embed the query IN-BROWSER, cosine-match it against
 * precomputed intent-phrase vectors, and a margin-gated hit converts into a
 * real plan via buildPlanFor(). The CALLER (Assistant.svelte) decides what to
 * do with the returned plan; plans built here carry unresolved slots
 * ({ train: matched-or-null, date: null }) and fall into the caller's
 * existing help/slot-gap flow when required slots are missing.
 *
 * Cost & availability contract:
 *   - transformers.js is loaded FROM THE CDN at runtime via a variable URL +
 *     `@vite-ignore`, so Vite never bundles or resolves the specifier:
 *     ZERO bytes added to the main bundle.
 *   - Model weights: mixedbread-ai/mxbai-embed-xsmall-v1 @ dtype q8 costs
 *     roughly 30MB on the FIRST visit (fetched from CDN, persisted by the
 *     browser Cache API); 0 bytes on subsequent visits while cached.
 *   - NOTHING downloads eagerly: no network happens until embedClassify()
 *     is actually called, and every load step is try/catch-wrapped. Offline,
 *     blocked CDN or an unsupported browser resolves the cached promise to
 *     null PERMANENTLY; every public async API then returns null. The app
 *     never breaks and we never console.error-spam (one info line, once).
 *
 * Why margin gating exists:
 *   Small sentence embedders are strongly anisotropic: UNRELATED sentence
 *   pairs routinely score > 0.7 absolute cosine similarity (mxbai-family
 *   models squeeze most of the score mass into a narrow high band), so any
 *   purely-absolute threshold misfires constantly. pickByMargin therefore
 *   requires BOTH a floor on the winner's absolute score AND a minimum gap
 *   over the runner-up intent; failing either returns "no match".
 *
 * Tiering contract: lexical ALWAYS wins when it fires. This tier only ever
 * sees queries that lexical matching already rejected ({kind:'help'}).
 *
 * Lazy gate guard (deliberate design):
 *   gate.js symbols (INTENTS, buildPlanFor) are consumed through a module
 *   namespace obtained from a dynamic import evaluated LAZILY INSIDE the
 *   functions that need it:
 *       let _gate;
 *       try { _gate = await import('./gate.js'); } catch { _gate = null; }
 *   (negative-cached: one attempt, permanent null on failure). Consequences:
 *     - Node unit tests import only the pure vector helpers below, so the
 *       gate import (and the CDN loader) NEVER execute under `node --test`.
 *     - If gate.js is absent or mid-refactor, embedClassify degrades to null
 *       instead of throwing; the bundler still code-splits gate.js normally
 *       because the specifier is a static string (no @vite-ignore here).
 */

// ---------------------------------------------------------------------------
// Pure vector core - no DOM, no network, no gate dependency. Directly
// unit-testable in Node; accepts any indexable numeric container
// (Array, Float32Array, ...).
// ---------------------------------------------------------------------------

/** Return a same-kind copy of `v` scaled to unit L2 norm. A zero vector is
 * returned unchanged (copied) instead of producing NaN. */
export function l2normalize(v) {
  let sum = 0;
  for (let i = 0; i < v.length; i++) sum += v[i] * v[i];
  const norm = Math.sqrt(sum);
  const out = v instanceof Float32Array ? new Float32Array(v.length) : new Array(v.length);
  if (norm === 0) {
    for (let i = 0; i < v.length; i++) out[i] = v[i];
    return out;
  }
  for (let i = 0; i < v.length; i++) out[i] = v[i] / norm;
  return out;
}

/** Cosine similarity; 0 when either operand has zero magnitude. */
export function cosine(a, b) {
  let dot = 0;
  let na = 0;
  let nb = 0;
  const len = Math.min(a.length, b.length);
  for (let i = 0; i < len; i++) {
    dot += a[i] * b[i];
    na += a[i] * a[i];
    nb += b[i] * b[i];
  }
  if (na === 0 || nb === 0) return 0;
  return dot / Math.sqrt(na * nb);
}

/**
 * Pick the best-matching item, gated on BOTH an absolute-score floor and a
 * minimum margin over the runner-up.
 * @param {ArrayLike<number>} queryVec
 * @param {{id: string, vec: ArrayLike<number>}[]} items
 * @param {{floor?: number, margin?: number}} [opts] defaults floor=0.55, margin=0.08
 * @returns {{id: string, score: number, gap: number} | null}
 *   gap = winner.score - runnerUp.score, or winner.score itself when there is
 *   only one candidate. Returns null when `items` is empty, when the best
 *   score is under `floor`, or when `gap` is under `margin` (ties die here).
 */
export function pickByMargin(queryVec, items, opts = {}) {
  const { floor = 0.55, margin = 0.08 } = opts;
  if (!Array.isArray(items) || items.length === 0) return null;
  const scored = items.map((item) => ({ id: item.id, score: cosine(queryVec, item.vec) }));
  scored.sort((a, b) => b.score - a.score);
  const best = scored[0];
  const gap = scored.length > 1 ? best.score - scored[1].score : best.score;
  if (best.score < floor) return null;
  if (gap < margin) return null;
  return { id: best.id, score: best.score, gap };
}

// ---------------------------------------------------------------------------
// Lazy model loader. Everything below touches network/DOM only when invoked.
// ---------------------------------------------------------------------------

let _extractorPromise = null;

/**
 * Cached promise for the feature-extraction pipeline. Any failure resolves
 * the cache to null PERMANENTLY (single info log, no retries, no spam).
 * @returns {Promise<Function|null>} extractor(text, opts) -> Tensor, or null.
 */
export function initEmbedder() {
  if (!_extractorPromise) {
    _extractorPromise = (async () => {
      try {
        const url = 'https://cdn.jsdelivr.net/npm/@huggingface/transformers@4.2.0';
        const mod = await import(/* @vite-ignore */ url);
        const pipeline = mod.pipeline ?? (mod.default && mod.default.pipeline);
        const extractor = await pipeline(
          'feature-extraction',
          'mixedbread-ai/mxbai-embed-xsmall-v1',
          { dtype: 'q8' },
        );
        if (typeof extractor !== 'function') throw new Error('pipeline returned non-callable');
        return extractor;
      } catch (reason) {
        console.info('[embed] unavailable:', reason && reason.message ? reason.message : String(reason));
        return null;
      }
    })();
  }
  return _extractorPromise;
}

/** Embed texts one-by-one (loop is deliberate: keeps peak memory tiny and the
 * call site simple). Returns one Float32Array per input, L2-normalized by the
 * pipeline's `normalize: true`. */
export async function embedTexts(extractor, texts) {
  const out = [];
  for (const text of texts) {
    const tensor = await extractor(text, { pooling: 'mean', normalize: true });
    const flat = typeof tensor.squeeze === 'function' ? tensor.squeeze() : tensor;
    out.push(flat.data instanceof Float32Array ? flat.data : new Float32Array(flat.data));
  }
  return out;
}

const _vectorCache = new WeakMap(); // keyed by extractor identity

/**
 * Build (once per session, cached by extractor identity) the intent vectors:
 * for each gate INTENT, embed all phrases, take the element-wise MEAN of the
 * phrase vectors, L2-normalize. Degrades to [] when gate.js is unavailable.
 */
export async function getIntentVectors(extractor) {
  const cached = _vectorCache.get(extractor);
  if (cached) return cached;
  const gate = await getGate();
  const intents = gate && Array.isArray(gate.INTENTS) ? gate.INTENTS : [];
  const vectors = [];
  for (const intent of intents) {
    if (!intent || !Array.isArray(intent.phrases) || intent.phrases.length === 0) continue;
    const phraseVecs = await embedTexts(extractor, intent.phrases);
    const dim = phraseVecs[0].length;
    const mean = new Float32Array(dim);
    for (const pv of phraseVecs) {
      for (let i = 0; i < dim; i++) mean[i] += pv[i];
    }
    for (let i = 0; i < dim; i++) mean[i] /= phraseVecs.length;
    vectors.push({ id: intent.id, vec: l2normalize(mean) });
  }
  _vectorCache.set(extractor, vectors);
  return vectors;
}

const TRAIN_RE = /\b([1-9]\d{4})\b/;

let _gate; // module namespace object | null (negative cache) | undefined (untouched)

async function getGate() {
  if (_gate !== undefined) return _gate;
  try {
    _gate = await import('./gate.js');
  } catch {
    _gate = null;
  }
  return _gate;
}

/**
 * Public entry point. Semantic fallback AFTER gate.classify() answered
 * { kind: 'help' }. Returns null whenever the tier cannot help: embedder
 * unavailable (offline/blocked/no support), gate.js unavailable, no intent
 * vectors, or no margin-gated hit.
 * @returns {Promise<null | {cardKind: string, plan: object, score: number, gap: number}>}
 */
export async function embedClassify(text) {
  try {
    const extractor = await initEmbedder();
    if (!extractor) return null;
    const vectors = await getIntentVectors(extractor);
    if (vectors.length === 0) return null;
    const [vec] = await embedTexts(extractor, [text]);
    const hit = pickByMargin(vec, vectors);
    if (!hit) return null;
    const train = (String(text).match(TRAIN_RE) || [])[1] ?? null;
    const gate = await getGate();
    if (!gate || typeof gate.buildPlanFor !== 'function') return null;
    const plan = gate.buildPlanFor(hit.id, { train, date: null }, {});
    return { cardKind: hit.id, plan, score: hit.score, gap: hit.gap };
  } catch (reason) {
    console.info('[embed] classify failed:', reason && reason.message ? reason.message : String(reason));
    return null;
  }
}
