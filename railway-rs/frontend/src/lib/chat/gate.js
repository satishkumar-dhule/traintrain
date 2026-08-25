/* gate.js - local-first intent router for the assistant.
   Classifies a user message BEFORE any network call. There is no LLM path
   anymore: every message resolves locally to one of
      trivial  -> canned reply, zero requests
      replay   -> session memory hit (exact or similar), zero requests
      tool     -> confident corpus hit + entities complete: deterministic
                  single-tool plan against plain REST endpoints
      confirm  -> heavy intent (seat availability) or ambiguous-band hit:
                  ask the user to confirm before executing
      help     -> no match / missing slot: capability summary or a targeted
                  "give me the missing piece" hint
   Matching pipeline: normalize (lowercase, punctuation strip, suffix strip,
   Hinglish glossary) -> entity/slot stripping -> MiniSearch BM25 top-5 ->
   tokenSetDice rerank -> ACCEPT/REJECT/MARGIN gates.
   Pure and Node-testable: no DOM, no fetch. The caller executes plans and
   feeds DTOs to the exported `project*` mappers, which mirror the server-side
   ai_chat projections 1:1 so ToolCards renders identical shapes. */

import MiniSearch from 'minisearch'
import { distance as levenshtein } from 'fastest-levenshtein'
import { findReplay } from './memory.js'

const TRAIN_RE = /\b([1-9]\d{4})\b/
const PNR_RE = /\b(\d{10})\b/
const DAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
const MAX_BETWEEN_TRAINS = 12
const MAX_UPCOMING_STOPS = 4
const MAX_SCHEDULE_STOPS = 12
const BOARD_MAX_TRAINS = 8
const MAX_AVAILABILITY_TRAINS = 8
const MAX_AVAILABILITY_CLASSES = 6

// ---------- normalization + Hinglish glossary ----------

/** Light suffix stripper: drop -es/-s only when the remaining stem keeps
 * >=4 chars. Guards keep 'status'/'always'-style words intact. */
function stripSuffix(w) {
  if (w.endsWith('es') && w.length - 2 >= 4) return w.slice(0, -2)
  if (
    w.endsWith('s') &&
    !w.endsWith('ss') &&
    !w.endsWith('us') &&
    w.length - 1 >= 4
  )
    return w.slice(0, -1)
  return w
}

/** Hinglish -> English glossary, applied word-wise AFTER suffix stripping.
 * Multi-word keys are replaced as phrases first, then single words. */
export const HINGLISH = {
  // multi-word phrases (checked longest-first before single words)
  'chal rahi': 'running',
  'chal rha': 'running',
  'mil jayegi': 'available',
  'mil jayega': 'available',
  'ban gaya': 'prepared',
  'ban gayi': 'prepared',
  // single words
  kahan: 'where',
  kaha: 'where',
  kab: 'when',
  hai: 'is',
  hain: 'is',
  kitna: 'how much',
  kitni: 'how much',
  kitne: 'how much',
  der: 'delay',
  late: 'late',
  chal: 'running',
  chalti: 'running',
  chalte: 'running',
  chali: 'running',
  rahi: '',
  raha: '',
  rahe: '',
  mil: 'available',
  milegi: 'available',
  milega: 'available',
  pe: 'at',
  gaadi: 'train',
  seat: 'seat',
  berth: 'berth',
  tiket: 'ticket',
  ticket: 'ticket',
  mera: 'my',
  meri: 'my',
  chart: 'chart',
  bana: 'prepared',
  bani: 'prepared',
  ban: 'prepared',
  platform: 'platform',
  nikal: 'depart',
  nikalnegi: 'depart',
  niklegi: 'depart',
  pahunch: 'reach',
  pahunchegi: 'reach',
  pahunchega: 'reach',
  kal: 'tomorrow',
  aaj: 'today',
  beech: 'between'
}

const HINGLISH_PHRASES = Object.entries(HINGLISH)
  .filter(([k, v]) => k.includes(' '))
  .sort((a, b) => b[0].length - a[0].length)

/** lowercase -> strip punctuation (keep hyphens for dates) -> collapse
 * spaces -> light suffix strip -> Hinglish glossary. */
export function normalize(q) {
  let s = String(q ?? '').toLowerCase()
  // Arrow / route separators become explicit " to " so "SC→PUNE" or "SC->PUNE" keeps signal
  s = s.replace(/[→⇒⟶➝➜➔➤]/g, ' to ')
  s = s.replace(/\s*->\s*/g, ' to ')
  s = s.replace(/\s*=>\s*/g, ' to ')
  s = s.replace(/[^a-z0-9\s-]/g, ' ').replace(/\s+/g, ' ').trim()
  if (!s) return ''
  s = s.split(' ').map(stripSuffix).join(' ')
  for (const [k, v] of HINGLISH_PHRASES) {
    s = s.replace(new RegExp(`\\b${k}\\b`, 'g'), v)
  }
  s = s
    .split(' ')
    .map((w) => (Object.prototype.hasOwnProperty.call(HINGLISH, w) ? HINGLISH[w] : w))
    .join(' ')
  return s.replace(/\s+/g, ' ').trim()
}

// ---------- entity + slot extraction ----------

function isoTomorrow() {
  return new Date(Date.now() + 86400000).toISOString().slice(0, 10)
}

/** {train: '12951'|null, pnr: '1234567890'|null, date: string|null}. Date is a server-compatible
 * string: 'today', ISO YYYY-MM-DD or DD-MM-YYYY. Relative words map:
 * aaj/today -> 'today', kal/tomorrow -> ISO of tomorrow (the server only
 * understands 'today' + absolute formats, so tomorrow is materialized). */
export function extractEntities(raw) {
  const s = String(raw ?? '')
  const train = (s.match(TRAIN_RE) || [])[1] ?? null
  const pnr = (s.match(PNR_RE) || [])[1] ?? null
  let date = null
  const iso = s.match(/\b(\d{4})-(\d{2})-(\d{2})\b/)
  const dmy = s.match(/\b(\d{2})-(\d{2})-(\d{4})\b/)
  const rel = s.match(/\b(today|tomorrow|kal|aaj)\b/i)
  if (iso) date = iso[0]
  else if (dmy) date = dmy[0]
  else if (rel) date = /^(today|aaj)$/i.test(rel[1]) ? 'today' : isoTomorrow()
  return { train, pnr, date }
}

const BETWEEN_RES = [
  /\bfrom\s+([a-z][a-z .'-]{1,29}?)\s+(?:to|till|upto|until|and)\s+([a-z][a-z .'-]{1,29}?)\s*(?:tak)?\s*$/i,
  /\bbetween\s+([a-z][a-z .'-]{1,29}?)\s+(?:to|and)\s+([a-z][a-z .'-]{1,29}?)\s*(?:tak)?\s*$/i,
  /\b([a-z][a-z .'-]{1,29}?)\s+se\s+([a-z][a-z .'-]{1,29}?)\s*(?:tak)?\s*$/i
]

const BOARD_RES = [
  /\b(?:board|arrivals?|departures?)\b(?:\s*(?:at|for|of))?\s+([a-z][a-z .'-]{1,30})\s*$/i,
  /^trains?\s+(?:at|from)\s+([a-z][a-z .'-]{1,30})\s*$/i
]

const STOPWORDS_FOR_SLOT = new Set([
  'check','show','find','get','tell','give','please','kindly','seat','availability','available','berth','ticket','reservation','booking','trains','train','between','from','to','and','for','of','at','the','a','an','is','are','am','was','were','be','been','being','do','does','did','done','i','me','my','mine','you','your','yours','we','our','ours','us','it','its','they','them','their','this','that','these','those','there','here','what','which','when','who','whom','how','much','many','get','got','give','show','has','have','had','now','yet','still','already','just','actually','kya','batao','nahin','nahi','haan','jaldi','station','stations','board','live','arrivals','departures','pnr','status','where','which','running','route','schedule','timetable','delay','chart','prepared','today','tomorrow','kal','aaj','next','now','upcoming','platform','halting','announcement','display','departures','arrival'
])

function isPlausibleStationToken(tok) {
  const t = String(tok ?? '').toLowerCase().trim()
  if (!t || t.length < 2) return false
  if (STOPWORDS.has(t)) return false
  if (STOPWORDS_FOR_SLOT.has(t)) return false
  if (/^\d+$/.test(t)) return false
  return /^[a-z][a-z0-9 .'-]*$/.test(t)
}

function cleanStationQuery(s) {
  if (!s) return s
  let out = String(s).trim()
  out = out.replace(/\s+(?:board|station|stations|please|tak|today|tomorrow|next|now)\s*$/i, '').trim()
  out = out.replace(/\s+tak$/i, '').trim()
  out = out.replace(/^(?:board|station)\s+/i, '').trim()
  return out
}

/** Fallback for bare "X to Y" without from/between — last " to " wins */
function fallbackBareTo(s, alreadySrc, alreadyDst) {
  if (alreadySrc && alreadyDst) return { srcQuery: alreadySrc, dstQuery: alreadyDst }
  // Find all occurrences of " to " with word boundaries
  const pattern = /\b to \b/g
  let lastIdx = -1
  let m
  while ((m = pattern.exec(s)) !== null) lastIdx = m.index
  if (lastIdx === -1) return { srcQuery: alreadySrc, dstQuery: alreadyDst }
  // extract left and right windows up to 40 chars or string boundaries, but prefer token boundaries
  const leftWindow = s.slice(Math.max(0, lastIdx - 40), lastIdx).trim()
  const rightWindow = s.slice(lastIdx + 4, lastIdx + 44).trim()
  if (!leftWindow || !rightWindow) return { srcQuery: alreadySrc, dstQuery: alreadyDst }

  // left: take last 1-3 plausible tokens
  const leftTokens = leftWindow.split(/\s+/).filter(Boolean)
  const rightTokens = rightWindow.split(/\s+/).filter(Boolean)
  // filter out filler from left tail: walk from end, collect plausible tokens up to 3 words
  const leftCollected = []
  for (let i = leftTokens.length - 1; i >= 0 && leftCollected.length < 3; i--) {
    const tok = leftTokens[i].replace(/[^a-z'-]/g, '')
    if (!tok) continue
    if (STOPWORDS_FOR_SLOT.has(tok)) {
      if (leftCollected.length === 0) continue // skip leading filler like "availability"
      else break // stop at filler that separates station from earlier intent words
    }
    if (!isPlausibleStationToken(tok)) continue
    leftCollected.unshift(tok)
  }
  // right: take first 1-3 plausible tokens
  const rightCollected = []
  for (let i = 0; i < rightTokens.length && rightCollected.length < 3; i++) {
    const tok = rightTokens[i].replace(/[^a-z'-]/g, '')
    if (!tok) continue
    if (STOPWORDS_FOR_SLOT.has(tok)) {
      if (rightCollected.length === 0) continue
      else break
    }
    if (!isPlausibleStationToken(tok)) break
    rightCollected.push(tok)
    // stop after we have 2 tokens, if next token would be filler like "trains"
    if (rightCollected.length >= 2) {
      // peek next token if it's "trains" -> stop
      const nxt = rightTokens[i + 1]
      if (nxt && STOPWORDS_FOR_SLOT.has(nxt)) break
    }
  }

  const src = leftCollected.join(' ').trim()
  const dst = rightCollected.join(' ').trim()
  // Validate lengths and not generic
  const isGeneric = (t) => /^(these|those|this|that|station|stations)$/i.test(String(t ?? '').trim())
  let outSrc = alreadySrc
  let outDst = alreadyDst
  if (!outSrc && src && src.length >= 2 && !isGeneric(src) && dst && dst.length >= 2 && !isGeneric(dst)) {
    outSrc = src
    outDst = dst
  } else if (!outSrc && src && src.length >= 2 && !isGeneric(src) && !outDst) {
    // single side? handled by enrich later
  }
  return { srcQuery: outSrc, dstQuery: outDst }
}

function fallbackBoard(s, already) {
  if (already) return already
  // Try "board <station>" already handled, but also "station <name> board" or isolated station after keywords
  // Look for last mention of plausible station near board keywords
  if (!/\b(board|arrivals?|departures?|live station|platform)\b/i.test(s)) return null
  // Try pattern like "station <name>"
  let m = s.match(/\bstation\s+([a-z][a-z .'-]{1,30})\b/i)
  if (m) {
    let cand = cleanStationQuery(m[1])
    if (cand && cand.length >= 2 && !/^(board|announcement|display)$/i.test(cand) && isPlausibleStationToken(cand.split(/\s+/)[0])) return cand
  }
  // Try last plausible token after "at/for/of" near board
  // Find all plausible tokens in string
  const tokens = s.split(/\s+/).map((t) => t.replace(/[^a-z]/g, '')).filter((t) => t && t.length >= 2 && /^[a-z]{2,10}$/.test(t) && isPlausibleStationToken(t) && !['board','station','live','at','for','of','in','arrivals','departures','announcement','display','next','upcoming','platform','trains','train'].includes(t))
  if (tokens.length) {
    // Prefer last token that is not at the very start if multiple
    return tokens[tokens.length - 1]
  }
  return null
}

/** Free-text station references: {srcQuery, dstQuery, stationQuery}. Run on
 * text that has already had entity literals stripped (see entityLiterals),
 * otherwise relative dates glom onto the destination span. */
export function extractSlots(mappedText) {
  const s = String(mappedText ?? '').toLowerCase()
  const out = { srcQuery: null, dstQuery: null, stationQuery: null }
  if (!s) return out
  for (const re of BETWEEN_RES) {
    const m = s.match(re)
    if (m) {
      out.srcQuery = cleanStationQuery(m[1].trim())
      out.dstQuery = cleanStationQuery(m[2].trim().replace(/\s+tak$/i, '').trim())
      break
    }
  }
  // Fallback bare "X to Y" if strict patterns missed
  if (!out.srcQuery || !out.dstQuery) {
    const fb = fallbackBareTo(s, out.srcQuery, out.dstQuery)
    if (fb.srcQuery) out.srcQuery = fb.srcQuery
    if (fb.dstQuery) out.dstQuery = fb.dstQuery
  }
  for (const re of BOARD_RES) {
    const m = s.match(re)
    if (m) {
      out.stationQuery = cleanStationQuery(m[1].trim())
      break
    }
  }
  if (!out.stationQuery) {
    const fb = fallbackBoard(s, null)
    if (fb) out.stationQuery = cleanStationQuery(fb)
  }
  // Final clean
  if (out.srcQuery) out.srcQuery = cleanStationQuery(out.srcQuery)
  if (out.dstQuery) out.dstQuery = cleanStationQuery(out.dstQuery)
  if (out.stationQuery) out.stationQuery = cleanStationQuery(out.stationQuery)
  // Guard generic words
  const isGeneric = (t) => /^(these|those|this|that|station|stations|these station|these stations|this station)$/i.test(String(t ?? '').trim())
  if (out.srcQuery && isGeneric(out.srcQuery)) out.srcQuery = null
  if (out.dstQuery && isGeneric(out.dstQuery)) out.dstQuery = null
  if (out.stationQuery && isGeneric(out.stationQuery)) out.stationQuery = null
  return out
}

// ---------- intent corpus ----------

/** Canonical phrases must be digit-free: entities are extracted separately
 * and excluded from similarity text. Hinglish inputs normalize onto these
 * English forms (token-set scoring is order-free). */
export const INTENTS = [
  {
    id: 'live_status',
    needsTrain: true,
    phrases: [
      'live status of train',
      'live running status',
      'train running status',
      'current status of my train',
      'where is my train',
      'where is the train',
      'running position of train',
      'is my train delayed',
      'is the train running late',
      'how late is the train',
      'track my train',
      'train location now',
      'has the train reached yet',
      'when will train reach',
      'train live position'
    ]
  },
  {
    id: 'average_delay',
    needsTrain: true,
    phrases: [
      'average delay of train',
      'avg delay history',
      'usual delay of train',
      'how much delay usually',
      'typical delay pattern of train',
      'average lateness of train',
      'delay statistics of train',
      'punctuality record of train',
      'how much late does train usually run',
      'historical delays by station',
      'mean delay minutes of train',
      'average running late pattern'
    ]
  },
  {
    id: 'train_schedule',
    needsTrain: true,
    phrases: [
      'route of train',
      'schedule of train',
      'timetable of train',
      'time table of train',
      'full route with all stops',
      'complete schedule of train',
      'list of stations on route',
      'which stations does train stop at',
      'halts of train',
      'journey timings station wise',
      'route map of train',
      'total stops and distance of train',
      'train route details'
    ]
  },
  {
    id: 'trains_between',
    needsSrcDst: true,
    phrases: [
      'trains between two stations',
      'trains from one station to another',
      'list of trains between stations',
      'direct trains between stations',
      'which trains run between these stations',
      'train options between two stations',
      'find trains connecting two stations',
      'all trains from source to destination',
      'services between stations',
      'weekly trains between stations',
      'show trains for this route',
      'trains available on this route'
    ]
  },
  {
    id: 'station_board',
    needsStation: true,
    phrases: [
      'station board',
      'live arrivals at station',
      'departures at station',
      'arrivals board for station',
      'which trains arrive at station now',
      'next trains at station',
      'live station display',
      'upcoming trains at station',
      'station announcement board',
      'trains halting at station now',
      'platform wise arrivals at station'
    ]
  },
  {
    id: 'seat_availability',
    needsSrcDst: true,
    heavy: true,
    phrases: [
      'seat availability',
      'check seat availability',
      'are seats available',
      'seat available in train',
      'berth availability',
      'ticket availability',
      'reservation availability',
      'waiting list chances',
      'can i get a confirmed seat',
      'seats left in train',
      'booking status of class',
      'general quota seat status',
      'sleeper class seats available',
      'tatkal availability',
      'will i get confirmation',
      'current reservation status'
    ]
  },
  {
    id: 'chart_status',
    needsTrain: true,
    phrases: [
      'chart status',
      'has the chart been prepared',
      'is chart prepared or not',
      'reservation chart status',
      'is the chart ready',
      'check chart preparation',
      'final chart released',
      'chart preparation time',
      'coach count in chart',
      'boarding station on chart',
      'chart made or not',
      'when will chart prepare'
    ]
  },
  {
    id: 'pnr_status',
    needsPnr: true,
    phrases: [
      'pnr status',
      'check my pnr',
      'pnr enquiry',
      'booking status of pnr',
      'is my ticket confirmed pnr',
      'current reservation status of pnr',
      'pnr number status',
      'has my pnr been confirmed',
      'pnr current status',
      'check pnr number',
      'pnr berth status',
      'my pnr status'
    ]
  }
]

export const REQUIRED_FIELDS = {
  live_status: ['train'],
  average_delay: ['train'],
  train_schedule: ['train'],
  trains_between: ['src', 'dst'],
  station_board: ['station'],
  seat_availability: ['src', 'dst'],
  chart_status: ['train'],
  pnr_status: ['pnr']
}

export const INTENT_LABELS = {
  live_status: 'Live running status',
  average_delay: 'Average delay',
  train_schedule: 'Train schedule',
  trains_between: 'Trains between stations',
  station_board: 'Station board',
  seat_availability: 'Seat availability',
  chart_status: 'Chart status',
  pnr_status: 'PNR status'
}

const PHRASES = INTENTS.flatMap((intent) =>
  intent.phrases.map((raw) => ({ intent, raw, text: normalize(raw) }))
)

function todayISO() {
  return new Date().toISOString().slice(0, 10)
}

// Fallback slot extraction for partial "from X" without "to Y" – used only on help path to prefill collected.
function enrichSlotsForHelp(mapped, slots) {
  const out = { ...slots }
  const isGeneric = (s) => {
    const t = String(s ?? '').toLowerCase().trim()
    return /^(these|those|this|that|station|stations|these station|these stations|this station)$/i.test(t)
  }
  if (!out.srcQuery) {
    let m =
      mapped.match(/\bfrom\s+([a-z][a-z .'-]{1,29}?)(?=\s+to\b|\s+tak\b|\s*$)/i) ||
      mapped.match(/\bfrom\s+([a-z][a-z .'-]{1,29}?)\s*$/i)
    if (m && !isGeneric(m[1])) out.srcQuery = cleanStationQuery(m[1].trim().toLowerCase())
    else {
      const simple = mapped.match(/\bfrom\s+([a-z]{2,30})\b/i)
      if (simple && !isGeneric(simple[1])) out.srcQuery = simple[1].trim().toLowerCase()
    }
  }
  if (out.srcQuery && !out.dstQuery) {
    const mTo =
      mapped.match(/\bto\s+([a-z][a-z .'-]{1,29}?)\s*$/i) ||
      mapped.match(/\bto\s+([a-z][a-z .'-]{1,29}?)(?:\s+tak)?\s*$/i)
    if (mTo && !isGeneric(mTo[1])) out.dstQuery = cleanStationQuery(mTo[1].trim().toLowerCase())
  }
  if (!out.stationQuery) {
    const mBoard = mapped.match(/\bboard\s+([a-z][a-z .'-]{1,30})\s*$/i)
    if (mBoard && !isGeneric(mBoard[1])) out.stationQuery = cleanStationQuery(mBoard[1].trim().toLowerCase())
    else {
      // try generic board fallback
      const fb = fallbackBoard(mapped, null)
      if (fb && !isGeneric(fb)) out.stationQuery = fb
    }
  }
  return out
}

function buildCandidates(mapped) {
  const base = String(mapped ?? '')
  if (!base) {
    return INTENTS.slice(0, 3).map((intent) => ({
      intentId: intent.id,
      label: INTENT_LABELS[intent.id] ?? intent.id,
      score: 0
    }))
  }
  const body0 = stripEntities(base)
  const slots = extractSlots(body0)
  const enrichedSlots = enrichSlotsForHelp(base, slots)
  // slot-aware boost: detect seat words early
  const hasSeatWords = /\b(seat|berth|availabilit|tiket|ticket|reservation|waitlist|booking|tatkal)\b/.test(base)
  const hasPnrWords = /\b(pnr)\b/.test(base)
  const hasRouteWords = /\b(route|schedule|timetable|time table|halts?|stops? at)\b/.test(base)
  const hasChartWords = /\b(chart|prepared|ready)\b/.test(base)
  const hasDelayWords = /\b(average|avg)\s+(delay|late)\b|\bhow much delay\b/.test(base)
  const hasBoardWords = /\b(board|arrivals?|departures?|live station)\b/.test(base)

  const body = stripStrings(body0, [slots.srcQuery, slots.dstQuery, slots.stationQuery])
  let qTokens = contentTokens(body)
  if (!qTokens.length) qTokens = contentTokens(base)
  if (!qTokens.length) {
    return INTENTS.slice(0, 3).map((intent) => ({
      intentId: intent.id,
      label: INTENT_LABELS[intent.id] ?? intent.id,
      score: 0
    }))
  }
  const q = qTokens.join(' ')
  // Try MiniSearch hits first; fall back to brute-force scoring when no hits
  let scored
  const hits = getIndex().search(q, { fuzzy: 0.2, prefix: false, combineWith: 'OR' }).slice(0, 5)
  if (hits.length) {
    scored = hits.map((h) => {
      const p = PHRASES[Number(h.id)]
      return { p, score: tokenSetDice(qTokens, contentTokens(p.text)) }
    })
    // If distinct intent count <3, supplement with brute-force for remaining intents
    const seenIds = new Set(scored.map((s) => s.p.intent.id))
    if (seenIds.size < 3) {
      const extra = PHRASES.map((p) => ({ p, score: tokenSetDice(qTokens, contentTokens(p.text)) }))
        .filter((e) => !seenIds.has(e.p.intent.id))
        .sort((a, b) => b.score - a.score)
      // add best per extra intent until we have enough
      const added = new Set()
      for (const e of extra) {
        const id = e.p.intent.id
        if (added.has(id) || seenIds.has(id)) continue
        added.add(id)
        scored.push(e)
        seenIds.add(id)
        if (seenIds.size >= INTENTS.length) break
      }
    }
    scored.sort((a, b) => b.score - a.score)
  } else {
    scored = PHRASES.map((p) => ({ p, score: tokenSetDice(qTokens, contentTokens(p.text)) })).sort(
      (a, b) => b.score - a.score
    )
  }

  // ---- slot-aware boost before distinct selection ----
  // Apply additive boost to intents that have required slots present in the query.
  // This fixes cases where token stripping leaves single "train" token causing wrong winner.
  const boostMap = new Map()
  if (enrichedSlots.srcQuery && enrichedSlots.dstQuery) {
    if (hasSeatWords) {
      boostMap.set('seat_availability', 0.25)
      // also keep trains_between as runner-up but slightly less
      if (!boostMap.has('trains_between')) boostMap.set('trains_between', 0.05)
    } else {
      boostMap.set('trains_between', 0.3)
      // seat without seat words shouldn't outrank trains_between
    }
  } else if (enrichedSlots.stationQuery || hasBoardWords) {
    boostMap.set('station_board', 0.28)
  } else if (hasPnrWords) {
    boostMap.set('pnr_status', 0.35)
  }

  // Also boost based on explicit verb cues even without slots
  if (hasDelayWords) boostMap.set('average_delay', (boostMap.get('average_delay') || 0) + 0.28)
  if (hasRouteWords) boostMap.set('train_schedule', (boostMap.get('train_schedule') || 0) + 0.28)
  if (hasChartWords) boostMap.set('chart_status', (boostMap.get('chart_status') || 0) + 0.25)
  if (hasPnrWords) boostMap.set('pnr_status', (boostMap.get('pnr_status') || 0) + 0.2)

  if (boostMap.size) {
    for (const entry of scored) {
      const b = boostMap.get(entry.p.intent.id)
      if (b) entry.score = Math.min(1, entry.score + b)
    }
    scored.sort((a, b) => b.score - a.score)
  }

  const seen = new Set()
  const distinct = []
  for (const c of scored) {
    const id = c.p.intent.id
    if (seen.has(id)) continue
    seen.add(id)
    distinct.push({ intentId: id, label: INTENT_LABELS[id] ?? id, score: round3(c.score) })
    if (distinct.length >= 3) break
  }
  // pad if still <3 (should not happen but guard)
  if (distinct.length < 3) {
    for (const intent of INTENTS) {
      if (distinct.find((d) => d.intentId === intent.id)) continue
      distinct.push({ intentId: intent.id, label: INTENT_LABELS[intent.id] ?? intent.id, score: 0 })
      if (distinct.length >= 3) break
    }
  }
  return distinct
}

export function getIntentCandidates(text) {
  const mapped = normalize(String(text ?? ''))
  return buildCandidates(mapped)
}

function collectForForm(rawText, mapped) {
  const ent = extractEntities(rawText)
  const body0 = stripEntities(mapped)
  let slots = extractSlots(body0)
  slots = enrichSlotsForHelp(mapped, slots)
  const collected = {}
  if (ent.train) collected.train = ent.train
  if (ent.pnr) collected.pnr = ent.pnr
  if (ent.date) collected.date = ent.date
  if (slots.srcQuery) collected.src = slots.srcQuery
  if (slots.dstQuery) collected.dst = slots.dstQuery
  if (slots.stationQuery) collected.station = slots.stationQuery
  return collected
}

function makeForm({ intentId, confidence, collected, candidates }) {
  const required = intentId ? REQUIRED_FIELDS[intentId] || [] : []
  const missing = required.filter((f) => !collected[f] || String(collected[f]).trim() === '')
  const fields = []
  if (confidence < 0.45 || intentId == null) {
    fields.push({
      name: 'intent',
      label: 'What do you want to check?',
      type: 'select',
      required: true,
      value: intentId || '',
      options: INTENTS.map((i) => ({ value: i.id, label: INTENT_LABELS[i.id] }))
    })
  }
  for (const name of required) {
    let spec = null
    if (name === 'train') {
      spec = {
        name,
        label: 'Train number',
        placeholder: 'e.g. 12951',
        value: collected.train || '',
        required: true,
        type: 'text',
        pattern: '\\d{5}'
      }
    } else if (name === 'pnr') {
      spec = {
        name,
        label: 'PNR number',
        placeholder: 'e.g. 1234567890',
        value: collected.pnr || '',
        required: true,
        type: 'text',
        pattern: '\\d{10}'
      }
    } else if (name === 'src') {
      spec = {
        name,
        label: 'From station',
        placeholder: 'e.g. NDLS or New Delhi',
        value: collected.src || '',
        required: true,
        type: 'text'
      }
    } else if (name === 'dst') {
      spec = {
        name,
        label: 'To station',
        placeholder: 'e.g. PUNE or Pune',
        value: collected.dst || '',
        required: true,
        type: 'text'
      }
    } else if (name === 'station') {
      spec = {
        name,
        label: 'Station',
        placeholder: 'e.g. PUNE or Pune',
        value: collected.station || '',
        required: true,
        type: 'text'
      }
    }
    if (spec) fields.push(spec)
  }
  if (intentId) {
    let dateVal = collected.date || ''
    if (String(dateVal).toLowerCase() === 'today') dateVal = todayISO()
    else {
      const dmy = String(dateVal).match(/^(\d{2})-(\d{2})-(\d{4})$/)
      if (dmy) dateVal = `${dmy[3]}-${dmy[2]}-${dmy[1]}`
    }
    if (!dateVal) dateVal = todayISO()
    // pnr & seat_availability respect date differently; pnr doesn't use date, but keep optional date for chart/trains
    if (intentId === 'pnr_status') {
      // pnr has no date field
    } else {
      fields.push({
        name: 'date',
        label: 'Date',
        type: 'date',
        value: dateVal,
        required: false,
        placeholder: ''
      })
    }
  }
  const form = {
    intentId,
    intentLabel: intentId ? INTENT_LABELS[intentId] || '' : '',
    confidence,
    collected,
    missing,
    fields
  }
  if (candidates && candidates.length) form.candidates = candidates
  return form
}

export function buildFormSpec(text) {
  const raw = String(text ?? '')
  const mapped = normalize(raw)
  const collected = collectForForm(raw, mapped)
  const candidates = buildCandidates(mapped)
  const best = candidates[0] || null
  // Slot-aware override: if form has clear src/dst but candidate is not trains_between/seat_availability, prefer slot-consistent intent
  let intentId = best && best.score >= 0.3 ? best.intentId : null
  let confidence = best ? best.score : 0
  // Heuristic override when slots strongly indicate between but fuzzy picked wrong
  if (collected.src && collected.dst) {
    const hasSeatWords = /\b(seat|berth|availabilit|tiket|ticket|reservation|waitlist|booking|tatkal)\b/i.test(mapped)
    const preferred = hasSeatWords ? 'seat_availability' : 'trains_between'
    if (intentId !== preferred) {
      const prefCandidate = candidates.find((c) => c.intentId === preferred)
      if (prefCandidate && prefCandidate.score >= 0.18) {
        intentId = preferred
        confidence = prefCandidate.score
      } else if (!intentId) {
        intentId = preferred
        confidence = 0.35
      }
    }
  } else if (collected.pnr) {
    intentId = 'pnr_status'
    confidence = Math.max(confidence, 0.6)
  } else if (collected.station && !collected.train && !collected.src) {
    // likely station board if only station collected
    const hasBoardHint = /\b(board|arrivals?|departures?|live station)\b/i.test(mapped)
    if (hasBoardHint && intentId !== 'station_board') {
      const boardCand = candidates.find((c) => c.intentId === 'station_board')
      if (boardCand && boardCand.score >= 0.18) {
        intentId = 'station_board'
        confidence = boardCand.score
      }
    }
  }
  // If pnr present, force pnr_status regardless of fuzzy
  if (collected.pnr && intentId !== 'pnr_status') {
    const p = candidates.find((c) => c.intentId === 'pnr_status')
    if (p) {
      intentId = 'pnr_status'
      confidence = Math.max(confidence, p.score, 0.55)
    } else {
      intentId = 'pnr_status'
      confidence = 0.6
    }
  }
  return makeForm({ intentId, confidence, collected, candidates })
}

// ---------- plan builder (stable export; embed.js reuses it) ----------

/** Same plan shape as the classic regex path: {cardKind, url, params, resolve?}. */
export function buildPlanFor(id, entities = {}, slots = {}) {
  switch (id) {
    case 'live_status':
      return {
        cardKind: id,
        url: '/rail-api/live-status',
        params: entities.train ? { train: entities.train } : {}
      }
    case 'average_delay':
      return {
        cardKind: id,
        url: '/rail-api/ntes/average-delay',
        params: entities.train ? { train: entities.train } : {}
      }
    case 'train_schedule':
      return {
        cardKind: id,
        url: '/rail-api/schedule',
        params: entities.train ? { train: entities.train } : {}
      }
    case 'trains_between':
      return {
        cardKind: id,
        url: '/rail-api/ntes/trains-between',
        params: { src: '$src', dst: '$dst' },
        resolve: [
          { slot: 'src', query: slots.srcQuery ?? slots.src ?? '' },
          { slot: 'dst', query: slots.dstQuery ?? slots.dst ?? '' }
        ]
      }
    case 'station_board':
      return {
        cardKind: id,
        url: '/rail-api/ntes/live-station',
        params: { station: '$station' },
        resolve: [{ slot: 'station', query: slots.stationQuery ?? slots.station ?? '', preferCode: true }]
      }
    case 'seat_availability': {
      const params = { src: '$src', dst: '$dst' }
      if (entities.date) params.date = entities.date
      return {
        cardKind: id,
        url: '/rail-api/availability',
        params,
        resolve: [
          { slot: 'src', query: slots.srcQuery ?? slots.src ?? '' },
          { slot: 'dst', query: slots.dstQuery ?? slots.dst ?? '' }
        ]
      }
    }
    case 'chart_status': {
      const params = entities.train ? { train: entities.train } : {}
      if (entities.date) params.date = entities.date
      return { cardKind: id, url: '/rail-api/irctc/chart', params }
    }
    case 'pnr_status': {
      return {
        cardKind: id,
        url: '/rail-api/pnr',
        params: entities.pnr ? { pnr: entities.pnr } : {}
      }
    }
    default:
      throw new Error(`unknown intent: ${id}`)
  }
}

// ---------- fuzzy matching pipeline ----------

const ACCEPT = 0.62
const REJECT = 0.45
const MARGIN = 0.08
const MIN_QUERY_TOKENS = 2

// Content-only tokens: filler words carry no intent signal and would dilute
// the geometric normalization on both sides of the comparison.
const STOPWORDS = new Set(
  ('a an the is are am was were be been being do does did done i me my we our you your it its ' +
    'this that these those of in on at for to from and or nor please kindly can could would shall ' +
    'should will tell know want need any some there here what which when who whom how much many ' +
    'get got give show has have had now yet still already just actually ' +
    'kya batao nahin nahi haan jaldi')
    .split(' ')
)

const contentTokens = (text) =>
  String(text ?? '')
    .split(' ')
    .filter((t) => t && !STOPWORDS.has(t))

let _index = null
function getIndex() {
  if (!_index) {
    _index = new MiniSearch({
      fields: ['text'],
      storeFields: ['id'],
      searchOptions: { fuzzy: 0.2, prefix: false, combineWith: 'OR' }
    })
    PHRASES.forEach((p, i) => _index.add({ id: String(i), text: p.text }))
  }
  return _index
}

function stripStrings(text, strings) {
  let body = String(text ?? '')
  for (const s of strings) {
    if (s) body = body.split(String(s).toLowerCase()).join(' ')
  }
  return body.replace(/\s+/g, ' ').trim()
}

/** Tolerant token equality: equal, or within edit distance
 * max(1, floor(minLen/5)). */
function tokenMatch(a, b) {
  if (a === b) return true
  const minLen = Math.min(a.length, b.length)
  return levenshtein(a, b) <= Math.max(1, Math.floor(minLen / 5))
}

/**
 * Normalized overlap between two content-token sequences.
 *
 *   matched(Q, P) = |greedy 1:1 pairing of query tokens to phrase tokens
 *                    where tokenMatch(q, p)|   (order-free, each p used once)
 *   score         = matched / sqrt(|Q| * |P|)
 *
 * A cosine-style geometric-mean normalization in (0, 1]: 1 when every query
 * token pairs up and the token counts match exactly, decaying as either side
 * carries unmatched tokens. Unlike plain Dice it penalizes length imbalance,
 * which keeps short garbage queries away from long corpus phrases.
 */
export function tokenSetDice(queryTokens, phraseTokens) {
  const Q = queryTokens.filter(Boolean)
  const P = phraseTokens.filter(Boolean)
  if (!Q.length || !P.length) return 0
  const used = new Array(P.length).fill(false)
  let matched = 0
  for (const q of Q) {
    const hit = P.findIndex((p, i) => !used[i] && tokenMatch(q, p))
    if (hit === -1) continue
    used[hit] = true
    matched++
  }
  return matched / Math.sqrt(Q.length * P.length)
}

const round3 = (x) => Math.round(x * 1000) / 1000

/** Literal substrings to remove before scoring/station-slot extraction:
 * the train number, the relative/absolute date words as written (NOT the
 * normalized ISO value extractEntities returns), and slot spans later. */
function entityLiterals(text) {
  const s = String(text ?? '')
  return [
    (s.match(TRAIN_RE) || [])[1] ?? null,
    (s.match(PNR_RE) || [])[1] ?? null,
    s.match(/\b(today|tomorrow|kal|aaj)\b/i)?.[0] ?? null,
    s.match(/\b(\d{2})-(\d{2})-(\d{4})\b/)?.[0] ?? null,
    s.match(/\b(\d{4})-(\d{2})-(\d{2})\b/)?.[0] ?? null
  ]
}

/** Text with entity literals stripped — the right input for station-slot
 * extraction and for the similarity body. */
export function stripEntities(text) {
  return stripStrings(String(text ?? ''), entityLiterals(text))
}

/** Corpus-driven match over an ALREADY-NORMALIZED string. Entity strings
 * (train number, date words) and captured slot spans (station names) are
 * stripped before scoring. Returns null below REJECT, when fewer than
 * MIN_QUERY_TOKENS content tokens were present to begin with, or when
 * stripping leaves nothing meaningful. */
export function matchIntent(text) {
  const base = String(text ?? '')
  if (!base) return null
  // Too-short guard runs on the PRE-strip tokens: 'status' alone stays help,
  // while entity-heavy queries ('availability from sc to pune tomorrow')
  // keep enough signal after stripping to be scored.
  const preTokens = contentTokens(base)
  if (preTokens.length < MIN_QUERY_TOKENS) return null

  const body0 = stripEntities(base)
  const slots = extractSlots(body0)
  const body = stripStrings(body0, [
    slots.srcQuery,
    slots.dstQuery,
    slots.stationQuery
  ])
  const qTokens = contentTokens(body)
  if (!qTokens.length) return null

  const q = qTokens.join(' ')
  const hits = getIndex()
    .search(q, { fuzzy: 0.2, prefix: false, combineWith: 'OR' })
    .slice(0, 5)
  if (!hits.length) return null

  const scored = hits
    .map((h) => {
      const p = PHRASES[Number(h.id)]
      return { p, score: tokenSetDice(qTokens, contentTokens(p.text)) }
    })
    .sort((a, b) => b.score - a.score)

  // slot-aware boost for matchIntent as well (mirror buildCandidates)
  const hasSeatWords = /\b(seat|berth|availabilit|tiket|ticket|reservation|waitlist|booking|tatkal)\b/.test(base)
  const hasPnrWords = /\b(pnr)\b/.test(base)
  const hasDelayWords = /\b(average|avg)\s+(delay|late)\b|\bhow much delay\b/.test(base)
  const hasRouteWords = /\b(route|schedule|timetable|time table|halts?|stops? at)\b/.test(base)
  const hasChartWords = /\b(chart|prepared|ready)\b/.test(base)
  const hasBoardWords = /\b(board|arrivals?|departures?|live station)\b/.test(base)
  const boostMap = new Map()
  if (slots.srcQuery && slots.dstQuery) {
    if (hasSeatWords) boostMap.set('seat_availability', 0.25)
    else boostMap.set('trains_between', 0.3)
  } else if (slots.stationQuery || hasBoardWords) {
    boostMap.set('station_board', 0.28)
  } else if (hasPnrWords) {
    boostMap.set('pnr_status', 0.35)
  }
  if (hasDelayWords) boostMap.set('average_delay', (boostMap.get('average_delay') || 0) + 0.28)
  if (hasRouteWords) boostMap.set('train_schedule', (boostMap.get('train_schedule') || 0) + 0.28)
  if (hasChartWords) boostMap.set('chart_status', (boostMap.get('chart_status') || 0) + 0.25)
  if (hasPnrWords) boostMap.set('pnr_status', (boostMap.get('pnr_status') || 0) + 0.2)
  if (boostMap.size) {
    for (const e of scored) {
      const b = boostMap.get(e.p.intent.id)
      if (b) e.score = Math.min(1, e.score + b)
    }
    scored.sort((a, b) => b.score - a.score)
  }

  const best = scored[0]
  // MARGIN separates INTENTS: near-duplicate phrases of the best intent
  // don't compete; the runner-up is the strongest other-intent candidate.
  const runnerUp = scored.find((c) => c.p.intent.id !== best.p.intent.id) ?? null
  const margin = runnerUp ? best.score - runnerUp.score : best.score
  const confident = best.score >= ACCEPT && margin >= MARGIN
  const ambiguous = !confident && best.score >= REJECT
  if (!confident && !ambiguous) return null

  return {
    intent: best.p.intent,
    phrase: best.p.raw,
    score: round3(best.score),
    confidence: confident ? 'confident' : 'ambiguous',
    runnerUp: runnerUp
      ? {
          id: runnerUp.p.intent.id,
          phrase: runnerUp.p.raw,
          score: round3(runnerUp.score)
        }
      : null
  }
}

// ---------- help surfaces ----------

export const HELP_REPLY =
  "I'm Train Bro — I check **live data**: train running status, routes & timetables, average delays, trains between stations, seat availability, station boards, chart and PNR status. Try: \"live status of 12951\" or \"PNR 1234567890\"."

export const HELP_CHIPS = [
  { label: 'Live status', prompt: 'live status of 12951' },
  { label: 'Trains SC→PUNE', prompt: 'trains from SC to PUNE' },
  { label: 'Seats SC→PUNE', prompt: 'seat availability from SC to PUNE' },
  { label: 'PNR status', prompt: 'pnr status 1234567890' }
]

function helpVerdictWithForm(text, fallback) {
  const raw = String(text ?? '')
  const form = buildFormSpec(raw)
  const base = { kind: 'help', reply: HELP_REPLY, actions: HELP_CHIPS, form }
  if (fallback) {
    // preserve fallback tailored reply when we already have one
    base.reply = fallback.reply
    base.actions = fallback.actions
  }
  return base
}

const helpVerdict = (text) => helpVerdictWithForm(text ?? '')

const TRAIN_EXAMPLE_PROMPTS = {
  live_status: 'live status of 12951',
  average_delay: 'average delay of 12626',
  train_schedule: 'route of 12951',
  chart_status: 'chart status of 12951',
  pnr_status: 'pnr status 1234567890'
}

function missingSlotHelp(intentId, missing, text) {
  let base
  if (missing === 'train') {
    const prompt = TRAIN_EXAMPLE_PROMPTS[intentId] ?? 'live status of 12951'
    base = {
      kind: 'help',
      reply: `Which train? Give me the 5-digit number — e.g. "${prompt}".`,
      actions: [{ label: 'Try 12951', prompt }]
    }
  } else if (missing === 'pnr') {
    base = {
      kind: 'help',
      reply: 'Which PNR? Give me the 10-digit number — e.g. "pnr status 1234567890".',
      actions: [{ label: 'Try 1234567890', prompt: 'pnr status 1234567890' }]
    }
  } else if (missing === 'stations' && intentId === 'seat_availability') {
    base = {
      kind: 'help',
      reply: 'From where to where? e.g. "seat availability from SC to PUNE".',
      actions: [{ label: 'SC → PUNE', prompt: 'seat availability from SC to PUNE' }]
    }
  } else if (missing === 'stations') {
    base = {
      kind: 'help',
      reply: 'Between which stations? e.g. "trains from SC to PUNE".',
      actions: [{ label: 'SC → PUNE', prompt: 'trains from SC to PUNE' }]
    }
  } else {
    base = {
      kind: 'help',
      reply: 'Which station? e.g. "station board Pune".',
      actions: [{ label: 'Pune board', prompt: 'station board pune' }]
    }
  }
  const raw = String(text ?? '')
  const mapped = normalize(raw)
  const collected = collectForForm(raw, mapped)
  const candidates = buildCandidates(mapped)
  // For missing-slot help, force intentId to the matched intent (even if candidates would pick different)
  const best = candidates.find((c) => c.intentId === intentId) || candidates[0]
  const confidence = best ? best.score : 0
  const form = makeForm({ intentId, confidence, collected, candidates })
  // Override candidates to ensure top 2-3 include the intended intent first
  return { ...base, form }
}

function requiredSlotMissing(intent, ent, slots) {
  if (intent.needsTrain && !ent.train) return 'train'
  if (intent.needsPnr && !ent.pnr) return 'pnr'
  if (intent.needsSrcDst && (!slots.srcQuery || !slots.dstQuery)) return 'stations'
  if (intent.needsStation && !slots.stationQuery) return 'station'
  return null
}

// ---------- tier 0: deterministic regex fast-paths ----------
// Exact structural patterns beat the corpus; everything else falls through
// to fuzzy matching. Runs on the normalized + Hinglish-mapped text.

const SEAT_DEFER_RE =
  /\b(seat|berth|availabilit|tiket|ticket|reservation|waitlist|booking|tatkal)\b/
const AVG_DELAY_RE = /\b(average|avg)\s+(delay|late)\b|\bhow much delay\b/
const CHART_RE = /\b(chart|prepared|ready)\b/
const SCHEDULE_RE = /\b(route|schedule|timetable|time table|stops? at|halts?)\b/
const LIVE_VERBS_RE =
  /\b(live|status|running|position|where|reached|late|delayed|track)\b/
const PNR_RE_TIER = /\b(pnr|booking status)\b/i

function tierZero(mapped) {
  const num = (mapped.match(TRAIN_RE) || [])[1] ?? null
  const pnr = (mapped.match(PNR_RE) || [])[1] ?? null

  // PNR: strongest signal — if 10-digit + pnr word, route directly
  if (pnr && PNR_RE_TIER.test(mapped)) {
    return { kind: 'tool', plan: buildPlanFor('pnr_status', { pnr }), confidence: 1 }
  }
  // bare 10-digit with pnr word context even without explicit pnr word? if string is just 10 digits and recent context suggests pnr, but tierZero sees only text; handle pure 10-digit as pnr only when pnr word present
  if (pnr && /\b(pnr|booking)\b/.test(mapped)) {
    return { kind: 'tool', plan: buildPlanFor('pnr_status', { pnr }), confidence: 1 }
  }

  // Seat family keeps date/class nuance -> always routed through the
  // heavy-intent confirm flow, never a silent tool call.
  if (SEAT_DEFER_RE.test(mapped)) return null

  if (num && AVG_DELAY_RE.test(mapped)) {
    return { kind: 'tool', plan: buildPlanFor('average_delay', { train: num }), confidence: 1 }
  }
  if (num && CHART_RE.test(mapped)) {
    return { kind: 'tool', plan: buildPlanFor('chart_status', { train: num }), confidence: 1 }
  }
  if (num && SCHEDULE_RE.test(mapped)) {
    return { kind: 'tool', plan: buildPlanFor('train_schedule', { train: num }), confidence: 1 }
  }
  if (num && LIVE_VERBS_RE.test(mapped)) {
    return { kind: 'tool', plan: buildPlanFor('live_status', { train: num }), confidence: 1 }
  }

  // Bare 10-digit PNR without explicit word? treat as pnr if looks like pnr and no train ambience
  if (pnr && !num && mapped.trim().split(/\s+/).length <= 4) {
    // e.g. user typed just "1234567890"
    if (/^\d{10}$/.test(mapped.trim())) {
      return { kind: 'tool', plan: buildPlanFor('pnr_status', { pnr }), confidence: 1 }
    }
  }

  const bare = stripEntities(mapped)
  // trains_between via tierZero should handle any "X to Y" when train word present or route-like phrasing
  const hasRouteSignal = /\b(train|trains|between|direct|available|route|services)\b/.test(bare)
  if (hasRouteSignal || /\btrain/.test(bare)) {
    for (const re of BETWEEN_RES) {
      const m = bare.match(re)
      if (m) {
        const slots = {
          srcQuery: cleanStationQuery(m[1].trim()),
          dstQuery: cleanStationQuery(m[2].trim().replace(/\s+tak$/i, '').trim())
        }
        return { kind: 'tool', plan: buildPlanFor('trains_between', {}, slots), confidence: 1 }
      }
    }
    // fallback bare "X to Y" when explicit from/between not found but we have route signal
    const fb = fallbackBareTo(bare, null, null)
    if (fb.srcQuery && fb.dstQuery) {
      // only trigger if bare contains route-like cue or at least one token looks like station code (2-4 letters)
      if (hasRouteSignal || /^[a-z]{2,4}$/.test(fb.srcQuery) || /^[a-z]{2,4}$/.test(fb.dstQuery)) {
        return { kind: 'tool', plan: buildPlanFor('trains_between', {}, { srcQuery: fb.srcQuery, dstQuery: fb.dstQuery }), confidence: 1 }
      }
    }
  }

  for (const re of BOARD_RES) {
    const m = mapped.match(re)
    if (m) {
      const slots = { stationQuery: cleanStationQuery(m[1].trim()) }
      return { kind: 'tool', plan: buildPlanFor('station_board', {}, slots), confidence: 1 }
    }
  }
  // fallback board detection
  const fbBoard = fallbackBoard(mapped, null)
  if (fbBoard) {
    return { kind: 'tool', plan: buildPlanFor('station_board', {}, { stationQuery: fbBoard }), confidence: 1 }
  }

  return null
}

// ---------- classifier ----------

const TRIVIALS = [
  [
    /^(hi+|hey+|hello+|yo|hii+|namaste|namaskar|good\s*(morning|afternoon|evening)|vanakkam)[\s!.,]*$/i,
    "Hey! I'm Train Bro — ask me about live trains, routes, delays, seats or PNR."
  ],
  [/^(thanks?|thank\s*you|thx|ty|dhanyavad)[\s!.,]*$/i, 'Anytime. Where to next?'],
  [/^(bye|bye bye|see ya|alvida|good ?night)[\s!.,]*$/i, 'Safe travels! Come back before you board.'],
  [
    /(what can you do|^help$|who are you|how do you work|capabilities)/i,
    'I check **live data** for you: train status, routes, station boards, avg delays, seat availability, chart and PNR status. Try: "live status of 12951", "trains from Secunderabad to Pune" or "pnr 1234567890".'
  ]
]

/** NEVER returns kind:'llm' — every message resolves locally. See the file
 * header for the full kind contract. */
export function classify(text, memory) {
  const q = String(text ?? '').trim()
  if (!q) return helpVerdict(q)

  for (const [re, reply] of TRIVIALS) {
    if (re.test(q)) return { kind: 'trivial', reply }
  }

  if (memory) {
    const hit = findReplay(memory, q)
    if (hit) return { kind: 'replay', ...hit }
  }

  const mapped = normalize(q)
  const t0 = tierZero(mapped)
  if (t0) return t0

  const m = matchIntent(mapped)
  if (!m) return helpVerdict(q)

  const ent = extractEntities(q)
  const slots = extractSlots(stripEntities(mapped))

  // Heavy intent: always confirm before spending the expensive call.
  if (m.intent.id === 'seat_availability') {
    if (!slots.srcQuery || !slots.dstQuery) return missingSlotHelp('seat_availability', 'stations', q)
    return {
      kind: 'confirm',
      plan: buildPlanFor('seat_availability', ent, slots),
      text: `Check seat availability ${slots.srcQuery.toUpperCase()} → ${slots.dstQuery.toUpperCase()}?`,
      choices: [
        { label: 'Confirm', value: '__exec' },
        { label: 'Cancel', value: '__cancel' }
      ],
      confidence: m.score
    }
  }

  // PNR: if intent is pnr but pnr missing, help
  if (m.intent.id === 'pnr_status' && !ent.pnr) {
    return missingSlotHelp('pnr_status', 'pnr', q)
  }

  // Missing a hard requirement -> targeted help beats confirming a plan
  // that cannot execute yet (applies in both bands).
  const missing = requiredSlotMissing(m.intent, ent, slots)
  if (missing) return missingSlotHelp(m.intent.id, missing, q)

  if (m.confidence === 'confident') {
    return { kind: 'tool', plan: buildPlanFor(m.intent.id, ent, slots), confidence: m.score }
  }

  // Ambiguous band: offer the best-guess phrase for one-tap confirmation.
  return {
    kind: 'confirm',
    plan: buildPlanFor(m.intent.id, ent, slots),
    text: `Did you mean: ${m.phrase}? `,
    choices: [
      { label: 'Yes, fetch it', value: '__exec' },
      { label: 'Cancel', value: '__cancel' }
    ],
    runnerUp: m.runnerUp,
    confidence: m.score
  }
}

// ---------- plan execution helpers ----------

const CODE_RE = /^[a-z]{2,5}$/i

/** Resolve one `$slot` reference via /rail-api/search/suggest.
 * Returns the station/train code, or null when nothing unambiguous shows up. */
export async function resolveSlot(fetcher, entry) {
  const raw = String(entry?.query ?? '').trim()
  if (!raw) return null
  if (CODE_RE.test(raw) && !raw.includes(' ')) return raw.toUpperCase()
  try {
    const res = await fetcher(`/rail-api/search/suggest?q=${encodeURIComponent(raw)}`)
    if (!res.ok) return null
    const hits = await res.json()
    const pick =
      hits.find((h) => h.type === 'station') ??
      (entry.preferCode ? null : hits.find((h) => h.type === 'train'))
    return pick?.code ?? pick?.number ?? null
  } catch {
    return null
  }
}

/** Execute a plan: resolve slots, fetch, project. Throws on hard failure so
 * the caller can fall back to the LLM path. */
export async function executePlan(plan, fetcher) {
  const params = { ...(plan.params ?? {}) }
  for (const entry of plan.resolve ?? []) {
    const code = await resolveSlot(fetcher, entry)
    if (!code) throw new Error(`could not resolve "${entry.query}"`)
    params[entry.slot] = code
  }
  const usp = new URLSearchParams(
    Object.entries(params).map(([k, v]) => [k, String(v)])
  )
  const res = await fetcher(`${plan.url}?${usp}`)
  if (!res.ok) throw new Error(`${plan.url} -> ${res.status}`)
  return res.json()
}

// ---------- DTO -> card-data projections (mirror src/slices/ai_chat/tools.rs) ----------

export function projectLiveStatus(dto = {}) {
  let lastDelay = null
  const nextStops = []
  for (const s of dto.stations ?? []) {
    if (s.status === 'departed') lastDelay = s.delay_minutes
    else if ((s.status === 'expected' || s.status === 'scheduled') && nextStops.length < MAX_UPCOMING_STOPS) {
      nextStops.push({
        code: s.code,
        name: s.name,
        sch: s.scheduled_arrival,
        act: s.actual_arrival || null,
        delay_min: s.delay_minutes,
        platform: s.platform || null
      })
    }
  }
  return {
    train_number: dto.train_number ?? null,
    train_name: dto.train_name ?? null,
    position: dto.current_location_info ?? null,
    platform: dto.platform_number ?? null,
    data_source: dto.data_source ?? null,
    last_seen_delay_minutes: lastDelay,
    next_stops: nextStops
  }
}

function runsLabel(mask) {
  if (Array.isArray(mask) && mask.length === 7 && mask.every(Boolean)) return 'Daily'
  return (mask ?? []).map((on, i) => (on ? DAY_NAMES[i] : null)).filter(Boolean).join(' ')
}

export function projectTrainsBetween(dto = {}, resolved = {}) {
  const all = dto.trains ?? []
  const trains = all.slice(0, MAX_BETWEEN_TRAINS).map((t) => ({
    number: t.number,
    name: t.name,
    dep: t.departure_time,
    arr: t.arrival_time,
    runs: runsLabel(t.runs_on)
  }))
  return {
    from: dto.src ?? null,
    to: dto.dst ?? null,
    total_found: all.length,
    data_source: dto.data_source ?? null,
    note: all.length > MAX_BETWEEN_TRAINS ? `showing first ${MAX_BETWEEN_TRAINS} of ${all.length}` : null,
    trains,
    ...(resolved.src ? { src_code: resolved.src } : {}),
    ...(resolved.dst ? { dst_code: resolved.dst } : {})
  }
}

export function projectAverageDelay(dto = {}) {
  const parseMin = (s) => {
    const raw = String(s ?? '').trim()
    if (!raw) return null
    const n = Number(raw.replace(/^\+/, ''))
    return Number.isFinite(n) ? n : null
  }
  const rows = [...(dto.stations ?? [])]
    .map((r) => ({ ...r, _a: parseMin(r.arrival_delay), _d: parseMin(r.departure_delay) }))
    .sort((x, y) => (y._a ?? -Infinity) - (x._a ?? -Infinity))
    .slice(0, 10)
    .map((r) => ({
      code: r.code,
      name: r.name,
      arr_delay_min: r._a,
      dep_delay_min: r._d
    }))
  return {
    train_no: dto.train_no ?? null,
    train_name: dto.train_name ?? null,
    days_of_run: dto.days_of_run ?? null,
    data_source: dto.data_source ?? null,
    stations_worst_first: rows
  }
}

export function projectSchedule(dto = {}) {
  const stopsAll = dto.stops ?? []
  return {
    train_number: dto.train_number ?? null,
    train_name: dto.train_name ?? null,
    running_days: dto.running_days ?? [],
    data_source: dto.source ?? null,
    total_stops: stopsAll.length,
    note: stopsAll.length > MAX_SCHEDULE_STOPS ? `showing first ${MAX_SCHEDULE_STOPS} of ${stopsAll.length}` : null,
    stops: stopsAll.slice(0, MAX_SCHEDULE_STOPS).map((s) => ({
      code: s.code,
      name: s.name,
      arr: s.arrival,
      dep: s.departure,
      day: s.day ?? null,
      km: s.distance_km != null ? Math.round(s.distance_km) : null
    }))
  }
}

export function projectStationBoard(dto = {}) {
  return {
    station_code: dto.station ?? null,
    hours: dto.hours ?? null,
    data_source: dto.data_source ?? null,
    trains: (dto.trains ?? []).slice(0, BOARD_MAX_TRAINS).map((t) => ({
      number: t.number,
      name: t.name,
      sch: t.sta,
      eta: t.eta,
      platform: t.platform,
      late: t.delay_arr
    }))
  }
}

/// Mirrors tools.rs availability_tone: green when bookable now, red when
/// hopeless, amber for waitlist-ish limbo.
function availabilityTone(available, status) {
  if (available === true) return 'ok'
  if (available === false) return 'bad'
  const s = String(status ?? '').trim().toUpperCase()
  if (s.startsWith('RAC') || s.includes('WL')) return 'warn'
  if (s.startsWith('REGRET') || s.startsWith('NOT')) return 'bad'
  if (s.includes('AVAILABLE')) return 'ok'
  return 'warn'
}

/** Mirrors tools.rs project_seat_availability: trains with class-wise
 * status rank first, rows capped at 8, classes capped at 6, fare/prediction
 * keys only materialize when the source provided them. `resolved` injects
 * src_code/dst_code the way the server does after projecting. */
export function projectSeatAvailability(dto = {}, resolved = {}) {
  const trains = dto.trains ?? []
  const ranked = [
    ...trains.filter((t) => (t.availability ?? []).length > 0),
    ...trains.filter((t) => !(t.availability ?? []).length)
  ]
  const rows = ranked.slice(0, MAX_AVAILABILITY_TRAINS).map((t) => ({
    number: t.number,
    name: t.name,
    dep: t.departure_time,
    arr: t.arrival_time,
    duration: t.duration,
    classes: (t.availability ?? []).slice(0, MAX_AVAILABILITY_CLASSES).map((c) => {
      const row = {
        class: c.class,
        status: c.status,
        tone: availabilityTone(c.available, c.status)
      }
      if (c.fare != null) row.fare = c.fare
      if (c.prediction != null) row.prediction = c.prediction
      return row
    })
  }))
  return {
    from: dto.src ?? null,
    to: dto.dst ?? null,
    date: dto.date ?? null,
    data_source: dto.data_source ?? null,
    notice: dto.notice ?? null,
    trains: rows,
    ...(resolved.src ? { src_code: resolved.src } : {}),
    ...(resolved.dst ? { dst_code: resolved.dst } : {})
  }
}

/** Mirrors tools.rs project_chart: identity + coach count only; the DTO's
 * coach bodies and train_name are deliberately dropped. */
export function projectChartStatus(dto = {}) {
  return {
    train_number: dto.train_number ?? null,
    journey_date: dto.journey_date ?? null,
    boarding_station: dto.boarding_station ?? null,
    coach_count: Array.isArray(dto.coaches) ? dto.coaches.length : null,
    data_source: dto.data_source ?? null,
    notice: dto.notice ?? null
  }
}

export function projectPnrStatus(dto = {}) {
  const pax = Array.isArray(dto.passengers) ? dto.passengers.slice(0, 6).map((p) => ({
    booking_status: p.booking_status ?? '',
    current_status: p.current_status ?? '',
    coach: p.coach ?? '',
    berth: p.berth ?? ''
  })) : []
  return {
    pnr: dto.pnr ?? null,
    train_number: dto.train_number ?? null,
    train_name: dto.train_name ?? null,
    journey_date: dto.journey_date ?? null,
    from: dto.from ? { code: dto.from.code, name: dto.from.name, time: dto.from.time } : null,
    to: dto.to ? { code: dto.to.code, name: dto.to.name, time: dto.to.time } : null,
    passengers: pax,
    data_source: dto.data_source ?? null,
    notice: dto.notice ?? null
  }
}

export const PROJECTORS = {
  live_status: projectLiveStatus,
  trains_between: projectTrainsBetween,
  average_delay: projectAverageDelay,
  train_schedule: projectSchedule,
  station_board: projectStationBoard,
  seat_availability: projectSeatAvailability,
  chart_status: projectChartStatus,
  pnr_status: projectPnrStatus
}

// ---------- next-action chips (mirror tools::next_actions, subset) ----------

export function nextActionsFor(kind, d = {}) {
  const out = []
  const push = (label, prompt) => {
    if (label && prompt && !out.some((x) => x.label === label)) out.push({ label, prompt })
  }
  if (kind === 'trains_between') {
    const n = d.trains?.[0]?.number
    if (n) push(`Track ${n}`, `live status of ${n}`)
    if (d.src_code && d.dst_code) push(`Availability ${d.src_code}→${d.dst_code}`, `seat availability from ${d.src_code} to ${d.dst_code}`)
    if (d.src_code && d.dst_code) push(`PNR check`, `pnr status 1234567890`)
  } else if (kind === 'live_status') {
    const n = d.train_number
    if (n) {
      push(`Route of ${n}`, `route of train ${n}`)
      push(`Avg delay ${n}`, `average delay of train ${n}`)
    }
  } else if (kind === 'station_board') {
    const n = d.trains?.find((t) => t.number)?.number
    if (n) push(`Track ${n}`, `live status of ${n}`)
  } else if (kind === 'train_schedule') {
    const n = d.train_number
    if (n) push(`Chart status ${n}`, `chart status of train ${n}`)
  } else if (kind === 'average_delay') {
    const n = d.train_no
    if (n) push(`Track ${n}`, `live status of ${n}`)
  } else if (kind === 'seat_availability') {
    const n = d.trains?.find((t) => t.number)?.number
    if (n) push(`Chart ${n}`, `chart status of train ${n}`)
  } else if (kind === 'chart_status') {
    const n = d.train_number
    if (n) push(`Track ${n}`, `live status of ${n}`)
  } else if (kind === 'pnr_status') {
    const n = d.train_number
    if (n) push(`Track ${n}`, `live status of ${n}`)
    else push(`Live status`, `live status of 12951`)
  }
  return out.slice(0, 4)
}
