/* gate.js - local-first intent router for the assistant.
   Classifies a user message BEFORE any network call:
     trivial  -> canned reply, zero requests
     replay   -> session memory hit (exact or similar), zero requests
     tool     -> deterministic single-tool plan against plain REST endpoints
                 (no LLM round-trip)
     llm      -> everything ambiguous; the only path that reaches /ai/chat
   Pure and Node-testable: no DOM, no fetch. The caller executes plans and
   feeds DTOs to the exported `project*` mappers, which mirror the server-side
   ai_chat projections 1:1 so ToolCards renders identical shapes. */

const TRAIN_RE = /\b([1-9]\d{4})\b/
import { findReplay } from './memory.js'
const DAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
const MAX_BETWEEN_TRAINS = 12
const MAX_UPCOMING_STOPS = 4
const MAX_SCHEDULE_STOPS = 12
const BOARD_MAX_TRAINS = 8

const TRIVIALS = [
  [
    /^(hi+|hey+|hello+|yo|hii+|namaste|namaskar|good\s*(morning|afternoon|evening)|vanakkam)[\s!.,]*$/i,
    "Hey! I'm Train Bro — ask me about live trains, routes, delays or seats."
  ],
  [/^(thanks?|thank\s*you|thx|ty|dhanyavad)[\s!.,]*$/i, 'Anytime. Where to next?'],
  [/^(bye|bye bye|see ya|alvida|good ?night)[\s!.,]*$/i, 'Safe travels! Come back before you board.'],
  [
    /(what can you do|^help$|who are you|how do you work|capabilities)/i,
    'I check **live data** for you: train status, routes, station boards, avg delays and seat availability. Try: "live status of 12951" or "trains from Secunderabad to Pune".'
  ]
]

export function classify(text, memory) {
  const q = String(text ?? '').trim()
  if (!q) return { kind: 'llm' }

  for (const [re, reply] of TRIVIALS) {
    if (re.test(q)) return { kind: 'trivial', reply }
  }

  if (memory) {
    const hit = findReplay(memory, q)
    if (hit) return { kind: 'replay', ...hit }
  }

  const tool = matchTool(q)
  if (tool) return tool

  return { kind: 'llm' }
}

function matchTool(q) {
  const lower = q.toLowerCase()

  // Seat availability needs date/class nuance -> LLM handles it better.
  if (/(seat|availability|berth|ticket|fare|chart)/i.test(lower)) return null

  const num = (q.match(TRAIN_RE) || [])[1] ?? null

  // "average delay of 12951"
  if (/(average|avg)\s+(delay|late)/i.test(lower) && num) {
    return {
      kind: 'tool',
      plan: { cardKind: 'average_delay', url: '/rail-api/ntes/average-delay', params: { train: num } }
    }
  }

  // "route of 12951", "schedule of 12626", "12951 timetable"
  if (/\b(route|schedule|timetable|time table|stops at)\b/i.test(lower) && num) {
    return {
      kind: 'tool',
      plan: { cardKind: 'train_schedule', url: '/rail-api/schedule', params: { train: num } }
    }
  }

  // "live status of 12951", "where is 12951", "12951 running status"
  if (num && /\b(live|status|running|position|where|reached|late|delayed)\b/i.test(lower)) {
    return {
      kind: 'tool',
      plan: { cardKind: 'live_status', url: '/rail-api/live-status', params: { train: num } }
    }
  }

  // "trains from SC to PUNE", "trains between secunderabad and pune"
  // (before station-board: "trains from X" alone is ambiguous, "from X to Y" is not)
  const between = lower.match(/\b(?:from|between)\s+(.+?)\s+(?:to|and)\s+(.+?)\s*\??$/) && /train/i.test(lower)
  if (between) {
    return {
      kind: 'tool',
      plan: {
        cardKind: 'trains_between',
        url: '/rail-api/ntes/trains-between',
        params: { src: '$src', dst: '$dst' },
        resolve: [
          { slot: 'src', query: between[1] },
          { slot: 'dst', query: between[2] }
        ]
      }
    }
  }

  // "station board SC", "arrivals at pune", "trains at secunderabad"
  const board = lower.match(/\b(?:board|arrivals?|departures?)\b(?:\s*(?:at|for|of))?\s+([a-z][a-z .'-]{1,30})\s*\??$/) ||
    lower.match(/^trains\s+(?:at|from)\s+([a-z][a-z .'-]{1,30})\s*\??$/)
  if (board) {
    return {
      kind: 'tool',
      plan: {
        cardKind: 'station_board',
        url: '/rail-api/ntes/live-station',
        params: { station: '$station' },
        resolve: [{ slot: 'station', query: board[1].trim(), preferCode: true }]
      }
    }
  }

  return null
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

export const PROJECTORS = {
  live_status: projectLiveStatus,
  trains_between: projectTrainsBetween,
  average_delay: projectAverageDelay,
  train_schedule: projectSchedule,
  station_board: projectStationBoard
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
  }
  return out.slice(0, 4)
}
