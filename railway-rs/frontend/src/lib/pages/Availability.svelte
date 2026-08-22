<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import DataTable from '$lib/components/DataTable.svelte'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'
import {
  TrainNumberBadge,
  RunsOnBadges,
  AvailabilityStatusBadge
} from '$lib/components/badges/index.js'
import ArrowDownUpIcon from 'lucide-svelte/icons/arrow-down-up'
import CalendarDaysIcon from 'lucide-svelte/icons/calendar-days'

  let { src = '', dst = '', date = '' } = $props()

  const DATE_RE = /^\d{4}-\d{2}-\d{2}$/

  let from = $state('')
  let to = $state('')
  let journeyDate = $state(today())

  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let committed = null

  const RECENT_KEY = 'rc-availability-recent'
  const recentValid = (r) => r && typeof r?.id === 'string' && DATE_RE.test(String(r?.date ?? ''))
  let recent = $state(loadRecent(RECENT_KEY, recentValid))

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

  function asText(v) {
    return String(v ?? '').trim()
  }

  function fmt(v) {
    const t = asText(v)
    return t && t !== '-' && t !== '--' ? t : '—'
  }

  const canSearch = $derived(
    Boolean(asText(from)) && Boolean(asText(to)) && DATE_RE.test(asText(journeyDate))
  )
  const trains = $derived(Array.isArray(data?.trains) ? data.trains : [])
  const notice = $derived(asText(data?.notice))

  function runDays(runsOn) {
    if (Array.isArray(runsOn)) return runsOn.map((d) => asText(d)).filter(Boolean)
    const t = asText(runsOn)
    if (!t) return []
    return t.split(/[\s,|/]+/).filter(Boolean)
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

  const cols = [
    { key: 'class', label: 'Class', cellClass: 'font-mono font-medium', value: (r) => fmt(r?.class) },
    { key: 'status', label: 'Status', value: (r) => asText(r?.status) || '—' },
    { key: 'fare', label: 'Fare', cellClass: 'font-mono text-xs', value: (r) => fmt(r?.fare) },
    { key: 'quota', label: 'Quota', cellClass: 'text-xs', value: (r) => fmt(r?.quota) },
    { key: 'prediction', label: 'Prediction', cellClass: 'text-xs', value: (r) => fmt(r?.prediction) },
  ]
</script>

{#snippet statusCell(row)}
  <AvailabilityStatusBadge status={row?.status} />
{/snippet}

<section class="grid gap-6" class:idle-center={phase === 'idle'}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Availability</h1>
    <p class="text-sm text-muted-foreground">Class-wise availability for a route and date.</p>
  </div>

  <Card.Root>
    <Card.Content
      class="flex flex-wrap items-end gap-3"
      onkeydown={(e) => {
        if (e.key === 'Enter' && !e.defaultPrevented) search()
      }}
    >
      <div class="grid min-w-48 flex-1 gap-2">
        <Label for="av-from">From</Label>
        <AutoCompleteInput
          id="av-from"
          bind:value={from}
          kind="station"
          placeholder="From station…"
          onpick={(item) => {
            if (asText(item?.code)) from = asText(item.code).toUpperCase()
          }}
        />
      </div>
      <Button
        type="button"
        variant="outline"
        size="icon"
        onclick={swap}
        aria-label="Swap From and To stations"
        title="Swap stations"
      >
        <ArrowDownUpIcon />
      </Button>
      <div class="grid min-w-48 flex-1 gap-2">
        <Label for="av-to">To</Label>
        <AutoCompleteInput
          id="av-to"
          bind:value={to}
          kind="station"
          placeholder="To station…"
          onpick={(item) => {
            if (asText(item?.code)) to = asText(item.code).toUpperCase()
          }}
        />
      </div>
      <div class="grid gap-2">
        <Label for="av-date">Date</Label>
        <Input id="av-date" type="date" bind:value={journeyDate} min={today()} />
      </div>
      <Button type="button" onclick={search} disabled={!canSearch}>Search</Button>
    </Card.Content>
  </Card.Root>

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
    <div class="grid gap-4" aria-busy="true">
      {#each [0, 1, 2] as i (i)}
        <Card.Root>
          <Card.Header>
            <Skeleton class="h-5 w-64" />
            <Skeleton class="h-4 w-44" />
          </Card.Header>
          <Card.Content class="grid gap-2">
            <Skeleton class="h-8 w-full" />
            <Skeleton class="h-8 w-full" />
            <Skeleton class="h-8 w-full" />
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
    {#if notice}
      <Alert.Root>
        <Alert.Title>Notice</Alert.Title>
        <Alert.Description>{notice}</Alert.Description>
      </Alert.Root>
    {/if}
    {#if trains.length === 0}
      <Card.Root>
        <Card.Content class="py-10 text-center text-sm text-muted-foreground">
          No trains found for this route/date.
        </Card.Content>
      </Card.Root>
    {:else}
      <div class="grid gap-4">
        {#each trains as tr, i ((asText(tr?.number) || `t-${i}`))}
          {@const days = runDays(tr?.runs_on)}
          <Card.Root>
            <Card.Header class="gap-2">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <Card.Title class="flex flex-wrap items-center gap-2">
                  <TrainNumberBadge number={tr?.number} name={tr?.name} />
                  <span>{asText(tr?.name) || 'Unknown train'}</span>
                </Card.Title>
                {#if days.length > 0}
                  <RunsOnBadges days={days} format="short" />
                {/if}
              </div>
              <Card.Description>
                Dep {fmt(tr?.departure_time)} · Arr {fmt(tr?.arrival_time)}
              </Card.Description>
            </Card.Header>
            <Card.Content>
              <DataTable
                columns={cols}
                rows={Array.isArray(tr?.availability) ? tr.availability : []}
                rowKey={(r, j) => j}
                cells={{ status: statusCell }}
                empty="No availability rows returned."
              />
            </Card.Content>
          </Card.Root>
        {/each}
      </div>
    {/if}
  {:else}
    <EmptyState
      icon={CalendarDaysIcon}
      title="Nothing searched yet"
      hint="Pick source, destination and a date, then Search."
    />
  {/if}
</section>
