/* Shared avg-delay store backing TrainDelayBadge.
   One request per train per session (plus a retry window on failure), so the
   badge can be dropped anywhere a train is shown without hammering NTES.
   Fetches start on a microtask so reads stay safe during renders. */
import { api } from '$lib/api.js'

const RETRY_MS = 5 * 60 * 1000

export const trainDelays = $state({})

function is5Digit(t) {
  return /^\d{5}$/.test(String(t ?? '').trim())
}

async function load(key) {
  trainDelays[key] = { status: 'loading', data: null, at: Date.now() }
  const res = await api(`/rail-api/ntes/average-delay?train=${encodeURIComponent(key)}`)
  const cur = trainDelays[key]
  if (!cur || cur.status !== 'loading') return
  if (res.ok && res.data?.stations?.length) {
    trainDelays[key] = { status: 'ok', data: res.data, at: Date.now() }
  } else {
    trainDelays[key] = {
      status: 'error',
      data: null,
      at: Date.now(),
      error: res.error || 'unavailable'
    }
  }
}

/* Read a train's avg-delay entry, lazily starting a background fetch for it.
   Safe to call anywhere a train is displayed; returns null until data lands. */
export function trainDelayEntry(train) {
  const key = String(train ?? '').trim()
  if (!is5Digit(key)) return null
  const cur = trainDelays[key]
  if (
    !cur ||
    (cur.status === 'error' && Date.now() - (cur.at ?? 0) > RETRY_MS)
  ) {
    queueMicrotask(() => {
      const now = trainDelays[key]
      if (now && now.status !== 'error') return
      if (now && Date.now() - (now.at ?? 0) <= RETRY_MS) return
      load(key)
    })
  }
  return cur ?? null
}

/* Prime the store with an already-loaded AverageDelayResponse so pages that
   fetched it themselves don't refetch. */
export function primeTrainDelay(data) {
  const key = String(data?.train_no ?? '').trim()
  if (!is5Digit(key)) return
  if (trainDelays[key]?.status === 'ok') return
  trainDelays[key] = { status: 'ok', data, at: Date.now() }
}
