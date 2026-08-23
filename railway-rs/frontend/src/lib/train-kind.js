/* Train-kind classification and kind-calibrated delay severity.
   Used by the avg-delay badge wherever a train is displayed. */

/* Severity thresholds per kind: [goodMax, badMax] in minutes of average
   delay. Slower trains run with looser punctuality expectations, so the same
   lateness reads worse on a premium/superfast than on a passenger. */
const KIND_THRESHOLDS = {
  premium: { good: 5, bad: 20 },
  superfast: { good: 10, bad: 30 },
  express: { good: 15, bad: 40 },
  passenger: { good: 20, bad: 50 }
}

const PREMIUM_RE =
  /rajdhani|shatabdi|duronto|vande\s*bharat|tejas|humsafar|garib\s*rath|sampark\s*kranti|antyodaya/i

const SLOW_RE = /passenger|\bmemu\b|\bdemu\b|\bemu\b|\blocal\b|\bslow\b/i

export function classifyTrainKind({ number = '', name = '', type = '' } = {}) {
  const nm = String(name ?? '')
  const tp = String(type ?? '')

  if (PREMIUM_RE.test(nm) || PREMIUM_RE.test(tp)) return 'premium'
  if (/\bsuperfast\b|\bsf\b|\bsuf\b/i.test(nm) || /\bsuperfast|\bsuf\b/i.test(tp)) return 'superfast'
  if (SLOW_RE.test(nm) || SLOW_RE.test(tp)) return 'passenger'

  const num = String(number ?? '').trim()
  /* 5xxxx–7xxxx series are passenger/MEMU/DEMU locals. */
  if (/^[567]\d{4}$/.test(num)) return 'passenger'

  return 'express'
}

export function kindLabel(kind) {
  switch (kind) {
    case 'premium':
      return 'Premium'
    case 'superfast':
      return 'Superfast'
    case 'passenger':
      return 'Slow'
    default:
      return 'Fast'
  }
}

/* NTES average-delay strings: "" (unknown), "On Time" (0), "HH:MM" (h:mm),
   or signed plain minutes ("+12"). → signed minutes, else null. */
export function parseAvgDelayMinutes(v) {
  const str = String(v ?? '').trim()
  if (!str) return null
  if (/on time/i.test(str)) return 0
  let m = /^(\d{1,3}):(\d{2})$/.exec(str)
  if (m) return Number(m[1]) * 60 + Number(m[2])
  m = /^[+-]?\d+$/.exec(str)
  return m ? Number(str) : null
}

/* Summarize the average-delay station pattern into minutes:
   mean over every parseable arrival/departure value plus the worst one. */
export function avgDelayPattern(stations) {
  let sum = 0
  let count = 0
  let worst = null
  for (const s of Array.isArray(stations) ? stations : []) {
    for (const v of [s?.arrival_delay, s?.departure_delay]) {
      const n = parseAvgDelayMinutes(v)
      if (n == null || n < 0) continue
      sum += n
      count += 1
      if (worst == null || n > worst) worst = n
    }
  }
  if (!count) return { mean: null, worst: null, samples: 0 }
  return { mean: Math.round(sum / count), worst, samples: count }
}

/* Severity tier for an average delay given the train's kind:
   good ≤ goodMax; minor ≤ badMax; major beyond. */
export function kindDelayKind(kind, minutes) {
  if (minutes == null) return 'none'
  const t = KIND_THRESHOLDS[kind] ?? KIND_THRESHOLDS.express
  if (minutes <= t.good) return 'good'
  return minutes <= t.bad ? 'minor' : 'major'
}
