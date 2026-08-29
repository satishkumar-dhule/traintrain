<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Separator } from '$lib/components/ui/separator/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import DateStrip from '$lib/components/DateStrip.svelte'
  import { pickNearbyStation } from '$lib/nearby.svelte.js'
  import { StationCodeBadge } from '$lib/components/badges/index.js'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import RouteIcon from 'lucide-svelte/icons/route'
  import Ticket from 'lucide-svelte/icons/ticket'
  import SearchIcon from 'lucide-svelte/icons/search'
  import MapPinIcon from 'lucide-svelte/icons/map-pin'
  import ArrowLeftRight from 'lucide-svelte/icons/arrow-left-right'
  import XIcon from 'lucide-svelte/icons/x'
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert'
import TrackRule from '$lib/components/TrackRule.svelte'
import BottomSpacer from '$lib/components/BottomSpacer.svelte'
import { norm, todayISO, DATE_RE } from '$lib/format.js'

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
  const TODAY = todayISO()

  let originCode = $state('')
  let originName = $state('')

  let destQuery = $state('')
  let journeyDate = $state(TODAY)
  let planError = $state('')

  const destCode = $derived(norm(destQuery))
  const canPlan = $derived(Boolean(originCode) && destCode.length > 0)

  const PLAN_LAST_KEY = 'rc-plan-last'

  function prefillFromLast() {
    let raw = null
    try {
      raw = JSON.parse(localStorage.getItem(PLAN_LAST_KEY) || 'null')
    } catch {
      return
    }
    const from = norm(raw?.from)
    const to = norm(raw?.to)
    if (!from || !to || from === to) return
    if (!originCode) {
      originCode = from
      originName = String(raw?.fromName || '')
    }
    if (!norm(destQuery)) destQuery = to
  }
  prefillFromLast()

  function swapPlan() {
    if (!canPlan) return
    const prevOrigin = norm(originCode)
    originCode = destCode
    originName = ''
    destQuery = prevOrigin
    planError = ''
  }

  async function pickOrigin() {
    const picked = await pickNearbyStation()
    if (!picked || !picked.code) return
    originCode = picked.code
    originName = picked.name || ''
    if (canPlan && DATE_RE.test(journeyDate)) submitPlan()
  }

  function clearOrigin() {
    originCode = ''
    originName = ''
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
    try {
      localStorage.setItem(
        PLAN_LAST_KEY,
        JSON.stringify({ from: norm(originCode), to: destCode, fromName: originName })
      )
    } catch {}
    navigate(
      `/availability/${encodeURIComponent(originCode)}/${encodeURIComponent(destCode)}/${encodeURIComponent(journeyDate)}`
    )
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
  let idxLoading = $state(false)
  let idxError = $state('')
  let query = $state('')
  let activeLetter = $state('All')
  let explorerOpen = $state(false)
  let explorerLoaded = false
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

  function loadStationSweep() {
    const seq = ++reqSeq
    idxLoading = true
    Promise.all(LETTERS.slice(1).map((letter) => fetchStations(letter, seq))).then(() => {
      if (seq === reqSeq) idxLoading = false
    })
  }

  function expandExplorer() {
    explorerOpen = true
    if (explorerLoaded) return
    explorerLoaded = true
    loadStationSweep()
  }

  $effect(() => {
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

<section class="grid gap-4 max-lg:gap-3">
  <div class="grid gap-2.5 max-lg:gap-2">
    <p class="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
      <span class="rounded-sm bg-saffron px-1.5 py-px text-xs font-bold tracking-[0.12em] text-saffron-ink">
        Indian Railways
      </span>
      <span class="lamp-pulse size-1.5 shrink-0 rounded-full bg-signal-go text-signal-go" aria-hidden="true"></span>
      Live &amp; free
    </p>
    <h1 class="signage text-4xl leading-[0.95] sm:text-5xl lg:text-6xl">Train Bro</h1>
    <p class="max-lg:hidden text-sm text-muted-foreground">
      Live status, PNR, journeys & station boards — free, no accounts.
    </p>

    <div class="flex items-center gap-2 pt-1 max-lg:flex-nowrap max-lg:overflow-x-auto max-lg:snap-x">
      <span class="max-lg:hidden shrink-0 text-[0.7rem] font-medium uppercase tracking-[0.14em] text-muted-foreground">
        Popular trains
      </span>
      {#each popularTrains as n (n)}
        <button
          type="button"
          class="inline-flex h-8 shrink-0 snap-start items-center gap-1.5 rounded-full border bg-card px-3 transition-colors hover:border-primary/50 hover:bg-muted"
          onclick={() => navigate(`/train/${n}`)}
        >
          <TrainFront class="size-3.5 text-primary" aria-hidden="true" />
          <span class="data-num text-xs font-semibold">{n}</span>
        </button>
      {/each}
    </div>
  </div>

  <Card.Root class="border-primary/30 transition-colors hover:border-primary/60">
    <form class="grid grid-cols-[minmax(0,1fr)] gap-4 max-lg:gap-2.5 p-5 max-lg:p-3.5" onsubmit={submitPlan}>
      <div class="flex items-start gap-2 max-lg:gap-1.5">
        <span class="flex size-9 max-lg:size-7 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
          <MapPinIcon class="size-5 max-lg:size-3.5" />
        </span>
        <div class="grid gap-0.5">
          <h2 class="text-base font-semibold leading-none sm:text-lg">Plan from where you are</h2>
          <p class="max-lg:hidden pt-1 text-xs text-muted-foreground">
            Pick a start station, then choose a destination and journey date.
          </p>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-0.5 max-lg:gap-0.5">
        <Button type="button" variant="outline" onclick={pickOrigin} class="max-lg:h-8 max-lg:text-xs">
          <MapPinIcon class="max-lg:size-3" />
          {originCode ? 'Change start' : 'Stations near me'}
        </Button>
        {#if originCode}
          <span
            class="inline-flex items-center gap-0.5 rounded-full border border-primary/40 bg-primary/10 px-1.5 py-0.5 text-xs font-medium"
          >
            From
            <StationCodeBadge code={originCode} name={originName} link={false} size="xs" />
            <span class="max-w-20 truncate">{originName || originCode}</span>
            <button
              type="button"
              aria-label="Clear chosen station"
              class="inline-flex items-center justify-center max-lg:size-7"
              onclick={clearOrigin}
            >
              <XIcon class="size-2 opacity-70 hover:opacity-100" />
            </button>
          </span>
        {/if}
        {#if canPlan}
          <Button type="button" variant="outline" onclick={swapPlan} class="max-lg:h-8 max-lg:text-xs">
            <ArrowLeftRight class="max-lg:size-3" />
            Swap
          </Button>
        {/if}
      </div>

        <div class="flex flex-wrap items-end gap-2.5 max-lg:gap-2">
          <div class="grid min-w-32 sm:min-w-44 flex-1">
            <AutoCompleteInput
              id="home-plan-to"
              kind="station"
              placeholder="Where to? e.g. PUNE"
              aria-label="Destination"
              bind:value={destQuery}
              onpick={onDestPick}
              inputClass="h-12 max-lg:h-12"
            />
          </div>
          <Button
            type="submit"
            disabled={!canPlan}
            class="min-h-12 shrink-0 px-5 max-lg:min-h-11 max-lg:w-full sm:w-auto"
          >
            <SearchIcon data-icon="inline-start" class="size-4" />
            Find trains &amp; availability
          </Button>
        </div>
        <DateStrip id="home-plan-date" bind:value={journeyDate} min={TODAY} label="Journey date" />
      {#if planError}
        <p class="text-xs text-destructive max-lg:text-sm">{planError}</p>
      {/if}
    </form>
  </Card.Root>

  <TrackRule label="Quick access" />

  <section class="grid gap-2 max-lg:gap-1.5" aria-label="Quick access">
    <div class="flex flex-wrap items-center gap-2 max-lg:gap-1.5">
      <button
        type="button"
        class="inline-flex h-10 items-center gap-2 rounded-full border bg-card px-4 text-xs font-semibold uppercase tracking-wide transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-muted/60 hover:shadow-md"
        onclick={() => navigate('/train')}
      >
        <TrainFront class="size-4 shrink-0 text-primary" />
        Live Train
      </button>
      <button
        type="button"
        class="inline-flex h-10 items-center gap-2 rounded-full border bg-card px-4 text-xs font-semibold uppercase tracking-wide transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-muted/60 hover:shadow-md"
        onclick={() => navigate('/pnr')}
      >
        <Ticket class="size-4 shrink-0 text-primary" />
        PNR Status
      </button>
      <button
        type="button"
        class="inline-flex h-10 items-center gap-2 rounded-full border bg-card px-4 text-xs font-semibold uppercase tracking-wide transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-muted/60 hover:shadow-md"
        onclick={() => navigate('/station')}
      >
        <Building2 class="size-4 shrink-0 text-primary" />
        Station Board
      </button>
      <button
        type="button"
        class="inline-flex h-10 items-center gap-2 rounded-full border bg-card px-4 text-xs font-semibold uppercase tracking-wide transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:bg-muted/60 hover:shadow-md"
        onclick={() => navigate('/exceptions')}
      >
        <TriangleAlert class="size-4 shrink-0 text-primary" />
        Service Alerts
      </button>
    </div>
  </section>

  <Separator />

  <section class="grid gap-2 max-lg:gap-1.5">
    <div class="flex flex-wrap items-end justify-between gap-2">
      <div class="grid gap-0.5">
        <h2 class="text-lg sm:text-xl font-semibold tracking-tight">Stations explorer</h2>
        <p class="max-lg:hidden text-sm text-muted-foreground">
          Browse the station index by letter or search a partial name, city or code.
        </p>
      </div>
      {#if explorerOpen}
        <span class="data-num text-xs text-muted-foreground">{stationView.matched} matched</span>
      {/if}
    </div>

    {#if !explorerOpen}
      <Card.Root class="transition-colors hover:border-primary/50">
        <button
          type="button"
          class="flex w-full items-start gap-3 p-4 max-lg:p-3 text-left"
          onclick={expandExplorer}
        >
          <span class="flex size-9 max-lg:size-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <Building2 class="size-5 max-lg:size-4" />
          </span>
          <div class="grid gap-0.5">
            <span class="text-sm sm:text-base font-semibold">Browse stations A–Z</span>
            <span class="max-lg:hidden text-sm text-muted-foreground">
              Opens the letter index and station search — loads on demand.
            </span>
          </div>
        </button>
      </Card.Root>
    {:else}
      <div class="flex flex-wrap gap-0.5 max-lg:gap-1">
      {#each LETTERS as letter (letter)}
        <button
          type="button"
          aria-pressed={activeLetter === letter}
          class="inline-flex h-6 min-w-6 items-center justify-center rounded-md border px-1 text-xs font-medium transition-colors max-lg:h-11 max-lg:min-w-10 {activeLetter === letter
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
            class="flex min-h-11 min-w-0 items-center justify-between gap-2 rounded-lg border bg-card px-3 py-2 text-left transition-colors hover:border-primary/50 hover:bg-muted/50"
            onclick={() => navigate(`/station/${encodeURIComponent(st.code)}`)}
          >
            <span class="min-w-0 flex-1 truncate text-sm">{st.name}</span>
            <StationCodeBadge code={st.code} name={st.name} link={false} size="xs" />
          </button>
        {/each}
      </div>
      {#if stationView.hidden > 0}
        <p class="text-xs text-muted-foreground"><span class="data-num">+{stationView.hidden}</span> more</p>
      {/if}
    {/if}
    {/if}
  </section>
  <BottomSpacer />
</section>
