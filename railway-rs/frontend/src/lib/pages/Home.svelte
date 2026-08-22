<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Separator } from '$lib/components/ui/separator/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { StationCodeBadge } from '$lib/components/badges/index.js'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import RouteIcon from 'lucide-svelte/icons/route'
  import Ticket from 'lucide-svelte/icons/ticket'
  import SearchIcon from 'lucide-svelte/icons/search'
  import MapPinIcon from 'lucide-svelte/icons/map-pin'
  import LocateFixedIcon from 'lucide-svelte/icons/locate-fixed'
  import XIcon from 'lucide-svelte/icons/x'

  const popularTrains = [12951, 12309, 12002]

  let trainQuery = $state('')
  let stationQuery = $state('')
  let journeyFrom = $state('')
  let journeyTo = $state('')
  let pnrQuery = $state('')
  let trainErr = $state('')
  let trainResolving = $state(false)
  let stationErr = $state('')
  let journeyErr = $state('')
  let pnrErr = $state('')

  // ---- Plan from where you are -------------------------------------------
  // Step 1: locate -> list every nearby station and let the user pick one.
  // Step 2: pick a destination (+ date) -> jump straight to the availability
  // page for that trip instead of mutating the home page.
  function todayISO() {
    const d = new Date()
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  }
  const TODAY = todayISO()
  const DATE_RE = /^\d{4}-\d{2}-\d{2}$/
  const NEARBY_CAP = 8

  let nearPhase = $state('idle') // idle | locating | loading | msg | ok
  let nearMsg = $state('')
  let nearbyStations = $state([])
  let originCode = $state('')

  let destQuery = $state('')
  let journeyDate = $state(TODAY)
  let planError = $state('')

  const destCode = $derived(norm(destQuery))
  const canPlan = $derived(Boolean(originCode) && destCode.length > 0)

  function findNearby() {
    if (!('geolocation' in navigator)) {
      nearPhase = 'msg'
      nearMsg = 'Geolocation is not available in this browser.'
      return
    }
    nearPhase = 'locating'
    navigator.geolocation.getCurrentPosition(
      async ({ coords }) => {
        nearPhase = 'loading'
        const res = await api(
          `/rail-api/askdisha/nearby?lat=${encodeURIComponent(coords.latitude)}&lng=${encodeURIComponent(coords.longitude)}`
        )
        const list =
          res.ok && Array.isArray(res.data?.stations)
            ? res.data.stations.slice(0, NEARBY_CAP)
            : null
        if (!list) {
          nearPhase = 'msg'
          nearMsg = res.error
            ? `Nearby lookup unavailable — ${res.error}`
            : 'Nearby station lookup is unavailable right now.'
          return
        }
        if (list.length === 0) {
          nearPhase = 'msg'
          nearMsg = 'No stations found around your location.'
          return
        }
        nearbyStations = list
        if (!list.some((s) => s.code === originCode)) originCode = list[0].code
        nearPhase = 'ok'
      },
      (err) => {
        nearPhase = 'msg'
        nearMsg =
          err && err.code === 1
            ? 'Location permission denied — allow location access to find nearby stations.'
            : (err && err.message) || 'Could not determine your location.'
      },
      { timeout: 10000, maximumAge: 60000 },
    )
  }

  function clearOrigin() {
    originCode = ''
  }

  function chooseOrigin(code) {
    originCode = code
  }

  function nearbyDistance(km) {
    const n = Number(km)
    return Number.isFinite(n) ? `${n.toFixed(1)} km` : null
  }

  function onDestPick(item) {
    if (item && norm(item.code)) destQuery = norm(item.code)
    if (canPlan && DATE_RE.test(journeyDate)) submitPlan()
  }

  function submitPlan(e) {
    e?.preventDefault?.()
    if (!originCode) {
      planError = 'Pick one of your nearby stations first.'
      return
    }
    if (!destCode) {
      planError = 'Enter a destination station code or name.'
      return
    }
    if (destCode === norm(originCode)) {
      planError = 'Destination must differ from your starting station.'
      return
    }
    if (!DATE_RE.test(journeyDate)) {
      planError = 'Pick a valid journey date.'
      return
    }
    planError = ''
    navigate(
      `/availability/${encodeURIComponent(originCode)}/${encodeURIComponent(destCode)}/${encodeURIComponent(journeyDate)}`
    )
  }

  function norm(v) {
    return String(v ?? '').trim().toUpperCase()
  }

  function goTrain(t) {
    trainErr = ''
    navigate(`/train/${encodeURIComponent(t)}`)
  }

  async function submitTrain(e) {
    e?.preventDefault?.()
    const t = String(trainQuery ?? '').trim()
    if (!t) {
      trainErr = 'Enter a train number or name.'
      return
    }
    if (/^\d{1,8}$/.test(t)) {
      goTrain(t)
      return
    }
    // Free-text name: resolve to a train number via the suggest index.
    trainErr = ''
    trainResolving = true
    const res = await api(`/rail-api/search/suggest?q=${encodeURIComponent(t)}`)
    trainResolving = false
    const items = Array.isArray(res.data) ? res.data.filter((r) => r.type === 'train') : []
    if (!res.ok || items.length === 0) {
      trainErr = 'No train matched — type a number or pick a name from the suggestions.'
      return
    }
    const ql = t.toLowerCase()
    const exact = items.filter(
      (r) => String(r.number ?? '') === t || String(r.name ?? '').toLowerCase() === ql
    )
    const best = exact.length ? exact : items
    if (best.length > 1) {
      trainErr = 'Several trains match — pick one from the suggestions.'
      return
    }
    goTrain(String(best[0].number))
  }

  function submitStation(e) {
    e?.preventDefault?.()
    const c = norm(stationQuery)
    if (!c) {
      stationErr = 'Pick a station code, e.g. NDLS.'
      return
    }
    stationErr = ''
    navigate(`/station/${encodeURIComponent(c)}`)
  }

  function submitJourney(e) {
    e?.preventDefault?.()
    const s = norm(journeyFrom)
    const d = norm(journeyTo)
    if (!s || !d) {
      journeyErr = 'Enter both source and destination codes.'
      return
    }
    if (s === d) {
      journeyErr = 'Source and destination must differ.'
      return
    }
    journeyErr = ''
    navigate(`/journeys/${encodeURIComponent(s)}/${encodeURIComponent(d)}`)
  }

  function submitPnr(e) {
    e?.preventDefault?.()
    const p = String(pnrQuery ?? '').trim()
    if (!/^\d{10}$/.test(p)) {
      pnrErr = 'Enter a valid 10-digit PNR.'
      return
    }
    pnrErr = ''
    navigate(`/pnr/${p}`)
  }

  const LETTERS = ['All', ...Array.from({ length: 26 }, (_, i) => String.fromCharCode(65 + i)), '#']
  const ALL_CAP = 24
  const DISPLAY_CAP = 48

  let stationIndex = $state([])
  let idxLoading = $state(true)
  let idxError = $state('')
  let query = $state('')
  let activeLetter = $state('All')
  let reqSeq = 0
  let debounceId = null

  function startsWithLetter(text, letter) {
    const c = (text || '').charAt(0).toUpperCase()
    if (letter === '#') return c !== '' && !/[A-Z]/.test(c)
    return c === letter
  }

  function mergeStations(list) {
    if (!Array.isArray(list)) return
    const seen = new Set(stationIndex.map((s) => s.code))
    const fresh = list.filter((s) => s && s.code && !seen.has(s.code))
    if (fresh.length === 0) return
    stationIndex = [...stationIndex, ...fresh].sort((a, b) =>
      (a.name || '').localeCompare(b.name || '')
    )
  }

  async function fetchStations(qs, seq) {
    const res = await api(`/rail-api/stations?q=${encodeURIComponent(qs)}`)
    if (seq !== reqSeq) return
    if (!res.ok) {
      idxError = res.error || 'station index unavailable'
      return
    }
    idxError = ''
    mergeStations(res.data)
  }

  function runFetch(qs) {
    const seq = ++reqSeq
    idxLoading = true
    fetchStations(qs, seq).finally(() => {
      if (seq === reqSeq) idxLoading = false
    })
  }

  function scheduleSearch() {
    if (debounceId) clearTimeout(debounceId)
    debounceId = setTimeout(() => {
      debounceId = null
      const qs = query.trim()
      if (!qs) return
      runFetch(qs)
    }, 200)
  }

  function selectLetter(letter) {
    activeLetter = letter
    if (query.trim() || letter === 'All') return
    if (stationIndex.some((s) => startsWithLetter(s.name, letter) || startsWithLetter(s.code, letter))) return
    runFetch(letter)
  }

  $effect(() => {
    const seq = ++reqSeq
    idxLoading = true
    Promise.all(LETTERS.slice(1).map((letter) => fetchStations(letter, seq))).then(() => {
      if (seq === reqSeq) idxLoading = false
    })
    return () => {
      reqSeq++
      if (debounceId) clearTimeout(debounceId)
    }
  })

  const stationView = $derived.by(() => {
    const ql = query.trim().toLowerCase()
    let rows = stationIndex
    if (ql) {
      rows = rows.filter(
        (s) =>
          (s.name || '').toLowerCase().includes(ql) ||
          (s.city || '').toLowerCase().includes(ql) ||
          (s.code || '').toLowerCase().includes(ql)
      )
    } else if (activeLetter !== 'All') {
      rows = rows.filter(
        (s) => startsWithLetter(s.name, activeLetter) || startsWithLetter(s.code, activeLetter)
      )
    }
    const cap = ql || activeLetter !== 'All' ? DISPLAY_CAP : ALL_CAP
    return { rows: rows.slice(0, cap), hidden: Math.max(0, rows.length - cap), matched: rows.length }
  })
</script>

<section class="grid gap-8">
  <div class="grid gap-3">
    <h1 class="text-4xl font-semibold tracking-tight">Train Bro</h1>
    <p class="max-w-xl text-muted-foreground">
      Live train status, station boards, journey planning and PNR — free, no accounts.
    </p>

    <div class="flex flex-wrap items-center gap-2 pt-1">
      <span class="text-xs text-muted-foreground">Popular trains</span>
      {#each popularTrains as n (n)}
        <button
          type="button"
          class="inline-flex h-6 items-center rounded-full border px-2.5 font-mono text-xs transition-colors hover:bg-muted hover:text-foreground"
          onclick={() => navigate(`/train/${n}`)}
        >
          {n}
        </button>
      {/each}
    </div>
  </div>

  <Card.Root class="border-primary/30 transition-colors hover:border-primary/60">
    <form class="grid gap-4 p-6" onsubmit={submitPlan}>
      <div class="flex items-start gap-3">
        <span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
          <MapPinIcon class="size-5" />
        </span>
        <div class="grid gap-0.5">
          <h2 class="text-base font-semibold">Plan from where you are</h2>
          <p class="text-sm text-muted-foreground">
            Find stations around you, pick one as your start, then jump straight to trains and live seat availability.
          </p>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="outline"
          onclick={findNearby}
          disabled={nearPhase === 'locating' || nearPhase === 'loading'}
        >
          {#if nearPhase === 'locating' || nearPhase === 'loading'}
            <LocateFixedIcon />
          {:else}
            <MapPinIcon />
          {/if}
          {nearPhase === 'locating'
            ? 'Locating…'
            : nearPhase === 'loading'
              ? 'Searching…'
              : nearbyStations.length
                ? 'Refresh nearby'
                : 'Stations near me'}
        </Button>
        {#if originCode && nearbyStations.some((s) => s.code === originCode)}
          {@const picked = nearbyStations.find((s) => s.code === originCode)}
          <span
            class="inline-flex items-center gap-1.5 rounded-full border border-primary/40 bg-primary/10 px-2.5 py-1 text-xs font-medium"
          >
            From
            <StationCodeBadge code={picked.code} name={picked.name} link={false} size="xs" />
            <span class="max-w-40 truncate">{picked.name}</span>
            <button type="button" aria-label="Clear chosen station" onclick={clearOrigin}>
              <XIcon class="size-3 opacity-70 hover:opacity-100" />
            </button>
          </span>
        {/if}
      </div>

      {#if nearPhase === 'msg'}
        <p class="text-xs text-muted-foreground" role="status">{nearMsg}</p>
      {:else if nearPhase === 'loading'}
        <div class="flex flex-wrap gap-1.5" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-8 w-44" />
          {/each}
        </div>
      {:else if nearPhase === 'ok'}
        <div class="grid gap-1">
          <p class="text-xs text-muted-foreground">
            {nearbyStations.length} station{nearbyStations.length === 1 ? '' : 's'} around you, closest first — pick one to start from:
          </p>
          <div class="flex flex-wrap gap-1.5" role="group" aria-label="Choose your starting station">
            {#each nearbyStations as s (s.code)}
              {@const active = s.code === originCode}
              <button
                type="button"
                aria-pressed={active}
                onclick={() => chooseOrigin(s.code)}
                class={`inline-flex max-w-full cursor-pointer items-center gap-2 rounded-lg border px-2.5 py-1.5 text-left text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
                  active
                    ? 'border-primary bg-primary text-primary-foreground shadow-sm'
                    : 'hover:border-primary/50 hover:bg-muted/50'
                }`}
              >
                <span class="font-mono font-semibold tabular-nums">{s.code}</span>
                <span class="min-w-0 max-w-36 truncate font-medium">{s.name}</span>
                {#if nearbyDistance(s.distance_km)}
                  <span class={`tabular-nums ${active ? 'opacity-80' : 'text-muted-foreground'}`}>
                    {nearbyDistance(s.distance_km)}
                  </span>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <div class="flex flex-wrap items-end gap-2">
        <div class="grid min-w-44 flex-1 gap-1.5">
          <Label for="home-plan-to">Destination</Label>
          <AutoCompleteInput
            id="home-plan-to"
            kind="station"
            placeholder="Where to? e.g. PUNE"
            bind:value={destQuery}
            onpick={onDestPick}
          />
        </div>
        <div class="grid gap-1.5">
          <Label for="home-plan-date">Journey date</Label>
          <Input id="home-plan-date" type="date" bind:value={journeyDate} min={TODAY} class="w-40" />
        </div>
        <Button type="submit" disabled={!canPlan}>
          <SearchIcon data-icon="inline-start" />
          Find trains &amp; availability
        </Button>
      </div>
      {#if planError}
        <p class="text-xs text-destructive">{planError}</p>
      {/if}
    </form>
  </Card.Root>

  <div class="grid gap-4 sm:grid-cols-2">
    <Card.Root class="transition-colors hover:border-primary/50">
      <form class="flex h-full flex-col gap-3 p-6" onsubmit={submitTrain}>
        <div class="flex items-start gap-3">
          <span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <TrainFront class="size-5" />
          </span>
          <div class="grid gap-0.5">
            <a
              href="/train"
              class="text-base font-semibold hover:underline"
              onclick={(e) => { e.preventDefault(); navigate('/train') }}
            >Live train status</a>
            <p class="text-sm text-muted-foreground">Where is my train right now, delay per station.</p>
          </div>
        </div>
        <div class="mt-auto flex items-end gap-2">
          <div class="grid flex-1 gap-1.5">
            <Label for="home-train">Train number or name</Label>
            <AutoCompleteInput
              id="home-train"
              kind="train"
              placeholder="Train number or name…"
              bind:value={trainQuery}
              onpick={(item) => {
                trainQuery = String(item.number)
                goTrain(String(item.number))
              }}
              inputClass={trainErr ? 'border-destructive' : ''}
            />
          </div>
          <Button type="submit" disabled={trainResolving}>
            <SearchIcon data-icon="inline-start" />
            {trainResolving ? '…' : 'Track'}
          </Button>
        </div>
        {#if trainErr}
          <p class="text-xs text-destructive">{trainErr}</p>
        {/if}
      </form>
    </Card.Root>

    <Card.Root class="transition-colors hover:border-primary/50">
      <form class="flex h-full flex-col gap-3 p-6" onsubmit={submitStation}>
        <div class="flex items-start gap-3">
          <span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Building2 class="size-5" />
          </span>
          <div class="grid gap-0.5">
            <a
              href="/station"
              class="text-base font-semibold hover:underline"
              onclick={(e) => { e.preventDefault(); navigate('/station') }}
            >Station boards</a>
            <p class="text-sm text-muted-foreground">Arrivals and departures for the next hours.</p>
          </div>
        </div>
        <div class="mt-auto flex items-end gap-2">
          <div class="grid flex-1 gap-1.5">
            <Label for="home-station">Station</Label>
            <AutoCompleteInput
              id="home-station"
              kind="station"
              placeholder="Station name or code, e.g. NDLS"
              bind:value={stationQuery}
              onpick={submitStation}
            />
          </div>
          <Button type="submit">
            <SearchIcon data-icon="inline-start" />
            Board
          </Button>
        </div>
        {#if stationErr}
          <p class="text-xs text-destructive">{stationErr}</p>
        {/if}
      </form>
    </Card.Root>

    <Card.Root class="transition-colors hover:border-primary/50 sm:col-span-2">
      <form class="flex h-full flex-col gap-3 p-6" onsubmit={submitJourney}>
        <div class="flex items-start gap-3">
          <span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <RouteIcon class="size-5" />
          </span>
          <div class="grid gap-0.5">
            <a
              href="/journeys"
              class="text-base font-semibold hover:underline"
              onclick={(e) => { e.preventDefault(); navigate('/journeys') }}
            >Find journeys</a>
            <p class="text-sm text-muted-foreground">Trains between any two stations, with run days.</p>
          </div>
        </div>
        <div class="flex flex-wrap items-end gap-2">
          <div class="grid min-w-44 flex-1 gap-1.5">
            <Label for="home-journey-from">From</Label>
            <AutoCompleteInput id="home-journey-from" kind="station" placeholder="NDLS" bind:value={journeyFrom} />
          </div>
          <div class="grid min-w-44 flex-1 gap-1.5">
            <Label for="home-journey-to">To</Label>
            <AutoCompleteInput
              id="home-journey-to"
              kind="station"
              placeholder="DLI"
              bind:value={journeyTo}
              onpick={() => { if (norm(journeyFrom)) submitJourney() }}
            />
          </div>
          <Button type="submit" disabled={!journeyFrom.trim() || !journeyTo.trim()}>
            <SearchIcon data-icon="inline-start" />
            Search
          </Button>
        </div>
        {#if journeyErr}
          <p class="text-xs text-destructive">{journeyErr}</p>
        {/if}
      </form>
    </Card.Root>

    <Card.Root class="transition-colors hover:border-primary/50 sm:col-span-2">
      <form class="flex h-full flex-col gap-3 p-6 sm:flex-row sm:gap-6" onsubmit={submitPnr}>
        <div class="flex shrink-0 items-start gap-3">
          <span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Ticket class="size-5" />
          </span>
          <div class="grid gap-0.5">
            <a
              href="/pnr"
              class="text-base font-semibold hover:underline"
              onclick={(e) => { e.preventDefault(); navigate('/pnr') }}
            >PNR status</a>
            <p class="text-sm text-muted-foreground">Passenger reservation status with captcha retry.</p>
          </div>
        </div>
        <div class="grid flex-1 content-center gap-1.5">
          <Label for="home-pnr">PNR number</Label>
          <div class="flex items-end gap-2">
            <Input
              id="home-pnr"
              bind:value={pnrQuery}
              inputmode="numeric"
              placeholder="10-digit PNR"
              maxlength={10}
              aria-invalid={pnrErr ? 'true' : undefined}
            />
            <Button type="submit">
              <SearchIcon data-icon="inline-start" />
              Check status
            </Button>
          </div>
          {#if pnrErr}
            <p class="text-xs text-destructive">{pnrErr}</p>
          {/if}
        </div>
      </form>
    </Card.Root>
  </div>

  <Separator />

  <section class="grid gap-3">
    <div class="flex flex-wrap items-end justify-between gap-2">
      <div class="grid gap-1">
        <h2 class="text-xl font-semibold tracking-tight">Stations explorer</h2>
        <p class="text-sm text-muted-foreground">
          Browse the station index by letter or search a partial name, city or code.
        </p>
      </div>
      <span class="text-xs text-muted-foreground tabular-nums">{stationView.matched} matched</span>
    </div>

    <div class="flex flex-wrap gap-1">
      {#each LETTERS as letter (letter)}
        <button
          type="button"
          aria-pressed={activeLetter === letter}
          class="inline-flex h-7 min-w-7 items-center justify-center rounded-md border px-1.5 text-xs font-medium transition-colors {activeLetter === letter
            ? 'border-primary bg-primary text-primary-foreground'
            : 'hover:bg-muted hover:text-foreground'}"
          onclick={() => selectLetter(letter)}
        >
          {letter}
        </button>
      {/each}
    </div>

    <div class="relative max-w-sm">
      <SearchIcon
        class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
      />
      <Input
        bind:value={query}
        oninput={scheduleSearch}
        placeholder="Search name, city or code…"
        class="pl-8"
      />
    </div>

    {#if idxLoading}
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
        {#each Array(12) as _, i (i)}
          <Skeleton class="h-11 w-full" />
        {/each}
      </div>
    {:else if stationView.rows.length === 0}
      {#if idxError && stationIndex.length === 0}
        <p class="text-sm text-destructive">Station index unavailable — {idxError}</p>
      {:else}
        <p class="text-sm text-muted-foreground">No stations matched</p>
      {/if}
    {:else}
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
        {#each stationView.rows as st (st.code)}
          <button
            type="button"
            class="flex min-w-0 items-center justify-between gap-2 rounded-lg border px-3 py-2 text-left transition-colors hover:border-primary/50 hover:bg-muted/50"
            onclick={() => navigate(`/station/${encodeURIComponent(st.code)}`)}
          >
            <span class="min-w-0 flex-1 truncate text-sm">{st.name}</span>
            <StationCodeBadge code={st.code} name={st.name} link={false} size="xs" />
          </button>
        {/each}
      </div>
      {#if stationView.hidden > 0}
        <p class="text-xs text-muted-foreground">+{stationView.hidden} more</p>
      {/if}
    {/if}
  </section>
</section>
