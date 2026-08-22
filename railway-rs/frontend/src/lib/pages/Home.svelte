<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Separator } from '$lib/components/ui/separator/index.js'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import RouteIcon from 'lucide-svelte/icons/route'
  import Ticket from 'lucide-svelte/icons/ticket'
  import SearchIcon from 'lucide-svelte/icons/search'

  let state = $state({ phase: 'loading', data: null })
  let obs = $state(null)

  $effect(() => {
    api('/rail-api/source-status').then((res) => {
      state = res.ok ? { phase: 'ok', data: res.data } : { phase: 'error', data: null }
    })
    api('/rail-api/observability').then((res) => {
      if (res.ok && res.data && typeof res.data === 'object') {
        obs = res.data
      }
    })
  })

  function fmtUptime(secs) {
    const n = Number(secs)
    if (!Number.isFinite(n) || n <= 0) return null
    const h = Math.floor(n / 3600)
    const m = Math.floor((n % 3600) / 60)
    return h > 0 ? `${h}h ${m}m` : `${m}m`
  }

  const uptimeLabel = $derived(fmtUptime(obs?.uptime_secs))
  const showObs = $derived(
    Boolean(obs && (obs?.requests_total != null || uptimeLabel || obs?.active_connections != null))
  )

  const popularTrains = [12951, 12309, 12002]

  const features = [
    { href: '/train', icon: TrainFront, title: 'Live train status', desc: 'Where is my train right now, delay per station.' },
    { href: '/station', icon: Building2, title: 'Station boards', desc: 'Arrivals and departures for the next hours.' },
    { href: '/journeys', icon: RouteIcon, title: 'Find journeys', desc: 'Trains between any two stations, with run days.' },
    { href: '/pnr', icon: Ticket, title: 'PNR status', desc: 'Passenger reservation status with captcha retry.' }
  ]

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
      Live Indian Railways data served by a Rust backend — honest sources, no accounts,
      no fabricated numbers.
    </p>
    <div class="flex flex-wrap items-center gap-2 pt-1">
      {#if state.phase === 'loading'}
        <Skeleton class="h-6 w-40" />
        <Skeleton class="h-6 w-24" />
      {:else if state.phase === 'error'}
        <Badge variant="destructive">source status unavailable</Badge>
      {:else}
        <Badge variant={state.data.live_enabled ? 'default' : 'destructive'}>
          {state.data.live_enabled ? `live · ${state.data.mode}` : 'live disabled'}
        </Badge>
        <Badge variant="secondary">cache {state.data.cache_ttl_seconds}s</Badge>
        <Badge variant="outline">
          {state.data.sources.filter((s) => s.reachable).length}/{state.data.sources.length} upstreams up
        </Badge>
      {/if}
    </div>

    {#if showObs}
      <div class="flex flex-wrap gap-2">
        <div class="rounded-md border bg-muted/40 px-3 py-1.5">
          <p class="text-[10px] uppercase tracking-wide text-muted-foreground">requests_total</p>
          <p class="text-sm font-medium tabular-nums">{obs?.requests_total ?? '—'}</p>
        </div>
        <div class="rounded-md border bg-muted/40 px-3 py-1.5">
          <p class="text-[10px] uppercase tracking-wide text-muted-foreground">uptime</p>
          <p class="text-sm font-medium tabular-nums">{uptimeLabel ?? '—'}</p>
        </div>
        <div class="rounded-md border bg-muted/40 px-3 py-1.5">
          <p class="text-[10px] uppercase tracking-wide text-muted-foreground">active connections</p>
          <p class="text-sm font-medium tabular-nums">{obs?.active_connections ?? '—'}</p>
        </div>
      </div>
    {/if}

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

  <div class="grid gap-4 sm:grid-cols-2">
    {#each features as f (f.href)}
      <Card.Root class="transition-colors hover:border-primary/50">
        <a
          href={f.href}
          class="flex h-full flex-col gap-3 p-6"
          onclick={(e) => {
            e.preventDefault()
            navigate(f.href)
          }}
        >
          <span class="flex size-10 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <f.icon class="size-5" />
          </span>
          <Card.Title class="text-base">{f.title}</Card.Title>
          <Card.Description>{f.desc}</Card.Description>
        </a>
      </Card.Root>
    {/each}
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
            <Badge variant="outline" class="shrink-0 font-mono text-[10px]">{st.code}</Badge>
          </button>
        {/each}
      </div>
      {#if stationView.hidden > 0}
        <p class="text-xs text-muted-foreground">+{stationView.hidden} more</p>
      {/if}
    {/if}
  </section>

  {#if state.phase === 'error'}
    <Alert.Root variant="destructive">
      <Alert.Title>Backend unreachable</Alert.Title>
      <Alert.Description>Could not load source status from /rail-api/source-status.</Alert.Description>
    </Alert.Root>
  {/if}
</section>
