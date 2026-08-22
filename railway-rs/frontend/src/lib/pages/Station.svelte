<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
  import ActivityIcon from 'lucide-svelte/icons/activity'
  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'

  let { code = '', view = '' } = $props()

  let query = $state('')
  let hours = $state('2')
  let dateInput = $state('')
  let tab = $state('live')
  let committedCode = $state('')

  let livePhase = $state('idle')
  let liveError = $state(null)
  let live = $state(null)

  let ttPhase = $state('idle')
  let ttError = $state(null)
  let timetable = $state(null)

  let liveKey = ''
  let ttKey = ''

  const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC']
  const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

  function norm(s) {
    return String(s ?? '').trim().toUpperCase()
  }

  function ntesDate(iso) {
    if (!iso) return null
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso).trim())
    if (!m) return null
    const mi = Number(m[2]) - 1
    if (mi < 0 || mi > 11) return null
    return `${m[3]}-${MONTHS[mi]}-${m[1]}`
  }

  function fmt(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }

  function numOrNull(v) {
    const s = String(v ?? '').trim()
    if (!s) return null
    const n = Number(s)
    return Number.isFinite(n) ? n : null
  }

  function daysText(flags) {
    if (!Array.isArray(flags)) return ''
    return flags.map((on, i) => (on ? DAY_LETTERS[i] ?? '' : '-')).join('')
  }

  const liveCols = [
    { key: 'train', label: 'Train', value: (t) => `${t.number ?? ''} ${t.name ?? ''}` },
    { key: 'sta', label: 'Sched', cellClass: 'font-mono text-xs', value: (t) => fmt(t.sta) },
    { key: 'eta', label: 'Expected', cellClass: 'font-mono text-xs', value: (t) => fmt(t.eta) },
    {
      key: 'delay',
      label: 'Delay',
      class: 'w-24',
      value: (t) => fmt(t.delay_arr),
      sortValue: (t) => numOrNull(String(t.delay_arr ?? '').replace(/m$/, '')),
    },
    {
      key: 'platform',
      label: 'Platform',
      class: 'w-24',
      cellClass: 'font-mono text-xs',
      value: (t) => fmt(t.platform),
      sortValue: (t) => numOrNull(t.platform),
    },
  ]

  const ttCols = [
    { key: 'train', label: 'Train', value: (t) => `${t.number ?? ''} ${t.name ?? ''}` },
    { key: 'type', label: 'Type', class: 'w-28', value: (t) => t.train_type || '' },
    { key: 'classes', label: 'Classes', class: 'w-28', value: (t) => t.classes || '' },
    { key: 'arrival', label: 'Arr', cellClass: 'font-mono text-xs', value: (t) => fmt(t.arrival) },
    { key: 'departure', label: 'Dep', cellClass: 'font-mono text-xs', value: (t) => fmt(t.departure) },
    {
      key: 'days',
      label: 'Days',
      value: (t) => daysText(Array.isArray(t.days) ? t.days : []),
      sortValue: (t) => (Array.isArray(t.days) ? t.days.filter(Boolean).length : null),
    },
  ]

  async function loadLive(target) {
    const c = norm(target)
    if (!c) return
    const h = String(hours)
    const k = `${c}|${h}`
    liveKey = k
    livePhase = live && `${live.station ?? ''}` === `${c}` ? 'refreshing' : 'loading'
    liveError = null
    const res = await api(`/rail-api/ntes/live-station?station=${encodeURIComponent(c)}&hours=${encodeURIComponent(h)}`)
    if (liveKey !== k) return
    if (res.ok) {
      live = res.data
      livePhase = 'ok'
    } else {
      livePhase = 'error'
      liveError = res.error || `HTTP ${res.status}`
    }
  }

  function ensureLive() {
    if (!committedCode) return
    if (`${committedCode}|${hours}` === liveKey) return
    loadLive(committedCode)
  }

  async function loadTimetable(target) {
    const c = norm(target)
    if (!c) return
    const d = ntesDate(dateInput) ?? ''
    const k = `${c}|${d}`
    ttKey = k
    ttPhase = timetable && `${timetable.station ?? ''}` === `${c}` ? 'refreshing' : 'loading'
    ttError = null
    const qs = `station=${encodeURIComponent(c)}${d ? `&date=${encodeURIComponent(d)}` : ''}`
    const res = await api(`/rail-api/ntes/station-timetable?${qs}`)
    if (ttKey !== k) return
    if (res.ok) {
      timetable = res.data
      ttPhase = 'ok'
    } else {
      ttPhase = 'error'
      ttError = res.error || `HTTP ${res.status}`
    }
  }

  function ensureTimetable() {
    if (!committedCode) return
    const d = ntesDate(dateInput) ?? ''
    if (`${committedCode}|${d}` === ttKey) return
    loadTimetable(committedCode)
  }

  function onPickStation(item) {
    const c = norm(item && item.code)
    if (!c) return
    committedCode = c
    const want = `/station/${encodeURIComponent(c)}/${tab}`
    if (route.path !== want) navigate(want)
    if (tab === 'live') loadLive(c)
  }

  function showBoard() {
    const c = norm(query)
    if (!c) return
    committedCode = c
    const want = `/station/${encodeURIComponent(c)}/${tab === 'timetable' ? 'timetable' : 'live'}`
    if (route.path !== want) navigate(want)
    if (tab === 'timetable') loadTimetable(c)
    else loadLive(c)
  }

  function onTabChange(next) {
    if (!next || next === tab) return
    tab = next
    if (committedCode) navigate(`/station/${committedCode}/${next}`)
  }

  $effect(() => {
    const c = norm(code)
    const v = view === 'timetable' ? 'timetable' : 'live'
    untrack(() => {
      if (!c) return
      if (norm(query) !== c) query = c
      if (committedCode !== c) committedCode = c
      if (tab !== v) tab = v
      if (v === 'timetable') ensureTimetable()
      else ensureLive()
    })
  })
</script>

{#snippet liveTrainCell(t)}
  <span class="font-mono text-xs text-muted-foreground">{t.number}</span>
  <span class="ml-2 font-medium">{t.name}</span>
{/snippet}

{#snippet liveDelayCell(t)}
  {#if Number(t.delay_arr) > 0}
    <Badge variant="destructive">{t.delay_arr}m</Badge>
  {:else}
    <Badge variant="secondary">on time</Badge>
  {/if}
{/snippet}

{#snippet ttTrainCell(t)}
  <span class="font-mono text-xs text-muted-foreground">{t.number}</span>
  <span class="ml-2 font-medium">{t.name}</span>
{/snippet}

{#snippet ttTypeCell(t)}
  {#if t.train_type}<Badge variant="outline">{t.train_type}</Badge>{:else}—{/if}
{/snippet}

{#snippet ttClassesCell(t)}
  <span class="text-xs text-muted-foreground">{t.classes || '—'}</span>
{/snippet}

{#snippet ttDaysCell(t)}
  {@const days = Array.isArray(t.days) ? t.days : []}
  <div class="flex flex-wrap gap-1">
    {#each DAY_LETTERS as letter, i (i)}
      {#if days[i]}
        <Badge variant="secondary" class="px-1.5 text-[10px]">{letter}</Badge>
      {:else}
        <Badge variant="outline" class="px-1.5 text-[10px] opacity-40">{letter}</Badge>
      {/if}
    {/each}
  </div>
{/snippet}

<section class="grid gap-6" class:idle-center={!committedCode}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Station board</h1>
    <p class="text-sm text-muted-foreground">Live board and full-day timetable for any station.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div class="grid min-w-56 flex-1 gap-2">
        <Label for="stn-code">Station</Label>
        <AutoCompleteInput
          id="stn-code"
          kind="station"
          placeholder="Station name or code, e.g. NDLS"
          bind:value={query}
          onpick={onPickStation}
        />
      </div>
      <div class="grid gap-2">
        <Label>Window</Label>
        <Select.Root type="single" bind:value={hours}>
          <Select.Trigger class="w-32" aria-label="Time window">
            {hours} hour{hours === '1' ? '' : 's'}
          </Select.Trigger>
          <Select.Content>
            {#each ['1', '2', '3', '4'] as h (h)}
              <Select.Item value={h} label="{h} hour{h === '1' ? '' : 's'}" />
            {/each}
          </Select.Content>
        </Select.Root>
      </div>
      <div class="grid gap-2">
        <Label for="stn-date">Date (timetable)</Label>
        <Input id="stn-date" type="date" bind:value={dateInput} class="w-40" />
      </div>
      <Button
        onclick={showBoard}
        disabled={(tab === 'live' && (livePhase === 'loading' || livePhase === 'refreshing')) ||
          (tab === 'timetable' && (ttPhase === 'loading' || ttPhase === 'refreshing'))}
      >
        {(tab === 'live' && livePhase === 'refreshing') || (tab === 'timetable' && ttPhase === 'refreshing')
          ? 'Refreshing…'
          : 'Show board'}
      </Button>
    </Card.Content>
  </Card.Root>

  <Tabs.Root bind:value={tab} onValueChange={onTabChange}>
    <Tabs.List class="w-full justify-start">
      <Tabs.Trigger value="live"><ActivityIcon class="mr-2 size-4" />Live</Tabs.Trigger>
      <Tabs.Trigger value="timetable"><CalendarClockIcon class="mr-2 size-4" />Timetable</Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="live" class="mt-4 grid gap-4">
      {#if livePhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if livePhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load board</Alert.Title>
          <Alert.Description>{liveError}</Alert.Description>
        </Alert.Root>
      {:else if live}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title>{live.station ?? '—'} departures &amp; arrivals</Card.Title>
              <Card.Description>{live.trains?.length ?? 0} trains within {live.hours}h</Card.Description>
            </div>
            <Badge variant="secondary">{live.hours}h window</Badge>
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={liveCols}
              rows={live.trains ?? []}
              rowKey={(t, i) => `${i}-${t?.number ?? ''}-${t?.name ?? ''}`}
              cells={{ train: liveTrainCell, delay: liveDelayCell }}
              empty="No trains in this window."
            />
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={ActivityIcon}
          title="No board loaded"
          hint="Enter a station and show the board to see live arrivals &amp; departures."
        />
      {/if}
    </Tabs.Content>

    <Tabs.Content value="timetable" class="mt-4 grid gap-4">
      {#if ttPhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if ttPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load timetable</Alert.Title>
          <Alert.Description>{ttError}</Alert.Description>
        </Alert.Root>
      {:else if timetable}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title>{timetable.station_name ?? timetable.station ?? '—'} timetable</Card.Title>
              <Card.Description>{timetable.total ?? timetable.trains?.length ?? 0} trains{timetable.date ? ` · ${timetable.date}` : ''}</Card.Description>
            </div>
            <Badge variant="secondary">{timetable.total ?? timetable.trains?.length ?? 0} total</Badge>
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={ttCols}
              rows={timetable.trains ?? []}
              rowKey={(t, i) => `${i}-${t?.number ?? ''}-${t?.name ?? ''}`}
              cells={{
                train: ttTrainCell,
                type: ttTypeCell,
                classes: ttClassesCell,
                days: ttDaysCell,
              }}
              empty="No scheduled trains found."
            />
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={CalendarClockIcon}
          title="No timetable loaded"
          hint="Enter a station (and optionally a date) to load the full-day timetable."
        />
      {/if}
    </Tabs.Content>
  </Tabs.Root>
</section>
