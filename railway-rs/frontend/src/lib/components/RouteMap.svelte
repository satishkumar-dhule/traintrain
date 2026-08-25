<script>
  import * as Card from '$lib/components/ui/card/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import L from 'leaflet'
  import 'leaflet/dist/leaflet.css'

  /** Train-on-map API response (route/track stations with lat-lng,
       optional live current_station / journey_station spot view). */
  let { data = null } = $props()

  let mapEl = $state(null)
  // reused across $effect invocations - not $state (avoid infinite loops)
  let mapInstance = null
  let featureGroup = null

  /* Single-pass, memoized line+stops derivation (shallow compare via $derived).
     Previously two separate filters over same data.route; now one pass. */
  const coords = $derived.by(() => {
    const rawRoute = data?.route ?? []
    const rawTrack = data?.track ?? []
    const stops = []
    const track = []
    for (const s of rawRoute) {
      if (Number.isFinite(s?.lat) && Number.isFinite(s?.lng)) stops.push(s)
    }
    for (const s of rawTrack) {
      if (Number.isFinite(s?.lat) && Number.isFinite(s?.lng)) track.push(s)
    }
    const line = track.length >= 2 ? track : stops
    return { line, stops }
  })
  const line = $derived(coords.line)
  const stops = $derived(coords.stops)

  const currentCode = $derived(data?.current_station?.code ?? null)

  // memoized tooltipLabel (tiny LRU)
  const _tipCache = new Map()
  function tooltipLabel(s) {
    const k = `${s.code}|${s.name ?? ''}`
    const hit = _tipCache.get(k)
    if (hit) return hit
    const v = s.code + (s.name ? ` — ${s.name}` : '')
    _tipCache.set(k, v)
    if (_tipCache.size > 256) {
      const first = _tipCache.keys().next().value
      _tipCache.delete(first)
    }
    return v
  }

  /* Reuse map instance: create once, then diff via featureGroup.clearLayers()
     + invalidateSize. Delegated click via featureGroup single handler (n2). */
  $effect(() => {
    const el = mapEl
    const l = line
    const st = stops
    const cur = currentCode
    const d = data
    // access reactive deps explicitly for effect tracking
    void l.length
    void st.length
    void cur
    void d?.current_station
    void d?.journey_station

    if (!el || l.length < 2) {
      if (mapInstance) {
        mapInstance.remove()
        mapInstance = null
        featureGroup = null
      }
      return
    }

    if (!mapInstance) {
      mapInstance = L.map(el, { zoomControl: true, attributionControl: true })
      L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '&copy; OpenStreetMap contributors',
        maxZoom: 18,
      }).addTo(mapInstance)
      featureGroup = L.featureGroup().addTo(mapInstance)
      // super n2 delegation: single handler for all markers
      featureGroup.on('click', (e) => {
        const ll = e.latlng
        if (ll && mapInstance) mapInstance.panTo(ll, { animate: true })
      })
    } else {
      featureGroup.clearLayers()
      mapInstance.invalidateSize()
    }

    L.polyline(
      l.map((s) => [s.lat, s.lng]),
      { color: '#2563eb', weight: 3, opacity: 0.85, lineJoin: 'round', lineCap: 'round' },
    ).addTo(featureGroup)

    for (const s of st) {
      const isCurrent = cur && s.code === cur
      const marker = L.circleMarker(
        [s.lat, s.lng],
        isCurrent
          ? { radius: 7, fillColor: '#dc2626', color: '#fff', weight: 2, fillOpacity: 0.9 }
          : { radius: 4, fillColor: '#2563eb', color: '#fff', weight: 1.5, fillOpacity: 0.85 },
      )
      marker.options.code = s.code
      marker.bindTooltip(tooltipLabel(s), { direction: 'top', offset: [0, -6] })
      marker.addTo(featureGroup)
    }

    const cs = d?.current_station
    if (cs && Number.isFinite(cs.lat) && Number.isFinite(cs.lng)) {
      L.circleMarker([cs.lat, cs.lng], {
        radius: 5,
        fillColor: '#f97316',
        color: '#fff',
        weight: 2,
        fillOpacity: 0.9,
      })
        .addTo(featureGroup)
        .bindTooltip(`Current: ${cs.code || ''}`, {
          permanent: true,
          direction: 'top',
          offset: [0, -6],
          className: 'font-medium',
        })
    }

    const js = d?.journey_station
    if (js && Number.isFinite(js.lat) && Number.isFinite(js.lng)) {
      L.circleMarker([js.lat, js.lng], {
        radius: 6,
        fillColor: '#16a34a',
        color: '#fff',
        weight: 2,
        fillOpacity: 0.9,
      })
        .addTo(featureGroup)
        .bindTooltip(`Your stop: ${js.code || ''}`, {
          permanent: true,
          direction: 'bottom',
          offset: [0, 6],
          className: 'font-medium',
        })
    }

    mapInstance.fitBounds(
      l.map((s) => [s.lat, s.lng]),
      { padding: [30, 30] },
    )
  })

  // onDestroy cleanup for reused instance
  $effect(() => {
    return () => {
      if (mapInstance) {
        mapInstance.remove()
        mapInstance = null
        featureGroup = null
      }
    }
  })
</script>

<Card.Root>
  <Card.Header>
    <Card.Title class="flex flex-wrap items-center gap-2">Route map</Card.Title>
    <Card.Description class="flex flex-wrap items-center gap-2">
      <span>{data?.source ?? ''} → {data?.destination ?? ''}</span>
      {#if data?.data_source}
        <Badge variant="outline" class="font-normal">{data.data_source}</Badge>
      {/if}
    </Card.Description>
  </Card.Header>
  <Card.Content>
    {#if line.length >= 2}
      <div bind:this={mapEl} class="h-[380px] w-full overflow-hidden rounded-md border md:h-[440px]" role="img" aria-label="Route map of train {data?.train_no ?? ''}"></div>
      <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
        <span class="flex items-center gap-1.5"><span class="inline-block size-2.5 rounded-full bg-[#2563eb]"></span> halt</span>
        <span class="flex items-center gap-1.5"><span class="inline-block size-2.5 rounded-full bg-[#dc2626]"></span> current (last reported halt)</span>
        {#if data?.current_station && Number.isFinite(data.current_station.lat)}
          <span class="flex items-center gap-1.5"><span class="inline-block size-2.5 rounded-full bg-[#f97316]"></span> live position</span>
        {/if}
        {#if data?.journey_station}
          <span class="flex items-center gap-1.5"><span class="inline-block size-2.5 rounded-full bg-[#16a34a]"></span> your stop</span>
        {/if}
      </div>
    {:else}
      <div
        class="flex flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
      >
        <span>No coordinate data for this route.</span>
      </div>
    {/if}
  </Card.Content>
</Card.Root>
