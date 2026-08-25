<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import { viewport } from '$lib/media.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
import DateStrip from '$lib/components/DateStrip.svelte'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import AsyncState from '$lib/components/AsyncState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'
import PageHeader from '$lib/components/PageHeader.svelte'
import SignalDot from '$lib/components/SignalDot.svelte'
import Breadcrumbs from '$lib/components/Breadcrumbs.svelte'
import RouteContextBar from '$lib/components/RouteContextBar.svelte'
import EntityChip from '$lib/components/EntityChip.svelte'
import ResultMeta from '$lib/components/ResultMeta.svelte'
import StatPill from '$lib/components/StatPill.svelte'
import { pickNearbyStation } from '$lib/nearby.svelte.js'
import {
  StationCodeBadge,
  DelayBadge,
  TrainDelayBadge,
  RunsOnBadges,
  StatusBadge,
  CountBadge,
  DAY_LETTERS
} from '$lib/components/badges/index.js'
import TabBar from '$lib/components/TabBar.svelte'
import TrackRule from '$lib/components/TrackRule.svelte'
import BottomSpacer from '$lib/components/BottomSpacer.svelte'
import { norm, fmtDash, numOrNull, ntesDate } from '$lib/format.js'
  import ActivityIcon from 'lucide-svelte/icons/activity'
  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'
  import MapPinIcon from 'lucide-svelte/icons/map-pin'

  let { code = '', view = '' } = $props()

  let query = $state('')
  let hours = $state('2')
  let dateInput = $state('')
  let destInput = $state('')
  let tab = $state('live')
  let committedCode = $state('')

  let livePhase = $state('idle')
  let liveError = $state(null)
  let live = $state(null)

  let ttPhase = $state('idle')
  let ttError = $state(null)
  let timetable = $state(null)

  let stationInfo = $state(null)
  let infoFetched = ''

  // Nearby uses the shared blocking dialog (`nearby.svelte.js`): locate ->
  // list stations around the user -> jump straight to that board.
  async function pickNearbyBoard() {
    const picked = await pickNearbyStation()
    if (!picked || !picked.code) return
    onPickStation({ code: picked.code })
  }

  let liveKey = ''
  let ttKey = ''

  const RECENT_KEY = 'rc-station-recent'
  let recent = $state(loadRecent(RECENT_KEY))

  function rememberStation(c, d) {
    if (!c) return
    const name = String(d?.station_name ?? '').trim()
    recent = rememberRecent(
      RECENT_KEY,
      { id: c, label: c, sub: name },
      (r) => r && typeof r?.id === 'string',
    )
  }

  const infoNames = $derived(
    stationInfo
      ? [stationInfo.name_hi, stationInfo.name_gu]
          .map((s) => String(s ?? '').trim())
          .filter(Boolean)
      : []
  )
  const stationName = $derived(
    live?.station ?? timetable?.station_name ?? timetable?.station ?? ''
  )

  const infoMeta = $derived(
    stationInfo
      ? [stationInfo.district, stationInfo.state]
          .map((s) => String(s ?? '').trim())
          .filter(Boolean)
          .join(' · ')
      : ''
  )

  async function loadStationInfo(c) {
    if (infoFetched === c) return
    infoFetched = c
    // Optional enrichment (hi/gu names, district/state). Absent fields or a
    // non-OK answer simply render nothing — never blocks the board.
    const res = await api(`/rail-api/stations/${encodeURIComponent(c)}`)
    if (infoFetched !== c) return
    stationInfo = res.ok && res.data && typeof res.data === 'object' ? res.data : null
  }

  const liveCols = [
    { key: 'train', label: 'Train', value: (t) => `${t.number ?? ''} ${t.name ?? ''}` },
    { key: 'sta', label: 'Sched', cellClass: 'data-num text-xs max-lg:text-sm', value: (t) => fmtDash(t.sta) },
    { key: 'eta', label: 'ETA', cellClass: 'data-num text-xs max-lg:text-sm', value: (t) => fmtDash(t.eta) },
    {
      key: 'delay',
      label: 'Delay',
      class: 'w-24',
      value: (t) => fmtDash(t.delay_arr),
      sortValue: (t) => numOrNull(String(t.delay_arr ?? '').replace(/m$/, '')),
    },
    {
      key: 'platform',
      label: 'Platform',
      class: 'w-24',
      cellClass: 'data-num text-xs max-lg:text-sm',
      value: (t) => fmtDash(t.platform),
      sortValue: (t) => numOrNull(t.platform),
    },
  ]

  const ttCols = [
    { key: 'train', label: 'Train', value: (t) => `${t.number ?? ''} ${t.name ?? ''}` },
    { key: 'type', label: 'Type', class: 'w-20 lg:w-28', value: (t) => t.train_type || '' },
    { key: 'classes', label: 'Cls', class: 'w-16 lg:w-28', value: (t) => t.classes || '' },
    { key: 'arrival', label: 'Arr', cellClass: 'data-num text-xs max-lg:text-sm', value: (t) => fmtDash(t.arrival) },
    { key: 'departure', label: 'Dep', cellClass: 'data-num text-xs max-lg:text-sm', value: (t) => fmtDash(t.departure) },
    {
      key: 'days',
      label: 'Days',
      value: (t) => {
        const f = Array.isArray(t.days) ? t.days : []
        return f.map((on, i) => (on ? DAY_LETTERS[i] ?? '' : '-')).join('')
      },
      sortValue: (t) => (Array.isArray(t.days) ? t.days.filter(Boolean).length : null),
    },
  ]

  async function loadLive(target) {
    const c = norm(target)
    if (!c) return
    const h = String(hours)
    const d = norm(destInput)
    const k = `${c}|${h}|${d}`
    liveKey = k
    livePhase = live && `${live.station ?? ''}` === `${c}` ? 'refreshing' : 'loading'
    liveError = null
    let qs = `station=${encodeURIComponent(c)}&hours=${encodeURIComponent(h)}`
    if (d) qs += `&destination=${encodeURIComponent(d)}`
    const res = await api(`/rail-api/ntes/live-station?${qs}`)
    if (liveKey !== k) return
    if (res.ok) {
      live = res.data
      livePhase = 'ok'
      rememberStation(c, res.data)
    } else {
      livePhase = 'error'
      liveError = res.error || `HTTP ${res.status}`
    }
  }

  function ensureLive() {
    if (!committedCode) return
    const d = norm(destInput)
    if (`${committedCode}|${hours}|${d}` === liveKey) return
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
      rememberStation(c, res.data)
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

  // Picking a destination re-queries the live board immediately (the NTES
  // source filters it upstream); typing still waits for "Show board".
  function onPickDest(item) {
    if (tab !== 'live' || !committedCode || !norm(item && item.code)) return
    loadLive(committedCode)
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
    // bits-ui updates the bound value before calling this callback, so compare
    // against the URL-derived prop rather than `tab`.
    if (!next || next === view) return
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
      loadStationInfo(c)
      if (v === 'timetable') ensureTimetable()
      else ensureLive()
    })
  })
</script>

{#snippet liveTrainCell(t)}
  <span class="flex items-center gap-2">
    <EntityChip type="train" code={t.number} name={t.name} />
    <span class="font-medium">{t.name}</span>
  </span>
{/snippet}

{#snippet liveDelayCell(t)}
  <DelayBadge minutes={t.delay_arr} compact />
{/snippet}

{#snippet ttTrainCell(t)}
  <span class="flex items-center gap-2">
    <EntityChip type="train" code={t.number} name={t.name} />
    <span class="font-medium">{t.name}</span>
    <TrainDelayBadge number={t.number} name={t.name} type={t.train_type} compact />
  </span>
{/snippet}

{#snippet ttTypeCell(t)}
  {#if t.train_type}<StatusBadge tone="outline">{t.train_type}</StatusBadge>{:else}—{/if}
{/snippet}

{#snippet ttClassesCell(t)}
  <span class="text-xs max-lg:text-sm text-muted-foreground">{t.classes || '—'}</span>
{/snippet}

{#snippet ttDaysCell(t)}
  {@const days = Array.isArray(t.days) ? t.days : []}
  <RunsOnBadges days={days} />
{/snippet}

<section class="grid grid-cols-[minmax(0,1fr)] gap-4 md:gap-6" class:idle-center={!committedCode}>
  {#if !viewport.narrow}
    <PageHeader title="Station board" description="Live board and full-day timetable for any station.">
      {#snippet children()}
        <Breadcrumbs items={[{ label: 'Home', href: '/' }, { label: 'Station', href: '/station' }, { label: committedCode ? 'Station ' + committedCode : 'Station' }]} />
        {#if infoNames.length}
          <p class="text-sm text-muted-foreground">{infoNames.join(' · ')}</p>
        {/if}
        {#if infoMeta}
          <p class="text-xs text-muted-foreground">{infoMeta}</p>
        {/if}
      {/snippet}
    </PageHeader>
  {/if}

  {#if viewport.narrow && committedCode}
    <RouteContextBar
      from={committedCode}
      to={stationName}
      onEdit={() => { committedCode = ''; query = '' }}
    />
  {/if}

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3 max-lg:gap-2">
      <div class="grid min-w-0 sm:min-w-56 flex-1 gap-2 max-lg:w-full">
        <Label for="stn-code" class="max-lg:hidden">Station</Label>
        <AutoCompleteInput
          id="stn-code"
          kind="station"
          placeholder="Station name or code, e.g. NDLS"
          bind:value={query}
          onpick={onPickStation}
        />
      </div>
      {#if tab === 'live'}
        <div class="grid min-w-0 sm:min-w-44 flex-1 gap-2 max-lg:w-full">
          <Label for="stn-dest" class="max-lg:hidden">Going to (optional)</Label>
          <AutoCompleteInput
            id="stn-dest"
            kind="station"
            nearby={false}
            placeholder="Filter board, e.g. BCT"
            bind:value={destInput}
            onpick={onPickDest}
          />
        </div>
      {/if}
      <div class="flex items-end gap-2 max-lg:w-full">
        <div class="grid gap-2">
          <Label class="max-lg:hidden">Window</Label>
          <Select.Root type="single" bind:value={hours}>
            <Select.Trigger class="w-28 sm:w-32 max-lg:h-10" aria-label="Time window">
              {hours} hour{hours === '1' ? '' : 's'}
            </Select.Trigger>
            <Select.Content>
              {#each ['1', '2', '3', '4'] as h (h)}
                <Select.Item value={h} label="{h} hour{h === '1' ? '' : 's'}" />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
        <Button
          class="shrink-0 max-lg:h-10 max-lg:px-4 sm:w-auto"
          onclick={showBoard}
          disabled={(tab === 'live' && (livePhase === 'loading' || livePhase === 'refreshing')) ||
            (tab === 'timetable' && (ttPhase === 'loading' || ttPhase === 'refreshing'))}
        >
          {(tab === 'live' && livePhase === 'refreshing') || (tab === 'timetable' && ttPhase === 'refreshing')
            ? 'Refreshing…'
            : 'Show board'}
        </Button>
      </div>
      <div class="grid gap-2 max-lg:w-full">
        <Label class="max-lg:hidden">Nearby</Label>
        <Button variant="outline" onclick={pickNearbyBoard} class="max-lg:h-10 max-lg:w-full">
          <MapPinIcon />
          Nearby
        </Button>
      </div>
    </Card.Content>
  </Card.Root>

  <DateStrip
    id="stn-date"
    bind:value={dateInput}
    label="Timetable date"
    class="z-20 lg:sticky lg:top-0"
    onchange={() => {
      if (tab === 'timetable') ensureTimetable()
    }}
  />

  {#if !committedCode && recent.length > 0}
    <RecentSearches
      items={recent}
      onpick={(r) => onPickStation({ code: r.label })}
      onclear={() => {
        clearStored(RECENT_KEY)
        recent = []
      }}
    />
  {/if}

  <TrackRule />

  <Tabs.Root class="min-w-0" bind:value={tab} onValueChange={onTabChange}>
    <TabBar cols={2}>
      <Tabs.Trigger value="live"><ActivityIcon class="size-4 max-lg:size-[18px] shrink-0" />Live</Tabs.Trigger>
      <Tabs.Trigger value="timetable"><CalendarClockIcon class="size-4 max-lg:size-[18px] shrink-0" />Timetable</Tabs.Trigger>
    </TabBar>

    <Tabs.Content value="live" class="mt-3 grid gap-4">
      <AsyncState
        phase={livePhase}
        error={liveError}
        empty={!live}
        skeletonCount={4}
        emptyIcon={ActivityIcon}
        emptyTitle="No board loaded"
        emptyHint="Enter a station and show the board to see live arrivals & departures."
      >
        {#if live}
          <Card.Root>
          <Card.Header class="flex flex-col items-start justify-between gap-3 space-y-0 sm:flex-row sm:items-center">
            <Card.Title class="flex flex-wrap items-center gap-2">
              <SignalDot tone="go" pulse />
              <StationCodeBadge code={live.station} link={false} />
              <span>departures &amp; arrivals</span>
            </Card.Title>
            <ResultMeta source={live.data_source}>
              <StatPill label="Trains" value={live.trains?.length ?? 0} />
              <StatPill label="Window" value={`${live.hours}h`} />
              {#if live.destination}
                <span class="inline-flex items-center gap-1 text-xs">
                  <span class="text-muted-foreground">Towards</span>
                  <StationCodeBadge code={live.destination} link={false} size="xs" />
                </span>
              {/if}
            </ResultMeta>
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={liveCols}
              rows={live.trains ?? []}
              primary="train"
              rowKey={(t, i) => `${i}-${t?.number ?? ''}-${t?.name ?? ''}`}
              cells={{ train: liveTrainCell, delay: liveDelayCell }}
              empty={live.destination
                ? 'No trains towards this destination in this window.'
                : 'No trains in this window.'}
            />
          </Card.Content>
          </Card.Root>
        {/if}
      </AsyncState>
    </Tabs.Content>

    <Tabs.Content value="timetable" class="mt-3 grid gap-4">
      <AsyncState
        phase={ttPhase}
        error={ttError}
        empty={!timetable}
        skeletonCount={4}
        emptyIcon={CalendarClockIcon}
        emptyTitle="No timetable loaded"
        emptyHint="Enter a station (and optionally a date) to load the full-day timetable."
      >
        {#if timetable}
          <Card.Root>
          <Card.Header class="flex flex-col items-start justify-between gap-3 space-y-0 sm:flex-row sm:items-center">
            <Card.Title>
              {#if timetable.station_name}{timetable.station_name}{:else}<span class="data-num tracking-[0.14em] uppercase">{timetable.station ?? '—'}</span>{/if}
              timetable
            </Card.Title>
            <ResultMeta source={timetable.data_source}>
              <StatPill label="Trains" value={timetable.total ?? timetable.trains?.length ?? 0} />
              {#if timetable.date}<StatPill label="Date" value={timetable.date} />{/if}
              {#snippet actions()}
                <CountBadge value={timetable.total ?? timetable.trains?.length ?? 0} label="total" />
              {/snippet}
            </ResultMeta>
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={ttCols}
              rows={timetable.trains ?? []}
              primary="train"
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
        {/if}
      </AsyncState>
    </Tabs.Content>
  </Tabs.Root>
  <BottomSpacer />
</section>
