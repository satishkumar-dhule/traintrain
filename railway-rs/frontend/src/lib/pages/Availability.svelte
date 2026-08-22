<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import EmptyState from '$lib/components/EmptyState.svelte'
  import RecentSearches from '$lib/components/RecentSearches.svelte'
  import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'
  import {
    TrainNumberBadge,
    RunsOnBadges,
    AvailabilityStatusBadge,
    DataSourceBadge,
    availabilityStatusKind,
    dayFlags
  } from '$lib/components/badges/index.js'
import ArrowDownUpIcon from 'lucide-svelte/icons/arrow-down-up'
import CalendarDaysIcon from 'lucide-svelte/icons/calendar-days'
import ChevronLeftIcon from 'lucide-svelte/icons/chevron-left'
import ChevronRightIcon from 'lucide-svelte/icons/chevron-right'

  let { src = '', dst = '', date = '' } = $props()

  const DATE_RE = /^\d{4}-\d{2}-\d{2}$/

  let from = $state('')
  let to = $state('')
  let journeyDate = $state(today())

  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let committed = null

  const TODAY = today()
  const MAX_DAY = 364
  const STRIP_WINDOW = 15
  const maxDate = isoShift(TODAY, MAX_DAY)

  function diffDays(a, b) {
    return Math.round((new Date(`${b}T00:00:00`) - new Date(`${a}T00:00:00`)) / 86400000)
  }
  function clampDate(iso) {
    if (!DATE_RE.test(iso)) return TODAY
    return iso < TODAY ? TODAY : iso > maxDate ? maxDate : iso
  }

  // Windowed date strip: ~STRIP_WINDOW day-buttons centered on the selection.
  // Bounded DOM width keeps the grid track from being blown out by min-content.
  const stripStart = $derived.by(() => {
    const sel = DATE_RE.test(asText(journeyDate)) ? journeyDate : TODAY
    const idx = Math.max(0, diffDays(TODAY, sel))
    return Math.max(0, Math.min(idx - Math.floor(STRIP_WINDOW / 2), MAX_DAY + 1 - STRIP_WINDOW))
  })
  const stripDates = $derived(
    Array.from(
      { length: Math.min(STRIP_WINDOW, MAX_DAY + 1) },
      (_, i) => isoShift(TODAY, stripStart + i),
    ),
  )

  function stepDay(days) {
    const next = clampDate(isoShift(DATE_RE.test(asText(journeyDate)) ? journeyDate : TODAY, days))
    if (next === journeyDate) return
    pickDate(next)
  }

  const RECENT_KEY = 'rc-availability-recent'
  const PREFS_KEY = 'rc-availability-prefs'
  const recentValid = (r) => r && typeof r?.id === 'string' && DATE_RE.test(String(r?.date ?? ''))
  let recent = $state(loadRecent(RECENT_KEY, recentValid))

  const SORTS = [
    ['departure', 'Departure'],
    ['duration', 'Duration'],
    ['fare', 'Lowest fare'],
    ['chance', 'Confirm chance']
  ]
  const CLASS_ORDER = ['1A', 'EA', 'EC', '2A', '3A', '3E', 'FC', 'CC', 'SL', '2S', 'UR']

  function loadPrefs() {
    try {
      const p = JSON.parse(localStorage.getItem(PREFS_KEY) || '{}')
      return p && typeof p === 'object' ? p : {}
    } catch {
      return {}
    }
  }
  const prefs = loadPrefs()
  let view = $state(prefs.view === 'matrix' ? 'matrix' : 'cards')
  let sortKey = $state(SORTS.some(([k]) => k === prefs.sortKey) ? prefs.sortKey : 'departure')
  let availableOnly = $state(prefs.availableOnly === true)
  let hiddenClasses = $state([])

  $effect(() => {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify({ view, sortKey, availableOnly }))
    } catch {}
  })

  function rememberRoute(s, d, dt) {
    recent = rememberRecent(
      RECENT_KEY,
      { id: `${s}|${d}`, label: `${s} → ${d}`, date: dt },
      recentValid,
    )
  }

  function pickRecent(r) {
    const [s, d] = String(r?.id ?? '').split('|')
    const dt = String(r?.date ?? '')
    if (!s || !d || !DATE_RE.test(dt)) return
    from = s
    to = d
    journeyDate = dt
    runSearch(s, d, dt, `${s}/${d}/${dt}`)
  }

  function today() {
    const d = new Date()
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  }

  function isoShift(iso, days) {
    const d = new Date(`${iso}T00:00:00`)
    d.setDate(d.getDate() + days)
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  }

  function weekdayShort(iso) {
    const d = new Date(`${iso}T00:00:00`)
    return isNaN(d.getTime()) ? '' : d.toLocaleDateString('en-IN', { weekday: 'short' })
  }

  function monthShort(iso) {
    const d = new Date(`${iso}T00:00:00`)
    return isNaN(d.getTime()) ? '' : d.toLocaleDateString('en-IN', { month: 'short' })
  }

  function pickDate(iso) {
    if (!DATE_RE.test(iso)) return
    journeyDate = iso
    if (asText(from) && asText(to)) search()
  }

  function asText(v) {
    return String(v ?? '').trim()
  }

  function fmt(v) {
    const t = asText(v)
    return t && t !== '-' && t !== '--' ? t : '—'
  }

  function numOrNull(v) {
    if (v == null || String(v).trim() === '') return null
    const n = Number(v)
    return Number.isFinite(n) ? n : null
  }

  function hmMin(v) {
    const m = /^(\d{1,3}):(\d{2})/.exec(asText(v))
    return m ? +m[1] * 60 + +m[2] : null
  }

  function durationMin(tr) {
    const d = hmMin(tr?.duration)
    if (d != null) return d
    const dep = hmMin(tr?.departure_time)
    const arr = hmMin(tr?.arrival_time)
    if (dep == null || arr == null) return null
    return arr >= dep ? arr - dep : arr + 1440 - dep
  }

  function durationLabel(tr) {
    const m = durationMin(tr)
    if (m == null) return fmt(tr?.duration)
    const h = Math.floor(m / 60)
    const mm = m % 60
    return h ? `${h}h ${mm ? mm + 'm' : ''}`.trim() : `${mm}m`
  }

  const canSearch = $derived(
    Boolean(asText(from)) && Boolean(asText(to)) && DATE_RE.test(asText(journeyDate))
  )
  const trains = $derived(Array.isArray(data?.trains) ? data.trains : [])
  const notice = $derived(asText(data?.notice))

  const rowsOf = (tr) => (Array.isArray(tr?.availability) ? tr.availability : [])
  const classCode = (row) => asText(row?.class).toUpperCase()

  const allClasses = $derived.by(() => {
    const seen = []
    for (const t of trains) {
      for (const r of rowsOf(t)) {
        const c = classCode(r)
        if (c && !seen.includes(c)) seen.push(c)
      }
    }
    return seen.sort(
      (a, b) =>
        (CLASS_ORDER.indexOf(a) === -1 ? 99 : CLASS_ORDER.indexOf(a)) -
          (CLASS_ORDER.indexOf(b) === -1 ? 99 : CLASS_ORDER.indexOf(b)) ||
        a.localeCompare(b),
    )
  })

  const visibleRows = (tr) => rowsOf(tr).filter((r) => !hiddenClasses.includes(classCode(r)))
  const hasAvailable = (tr) => visibleRows(tr).some((r) => availabilityStatusKind(r?.status) === 'available')

  const matrixClasses = $derived(allClasses.filter((c) => !hiddenClasses.includes(c)))

  const filteredTrains = $derived.by(() => {
    let list = trains.filter((t) => visibleRows(t).length > 0)
    if (availableOnly) list = list.filter(hasAvailable)
    const dir = sortKey === 'chance' ? -1 : 1
    return list.slice().sort((a, b) => {
      let va, vb
      if (sortKey === 'departure') {
        va = hmMin(a?.departure_time)
        vb = hmMin(b?.departure_time)
      } else if (sortKey === 'duration') {
        va = durationMin(a)
        vb = durationMin(b)
      } else if (sortKey === 'fare') {
        const fa = visibleRows(a).map((r) => numOrNull(r?.fare)).filter((n) => n != null)
        const fb = visibleRows(b).map((r) => numOrNull(r?.fare)).filter((n) => n != null)
        va = fa.length ? Math.min(...fa) : null
        vb = fb.length ? Math.min(...fb) : null
      } else {
        const pa = visibleRows(a).map((r) => numOrNull(r?.prediction)).filter((n) => n != null)
        const pb = visibleRows(b).map((r) => numOrNull(r?.prediction)).filter((n) => n != null)
        va = pa.length ? Math.max(...pa) : null
        vb = pb.length ? Math.max(...pb) : null
      }
      if (va == null && vb == null) return 0
      if (va == null) return 1
      if (vb == null) return -1
      return (va - vb) * dir
    })
  })

  const stats = $derived.by(() => {
    let totalRows = 0
    let availRows = 0
    let cheapest = null
    let bestChance = null
    for (const t of trains) {
      for (const r of rowsOf(t)) {
        totalRows++
        if (availabilityStatusKind(r?.status) === 'available') {
          availRows++
          const f = numOrNull(r?.fare)
          if (f != null && (!cheapest || f < cheapest.fare))
            cheapest = { fare: f, cls: classCode(r), number: asText(t?.number) }
        }
        const p = numOrNull(r?.prediction)
        if (p != null && (!bestChance || p > bestChance.pct))
          bestChance = { pct: Math.round(p), cls: classCode(r), number: asText(t?.number) }
      }
    }
    return { totalRows, availRows, cheapest, bestChance }
  })

  const filtersActive = $derived(availableOnly || hiddenClasses.length > 0)

  function resetFilters() {
    availableOnly = false
    hiddenClasses = []
    sortKey = 'departure'
  }

  function toggleClass(c) {
    hiddenClasses = hiddenClasses.includes(c)
      ? hiddenClasses.filter((x) => x !== c)
      : [...hiddenClasses, c]
  }

  async function runSearch(s, d, dt, key) {
    committed = key
    phase = 'loading'
    errorMsg = null
    const res = await api(
      `/rail-api/availability?src=${encodeURIComponent(s)}&dst=${encodeURIComponent(d)}&date=${encodeURIComponent(dt)}`
    )
    if (committed !== key) return
    if (res.ok) {
      data = res.data
      phase = 'ok'
      rememberRoute(s, d, dt)
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  function search() {
    const s = asText(from).toUpperCase()
    const d = asText(to).toUpperCase()
    const dt = asText(journeyDate)
    if (!s || !d || !DATE_RE.test(dt)) return
    const want =
      '/availability/' +
      [s, d, dt].filter(Boolean).map((p) => encodeURIComponent(p)).join('/')
    if (route.path !== want) navigate(want)
    else runSearch(s, d, dt, `${s}/${d}/${dt}`)
  }

  function swap() {
    const t = from
    from = to
    to = t
  }

  $effect(() => {
    const s = asText(src).toUpperCase()
    const d = asText(dst).toUpperCase()
    const dt = asText(date)
    if (s) from = s
    if (d) to = d
    if (DATE_RE.test(dt)) journeyDate = dt
    const key = s && d && DATE_RE.test(dt) ? `${s}/${d}/${dt}` : null
    if (!key || key === committed) {
      if (!committed) phase = 'idle'
      return
    }
    runSearch(s, d, dt, key)
  })
</script>

{#snippet chanceBar(row)}
  {@const pct = numOrNull(row?.prediction)}
  {#if pct != null}
    {@const w = Math.min(100, Math.max(0, pct))}
    <div
      class="mt-1 h-0.5 w-full rounded-full bg-muted"
      title={`Confirmation chance ${Math.round(pct)}%`}
    >
      <div
        class={`h-full rounded-full ${pct >= 90 ? 'bg-emerald-500' : pct >= 60 ? 'bg-amber-500' : 'bg-red-500'}`}
        style={`width:${w}%`}
      ></div>
    </div>
    <div class="mt-0.5 text-right text-[9px] leading-none tabular-nums text-muted-foreground">
      {Math.round(pct)}% confirm
    </div>
  {/if}
{/snippet}

{#snippet avlChip(row)}
  {@const kind = availabilityStatusKind(row?.status)}
  {@const tone =
    kind === 'available'
      ? 'border-emerald-600/25 bg-emerald-500/10 text-emerald-700 dark:border-emerald-500/35 dark:bg-emerald-400/10 dark:text-emerald-400'
      : kind === 'rac'
        ? 'border-amber-600/30 bg-amber-500/10 text-amber-700 dark:border-amber-500/35 dark:bg-amber-400/10 dark:text-amber-400'
        : kind === 'waitlist' || kind === 'closed'
          ? 'border-red-600/30 bg-red-500/10 text-red-700 dark:border-red-500/35 dark:bg-red-400/10 dark:text-red-400'
          : 'border-border bg-muted/50 text-muted-foreground'}
  {@const fare = numOrNull(row?.fare)}
  <div class={`overflow-hidden rounded-md border px-2 py-1 ${tone}`}>
    <div class="flex items-baseline justify-between gap-2">
      <span class="font-mono text-[11px] font-semibold">{fmt(classCode(row))}</span>
      <span class="font-mono text-[11px] tabular-nums">{fare != null ? `₹${fare.toLocaleString('en-IN')}` : ''}</span>
    </div>
    <div class="flex min-w-0 items-center gap-1 text-[10px]">
      <span class="size-1.5 shrink-0 rounded-full bg-current opacity-80"></span>
      <span class="min-w-0 truncate font-medium" title={asText(row?.status)}>{asText(row?.status) || '—'}</span>
    </div>
    {@render chanceBar(row)}
  </div>
{/snippet}

<section class="grid grid-cols-[minmax(0,1fr)] gap-3">
  <div class="flex flex-wrap items-baseline gap-x-3 gap-y-0">
    <h1 class="text-xl font-semibold tracking-tight">Availability</h1>
    <p class="text-xs text-muted-foreground">
      Class-wise availability, fares and confirm chances across every train.
    </p>
  </div>

  <div
    class="flex flex-wrap items-center gap-2 rounded-lg border bg-card p-2"
    onkeydown={(e) => {
      if (e.key === 'Enter' && !e.defaultPrevented) search()
    }}
  >
    <AutoCompleteInput
      id="av-from"
      bind:value={from}
      kind="station"
      placeholder="From station…"
      class="min-w-44 flex-1"
      inputClass="h-8"
      onpick={(item) => {
        if (asText(item?.code)) from = asText(item.code).toUpperCase()
      }}
    />
    <Button
      type="button"
      variant="outline"
      size="icon"
      class="size-8 shrink-0"
      onclick={swap}
      aria-label="Swap From and To stations"
      title="Swap stations"
    >
      <ArrowDownUpIcon />
    </Button>
    <AutoCompleteInput
      id="av-to"
      bind:value={to}
      kind="station"
      placeholder="To station…"
      class="min-w-44 flex-1"
      inputClass="h-8"
      onpick={(item) => {
        if (asText(item?.code)) to = asText(item.code).toUpperCase()
      }}
    />
    <Button type="button" class="ml-auto h-8" onclick={search} disabled={!canSearch}>
      Search
    </Button>
  </div>

  <div
    class="sticky top-14 z-20 flex items-center gap-1 rounded-lg border bg-card p-1 shadow-sm lg:top-0"
    role="group"
    aria-label="Journey date"
  >
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="size-7 shrink-0"
      onclick={() => stepDay(-1)}
      aria-label="Previous day"
      title="Previous day"
    >
      <ChevronLeftIcon />
    </Button>
    <div class="min-w-0 flex-1">
      <div
        class="flex items-center gap-1 overflow-x-auto px-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {#each stripDates as iso (iso)}
          {@const active = iso === journeyDate}
          <button
            type="button"
            data-active={active}
            aria-current={active ? 'date' : undefined}
            aria-label={`Journey on ${iso}`}
            onclick={() => pickDate(iso)}
            class={`flex w-[3.4rem] shrink-0 cursor-pointer flex-col items-center rounded-md border px-1 py-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
              active
                ? 'border-transparent bg-primary text-primary-foreground shadow-sm'
                : 'border-transparent hover:bg-muted'
            }`}
          >
            <span
              class={`text-[9px] font-medium uppercase tracking-wide ${active ? 'opacity-80' : 'text-muted-foreground'}`}
            >
              {iso === TODAY ? 'Today' : weekdayShort(iso)}
            </span>
            <span class="font-mono text-xs font-semibold tabular-nums">
              {iso.slice(8)}&thinsp;{monthShort(iso)}
            </span>
          </button>
        {/each}
      </div>
    </div>
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="size-7 shrink-0"
      onclick={() => stepDay(1)}
      aria-label="Next day"
      title="Next day"
    >
      <ChevronRightIcon />
    </Button>
    <Input
      id="av-date"
      type="date"
      bind:value={journeyDate}
      min={TODAY}
      max={maxDate}
      onchange={() => {
        if (canSearch) search()
      }}
      aria-label="Pick journey date (calendar)"
      title="Calendar"
      class="h-8 w-36 shrink-0"
    />
  </div>

  {#if phase === 'idle' && recent.length > 0}
    <RecentSearches
      items={recent}
      onpick={pickRecent}
      onclear={() => {
        clearStored(RECENT_KEY)
        recent = []
      }}
    />
  {/if}

  {#if phase === 'loading'}
    <div class="grid gap-2" aria-busy="true">
      <Skeleton class="h-10 w-full rounded-lg" />
      {#each [0, 1, 2, 3] as i (i)}
        <Card.Root>
          <Card.Content class="grid gap-2 py-2.5">
            <Skeleton class="h-4 w-72 max-w-full" />
            <div class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-1.5">
              <Skeleton class="h-12" />
              <Skeleton class="h-12" />
              <Skeleton class="h-12" />
              <Skeleton class="h-12" />
            </div>
          </Card.Content>
        </Card.Root>
      {/each}
    </div>
  {:else if phase === 'error'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not load availability</Alert.Title>
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {:else if phase === 'ok'}
    {#if trains.length === 0}
      <EmptyState
        icon={CalendarDaysIcon}
        title="No trains found"
        hint="No availability data returned for this route and date."
      />
    {:else}
      <div class="rounded-lg border bg-card px-3 py-1.5">
        <div
          class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs"
          role="status"
        >
          <span><span class="font-mono font-semibold tabular-nums">{trains.length}</span> <span class="text-muted-foreground">trains</span></span>
          <span class="inline-flex items-center gap-1">
            <span class="size-1.5 rounded-full {stats.availRows > 0 ? 'bg-emerald-500' : 'bg-red-500'}"></span>
            <span class="font-medium tabular-nums">{stats.availRows}</span>
            <span class="text-muted-foreground">classes open now</span>
          </span>
          {#if stats.cheapest}
            <span>
              <span class="text-muted-foreground">Cheapest</span>
              <span class="font-mono font-medium">₹{stats.cheapest.fare.toLocaleString('en-IN')}</span>
              <span class="text-muted-foreground">· {stats.cheapest.cls}{stats.cheapest.number ? ` · ${stats.cheapest.number}` : ''}</span>
            </span>
          {/if}
          {#if stats.bestChance}
            <span>
              <span class="text-muted-foreground">Best chance</span>
              <span class="font-mono font-medium tabular-nums">{stats.bestChance.pct}%</span>
              <span class="text-muted-foreground">· {stats.bestChance.cls}{stats.bestChance.number ? ` · ${stats.bestChance.number}` : ''}</span>
            </span>
          {/if}
          <DataSourceBadge source={data?.data_source} class="ml-auto" />
          {#if notice}
            <p class="w-full truncate text-[11px] text-muted-foreground" title={notice}>{notice}</p>
          {/if}
        </div>

        <div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 border-t pt-1.5">
          <Button
            type="button"
            variant={availableOnly ? 'default' : 'outline'}
            size="sm"
            class="h-6 text-xs"
            aria-pressed={availableOnly}
            onclick={() => (availableOnly = !availableOnly)}
          >
            Available only
          </Button>
          {#if allClasses.length > 1}
            <div class="flex flex-wrap items-center gap-1" role="group" aria-label="Filter classes">
              {#each allClasses as c (c)}
                <button
                  type="button"
                  aria-pressed={!hiddenClasses.includes(c)}
                  title={hiddenClasses.includes(c) ? `Show ${c}` : `Hide ${c}`}
                  onclick={() => toggleClass(c)}
                  class={`inline-flex h-6 cursor-pointer items-center rounded-md border px-2 font-mono text-[11px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
                    hiddenClasses.includes(c)
                      ? 'border-border text-muted-foreground opacity-50 hover:opacity-100'
                      : 'border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80'
                  }`}
                >
                  {c}
                </button>
              {/each}
            </div>
          {/if}
          {#if filtersActive || sortKey !== 'departure'}
            <button
              type="button"
              class="text-xs text-muted-foreground underline underline-offset-2 hover:text-foreground"
              onclick={resetFilters}
            >
              Reset ({filteredTrains.length}/{trains.length})
            </button>
          {/if}
          <div class="ml-auto flex items-center gap-2">
            <Select.Root type="single" bind:value={sortKey}>
              <Select.Trigger class="h-7 w-32 text-xs" aria-label="Sort trains by">
                {SORTS.find(([k]) => k === sortKey)?.[1] ?? 'Departure'}
              </Select.Trigger>
              <Select.Content>
                {#each SORTS as [k, label] (k)}
                  <Select.Item value={k} {label} />
                {/each}
              </Select.Content>
            </Select.Root>
            <div
              class="flex overflow-hidden rounded-md border"
              role="group"
              aria-label="Result layout"
            >
              <button
                type="button"
                aria-pressed={view === 'cards'}
                title="Card view"
                onclick={() => (view = 'cards')}
                class={`cursor-pointer px-2.5 py-1 text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${view === 'cards' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'}`}
              >
                Cards
              </button>
              <button
                type="button"
                aria-pressed={view === 'matrix'}
                title="Matrix view"
                onclick={() => (view = 'matrix')}
                class={`cursor-pointer px-2.5 py-1 text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${view === 'matrix' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted'}`}
              >
                Matrix
              </button>
            </div>
          </div>
        </div>
      </div>

      {#if filteredTrains.length === 0}
        <Card.Root>
          <Card.Content class="py-8 text-center text-sm text-muted-foreground">
            No trains match the current filters.
          </Card.Content>
        </Card.Root>
      {:else if view === 'matrix'}
        <div class="rounded-lg border">
          <Table.Root class="text-xs">
            <Table.Header>
              <Table.Row>
                <Table.Head
                  class="sticky left-0 z-10 min-w-48 border-r bg-background"
                  scope="col"
                >
                  Train
                </Table.Head>
                {#each matrixClasses as c (c)}
                  <Table.Head class="text-center font-mono" scope="col">{c}</Table.Head>
                {/each}
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each filteredTrains as tr, i (asText(tr?.number) || `m-${i}`)}
                <Table.Row>
                  <Table.Cell class="sticky left-0 z-10 border-r bg-background">
                    <div class="flex items-center gap-1.5">
                      <TrainNumberBadge number={tr?.number} name={tr?.name} />
                      <span class="max-w-36 truncate font-medium">{asText(tr?.name)}</span>
                    </div>
                    <div class="mt-0.5 font-mono text-[10px] tabular-nums text-muted-foreground">
                      {fmt(tr?.departure_time)} → {fmt(tr?.arrival_time)} · {durationLabel(tr)}
                    </div>
                  </Table.Cell>
                  {#each matrixClasses as c (c)}
                    {@const row = visibleRows(tr).find((r) => classCode(r) === c)}
                    <Table.Cell class="min-w-24 text-center align-top">
                      {#if row}
                        <div class="flex justify-center">
                          <AvailabilityStatusBadge status={row?.status} size="xs" class="max-w-28 truncate" />
                        </div>
                        {#if numOrNull(row?.fare) != null}
                          <div class="mt-0.5 font-mono text-[10px] tabular-nums text-muted-foreground">
                            ₹{numOrNull(row?.fare).toLocaleString('en-IN')}
                          </div>
                        {/if}
                      {:else}
                        <span class="text-muted-foreground">—</span>
                      {/if}
                    </Table.Cell>
                  {/each}
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        </div>
      {:else}
        <div class="grid gap-2.5">
          {#each filteredTrains as tr, i (asText(tr?.number) || `c-${i}`)}
            {@const flags = dayFlags(tr?.runs_on)}
            <article class="rounded-lg border bg-card">
              <div class="flex flex-wrap items-center gap-x-2.5 gap-y-1 px-3 py-2">
                <div class="flex min-w-0 items-center gap-2">
                  <TrainNumberBadge number={tr?.number} name={tr?.name} />
                  <span class="truncate text-sm font-medium">{asText(tr?.name) || 'Unknown train'}</span>
                </div>
                <div class="ml-auto flex flex-wrap items-center gap-2">
                  {#if flags}
                    <RunsOnBadges {flags} format="letter" />
                  {/if}
                  <span class="font-mono text-xs tabular-nums text-muted-foreground">
                    {fmt(tr?.departure_time)} → {fmt(tr?.arrival_time)} · {durationLabel(tr)}
                  </span>
                </div>
              </div>
              <div class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-1.5 border-t px-3 py-2">
                {#each visibleRows(tr) as r, j (j)}
                  {@render avlChip(r)}
                {:else}
                  <span class="text-xs text-muted-foreground">No class-level status returned.</span>
                {/each}
              </div>
            </article>
          {/each}
        </div>
      {/if}
    {/if}
  {:else}
    <EmptyState
      icon={CalendarDaysIcon}
      title="Nothing searched yet"
      hint="Pick source, destination and a date, then Search."
    />
  {/if}
</section>
