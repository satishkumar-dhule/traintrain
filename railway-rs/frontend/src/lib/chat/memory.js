/* memory.js - session Q&A memory for the assistant.
   Two jobs, both zero-network:
     1. Replay: an asked-and-answered question (exact or similar enough)
        is served from the local cache instead of hitting /ai/chat again.
     2. Auto-compaction: when the transcript grows, old turns are folded
        into a compact digest message so LLM posts stay small and under
        the server's 40-message cap. Recent turns stay verbatim.
   Pure and Node-testable. */

export function createMemory(max = 30) {
  return { entries: [], max }
}

function normalize(q) {
  return String(q ?? '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s]/gu, ' ')
    .replace(/\b(live|current|now|today|please|tell|me|the|a|an|is|are|of|for)\b/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
}

function tokens(q) {
  return new Set(normalize(q).split(' ').filter(Boolean))
}

function jaccard(aSet, bSet) {
  if (!aSet.size || !bSet.size) return 0
  let inter = 0
  for (const t of aSet) if (bSet.has(t)) inter++
  return inter / (aSet.size + bSet.size - inter)
}

/** Entities that MUST match exactly for a non-exact replay: train numbers
 * (4-5 digits) and station-ish codes (2-5 uppercase alnum). */
function entities(q) {
  const nums = (q.match(/\b[1-9]\d{3,4}\b/g) ?? []).sort().join(',')
  const codes = (q.match(/\b[A-Z][A-Z0-9]{1,4}\b/g) ?? []).sort().join(',')
  return `${nums}|${codes}`
}

export function remember(memory, question, answer, opts = {}) {
  if (!memory || !question || !answer) return
  memory.entries.unshift({
    q: String(question),
    nq: normalize(question),
    ent: entities(question),
    answer,
    ts: Date.now(),
    ttlMs: opts.ttlMs ?? Infinity
  })
  if (memory.entries.length > memory.max) memory.entries.length = memory.max
}

/** Exact normalized match first; otherwise token similarity with mandatory
 * entity equality. Very short queries only replay on exact hits to avoid
 * false positives ("ok", "status"). */
function fresh(e, now = Date.now()) {
  return e.ts + (e.ttlMs ?? Infinity) > now
}

export function findReplay(memory, question) {
  if (!memory?.entries?.length) return null
  const nq = normalize(question)
  const exact = memory.entries.find((e) => e.nq === nq && e.answer && fresh(e))
  if (exact) return { entry: exact, exact: true }

  const q = String(question ?? '')
  if (nq.split(' ').length < 4) return null

  const qt = tokens(question)
  const ent = entities(q)
  let best = null
  let bestScore = 0
  for (const e of memory.entries) {
    if (!e.answer || !fresh(e) || e.ent !== ent) continue
    const et = new Set(e.nq.split(' '))
    let inter = 0
    for (const t of qt) if (et.has(t)) inter++
    if (inter < 3) continue
    const score = jaccard(qt, et)
    if (score > bestScore) {
      best = e
      bestScore = score
    }
  }
  return bestScore >= 0.6 ? { entry: best, exact: false } : null
}

// ---------- auto-compaction ----------

const MAX_SERVER_MESSAGES = 40
const DIGEST_LINE_CAP = 200

function clip(s, n) {
  s = String(s ?? '').replace(/\s+/g, ' ').trim()
  return s.length > n ? `${s.slice(0, n - 1)}…` : s
}

/** Fold all but the last `keep` turns into a single digest user-message so
 * the POSTed history stays small (and always <= 40 messages). Returns
 * {messages, compacted} where messages[] is ready for /ai/chat. */
export function compact(turns, opts = {}) {
  const keep = Math.max(2, opts.keep ?? 8)
  const list = (turns ?? []).filter((t) => t && t.role !== 'system' && String(t.content ?? '').trim())
  if (list.length === 0) return { messages: [], compacted: false }

  const cut = Math.max(0, list.length - keep)
  if (cut === 0) {
    return {
      messages: list.map((t) => ({ role: t.role, content: t.content })),
      compacted: false
    }
  }

  // Never leave a dangling assistant turn at the head of the kept window.
  let start = cut
  while (start < list.length && list[start].role === 'assistant') start++

  const old = list.slice(0, cut)
  const lines = old.map((t) => `- ${t.role === 'user' ? 'asked' : 'answered'}: ${clip(t.content, DIGEST_LINE_CAP)}`)
  const digest =
    `[Earlier conversation summary — details may be trimmed. Continue naturally from it.]\n` +
    lines.join('\n')

  const kept = list.slice(start).map((t) => ({ role: t.role, content: t.content }))
  const messages = [{ role: 'user', content: digest }, ...kept]

  // Server caps: 40 messages / 32k chars each. Drop oldest digest+kept pairs
  // until it fits (kept turns win; digest survives unless alone too big).
  while (messages.length > MAX_SERVER_MESSAGES) {
    if (messages.length > 2) messages.splice(1, 1)
    else break
  }
  return { messages, compacted: true }
}
