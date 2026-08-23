import { api } from '$lib/api.js'

// Shared state for the global "stations near you" blocking dialog.
// One dialog serves every station input on the site; `pickNearbyStation()`
// returns a promise that resolves with `{ code, name, distance_km, ... }`
// once the user picks a row, or `null` when the dialog is dismissed.
export const nearby = $state({
  open: false,
  phase: 'idle', // locating | loading | ready | error
  message: '',
  stations: []
})

let resolveFn = null
let activePromise = null

export function pickNearbyStation() {
  if (activePromise) return activePromise
  nearby.open = true
  nearby.phase = 'locating'
  nearby.message = ''
  nearby.stations = []
  activePromise = new Promise((resolve) => {
    resolveFn = resolve
  })
  locate()
  return activePromise
}

export function chooseNearby(station) {
  settle(station)
}

export function cancelNearby() {
  settle(null)
}

function settle(value) {
  const resolve = resolveFn
  resolveFn = null
  activePromise = null
  nearby.open = false
  resolve?.(value ?? null)
}

function locate() {
  if (!('geolocation' in navigator)) {
    nearby.phase = 'error'
    nearby.message = 'Geolocation is not available in this browser.'
    return
  }
  navigator.geolocation.getCurrentPosition(
    async ({ coords }) => {
      if (!activePromise) return
      nearby.phase = 'loading'
      const res = await api(
        `/rail-api/nearby/stations?lat=${encodeURIComponent(coords.latitude)}&lng=${encodeURIComponent(coords.longitude)}`
      )
      if (!activePromise) return
      if (!res.ok || !Array.isArray(res.data?.stations)) {
        nearby.phase = 'error'
        nearby.message =
          res.error && res.status !== 0 ? `Nearby lookup failed — ${res.error}` : 'Nearby station lookup failed. Check your connection and try again.'
        return
      }
      nearby.stations = res.data.stations
      nearby.phase = 'ready'
    },
    (err) => {
      if (!activePromise) return
      nearby.phase = 'error'
      nearby.message =
        err && err.code === 1
          ? 'Location permission denied — allow location access to find stations near you.'
          : (err && err.message) || 'Could not determine your location.'
    },
    { timeout: 10000, maximumAge: 60000 }
  )
}
