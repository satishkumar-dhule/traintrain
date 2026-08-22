<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'
import {
  TrainNumberBadge,
  StationCodeBadge,
  DelayBadge,
  RunsOnBadges,
  HaltStatusBadge,
  StatusBadge,
  parseDelayMinutes
} from '$lib/components/badges/index.js'
import RouteMap from '$lib/components/RouteMap.svelte'
import ActivityIcon from 'lucide-svelte/icons/activity'
  import LightbulbIcon from 'lucide-svelte/icons/lightbulb'
  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'
  import ChartColumnIcon from 'lucide-svelte/icons/chart-no-axes-column'
  import MapIcon from 'lucide-svelte/icons/map'
  import { Input } from '$lib/components/ui/input/index.js'

  let { number = '', view = '' } = $props()

  let query = $state('')
  let auto = $state(false)
  let activeTab = $state('status')
  let committed = $state('')

  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)

  let schPhase = $state('idle')
  let schErr = $state(null)
  let schData = $state(null)
  let schFor = null

  let avgPhase = $state('idle')
  let avgErr = $state(null)
  let avgData = $state(null)
  let avgFor = null

  /* Route-map tab state. mapFor keys on "train|station" so an optional
     station-code spot lookup re-fetches without losing the plain route. */
  let mapPhase = $state('idle')
  let mapErr = $state(null)
  let mapData = $state(null)
  let mapFor = null
  let mapStation = $state('')

  /* Run-instance selection (the NTES "start date" tabs): index into
     data.instances, plus whether the user manually picked one so a 30s
     auto-refresh doesn't yank them back to the active run. */
  let runIdx = $state(0)
  let runTouched = $state(false)

  const RECENT_KEY = 'rc-train-recent'
  let recent = $state(loadRecent(RECENT_KEY))

  function rememberTrain(t, d) {
    const number = String(d?.train_number ?? t).trim()
    if (!number) return
    recent = rememberRecent(
      RECENT_KEY,
      { id: number, label: number, sub: String(d?.train_name ?? '') },
      (r) => r && typeof r?.id === 'string',
    )
  }

  const VIEW_TO_TAB = { status: 'status', schedule: 'schedule', delay: 'avg', map: 'map' }
  const TAB_TO_VIEW = Object.fromEntries(Object.entries(VIEW_TO_TAB).map(([v, t]) => [t, v]))
  const RUN_MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec']

  function asText(v) {
    return String(v ?? '').trim()
  }

  async function loadStatus(t) {
    const fresh = `${data?.train_number}` !== `${t}`
    phase = fresh ? 'loading' : 'refreshing'
    errorMsg = null
    const res = await api(`/rail-api/live-status?train=${encodeURIComponent(t)}`)
    if (`${committed}` !== `${t}`) return
    if (res.ok) {
      data = res.data
      if (fresh) {
        runTouched = false
        rememberTrain(t, data)
      }
      if (!runTouched || !Array.isArray(data.instances) || runIdx >= data.instances.length) {
        runIdx = defaultRunIdx(data)
      }
      phase = 'ok'
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  async function loadSchedule(t) {
    schFor = t
    schPhase = 'loading'
    schErr = null
    const res = await api(`/rail-api/schedule?train=${encodeURIComponent(t)}`)
    if (`${schFor}` !== `${t}`) return
    if (res.ok) {
      schData = res.data
      schPhase = 'ok'
    } else {
      schErr = res.error || `HTTP ${res.status}`
      schPhase = 'error'
    }
  }

  async function loadAvg(t) {
    avgFor = t
    avgPhase = 'loading'
    avgErr = null
    const res = await api(`/rail-api/ntes/average-delay?train=${encodeURIComponent(t)}`)
    if (`${avgFor}` !== `${t}`) return
    if (res.ok) {
      avgData = res.data
      avgPhase = 'ok'
    } else {
      avgErr = res.error || `HTTP ${res.status}`
      avgPhase = 'error'
    }
  }

  async function loadMap(t, station = '') {
    const key = `${t}|${station}`
    mapFor = key
    mapPhase = 'loading'
    mapErr = null
    const qs =
      `/rail-api/ntes/train-on-map?train=${encodeURIComponent(t)}` +
      (station ? `&station=${encodeURIComponent(station)}` : '')
    const res = await api(qs)
    if (mapFor !== key) return
    if (res.ok) {
      mapData = res.data
      mapPhase = 'ok'
    } else {
      mapErr = res.error || `HTTP ${res.status}`
      mapPhase = 'error'
    }
  }

  /* Station-code spot lookup from the map tab's input. */
  function applyMapStation() {
    if (!committed) return
    const st = mapStation.trim().toUpperCase()
    if (st && !/^[A-Z0-9]{2,4}$/.test(st)) {
      mapPhase = 'error'
      mapErr = 'Enter a valid station code (2–4 letters/digits, e.g. NDLS), or leave blank for route only.'
      return
    }
    loadMap(committed, st)
  }

  function track(n) {
    const t = String(n ?? query ?? '').trim()
    if (!t) return
    committed = t
    const want = `/train/${encodeURIComponent(t)}/${TAB_TO_VIEW[activeTab] ?? 'status'}`
    if (route.path !== want) navigate(want)
    loadStatus(t)
  }

  function onTabChange(v) {
    if (!committed) return
    navigate(`/train/${committed}/${TAB_TO_VIEW[v] ?? 'status'}`)
  }

  $effect(() => {
    const n = asText(number)
    const tab = VIEW_TO_TAB[asText(view)] ?? null
    if (tab && tab !== activeTab) activeTab = tab
    if (n && `${n}` !== `${committed}`) {
      query = n
      committed = n
      loadStatus(n)
    }
  })

  $effect(() => {
    const t = committed
    const tab = activeTab
    if (!t) return
    if (tab === 'schedule') {
      if (`${schFor}` !== `${t}`) loadSchedule(t)
    } else if (tab === 'avg') {
      if (`${avgFor}` !== `${t}`) loadAvg(t)
    } else if (tab === 'map') {
      /* Load for a new train; explicit station lookups call loadMap directly.
         Deliberately does NOT read mapStation (an input keystroke would
         otherwise re-trigger this effect and refetch). */
      if (!String(mapFor ?? '').startsWith(`${t}|`)) loadMap(t)
    }
  })

  $effect(() => {
    if (!auto) return
    const timer = setInterval(() => {
      if (committed) loadStatus(committed)
    }, 30000)
    return () => clearInterval(timer)
  })

  function delayText(d) {
    const mins = parseDelayMinutes(d)
    if (mins == null) return '—'
    if (mins > 0) return `${mins} min late`
    return 'on time'
  }

  function fmtTime(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }

  function numOrNull(v) {
    const s = String(v ?? '').trim()
    if (!s) return null
    const n = Number(s)
    return Number.isFinite(n) ? n : null
  }

  /* Normalize NTES "DD-MMM-YYYY" (and ISO) dates to YYYY-MM-DD. */
  function normDay(s) {
    const str = String(s ?? '').trim()
    const m = /^(\d{1,2})-([A-Za-z]{3})-(\d{4})$/.exec(str)
    if (m) {
      const mo = RUN_MONTHS.indexOf(m[2].toLowerCase())
      return mo >= 0 ? `${m[3]}-${String(mo + 1).padStart(2, '0')}-${m[1].padStart(2, '0')}` : ''
    }
    return str.slice(0, 10)
  }

  function parseRunDate(s) {
    const iso = normDay(s)
    if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) return null
    const d = new Date(`${iso}T00:00:00`)
    return Number.isNaN(d.getTime()) ? null : d
  }

  /* NTES-style tab label: "Today · 22 Aug" near today, plain date after. */
  function runLabel(sd) {
    const d = parseRunDate(sd)
    if (!d) return String(sd ?? '').trim() || 'Run'
    const now = new Date()
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
    const target = new Date(d.getFullYear(), d.getMonth(), d.getDate())
    const diff = Math.round((target - today) / 86400000)
    const rel = diff === 0 ? 'Today' : diff === 1 ? 'Tomorrow' : diff === -1 ? 'Yesterday' : null
    const short = `${d.getDate()} ${d.toLocaleDateString('en-US', { month: 'short' })}`
    return rel ? `${rel} · ${short}` : short
  }

  /* The run NTES lands on by default: the instance whose start date is the
     train's reported train_start_date (the active run), else the first. */
  function defaultRunIdx(d) {
    const insts = Array.isArray(d?.instances) ? d.instances : []
    if (!insts.length) return -1
    const start = normDay(d.train_start_date)
    if (start) {
      const i = insts.findIndex((x) => normDay(x.start_date) === start)
      if (i >= 0) return i
    }
    return 0
  }

  function pickRun(i) {
    runTouched = true
    runIdx = i
  }

  const runInstances = $derived(data?.instances ?? [])

  const selectedRun = $derived.by(() => {
    if (!runInstances.length) return null
    const i = Math.min(Math.max(runIdx, 0), runInstances.length - 1)
    return runInstances[i] ?? null
  })

  /* Selected instance's own timeline when it has one (real arrivals for a
     past run, scheduled for an upcoming one); else the active run's. */
  const statusRows = $derived.by(() => {
    if (!data) return []
    const stops = selectedRun?.stops
    return Array.isArray(stops) && stops.length ? stops : (data.stations ?? [])
  })

  const runPosition = $derived(
    String(selectedRun?.position || data?.current_location_info || '').trim(),
  )

  const statusCols = $derived.by(() => {
    const cols = [
      { key: 'station', label: 'Station', value: (s) => `${s.name ?? ''} ${s.code ?? ''}` },
      { key: 'sched', label: 'Sched', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.scheduled_arrival) },
      { key: 'actual', label: 'Actual', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.actual_arrival) },
      {
        key: 'delay',
        label: 'Delay',
        class: 'w-28',
        value: (s) => delayText(s.delay_minutes),
        sortValue: (s) => parseDelayMinutes(s.delay_minutes),
      },
      {
        key: 'status',
        label: 'Status',
        class: 'w-28',
        value: (s) => String(s.status ?? ''),
      },
    ]
    if (statusRows.some((s) => s?.platform)) {
      cols.splice(1, 0, { key: 'pf', label: 'PF', cellClass: 'font-mono text-xs', value: (s) => String(s.platform ?? '') })
    }
    return cols
  })

  const schedCols = [
    { key: 'code', label: 'Code', cellClass: 'font-mono text-xs', value: (s) => s.code },
    { key: 'station', label: 'Station', cellClass: 'font-medium', value: (s) => s.name },
    { key: 'arrival', label: 'Arr', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.arrival) },
    { key: 'departure', label: 'Dep', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.departure) },
    {
      key: 'distance',
      label: 'Dist',
      class: 'w-20',
      cellClass: 'font-mono text-xs',
      value: (s) => (s.distance_km == null ? '' : `${s.distance_km}`),
      sortValue: (s) => numOrNull(s.distance_km),
    },
    {
      key: 'day',
      label: 'Day',
      class: 'w-16',
      cellClass: 'font-mono text-xs',
      value: (s) => (s.day == null ? '' : String(s.day)),
      sortValue: (s) => numOrNull(s.day),
    },
  ]

  const avgCols = [
    { key: 'station', label: 'Station', value: (s) => `${s.name ?? ''} ${s.code ?? ''}` },
    {
      key: 'arrival_delay',
      label: 'Arrival delay',
      value: (s) => delayLabel(s.arrival_delay),
      sortValue: (s) => avgDelayMins(s.arrival_delay),
    },
    {
      key: 'departure_delay',
      label: 'Departure delay',
      value: (s) => delayLabel(s.departure_delay),
      sortValue: (s) => avgDelayMins(s.departure_delay),
    },
  ]

  const mapCols = [
    { key: 'code', label: 'Code', cellClass: 'font-mono text-xs', value: (s) => s.code },
    { key: 'station', label: 'Station', cellClass: 'font-medium', value: (s) => s.name },
    { key: 'arrival', label: 'Arr', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.arrival) },
    { key: 'departure', label: 'Dep', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.departure) },
    {
      key: 'day',
      label: 'Day',
      class: 'w-16',
      cellClass: 'font-mono text-xs',
      value: (s) => String(s.day ?? ''),
      sortValue: (s) => numOrNull(s.day),
    },
    {
      key: 'distance',
      label: 'Dist (km)',
      class: 'w-24',
      cellClass: 'font-mono text-xs',
      value: (s) => String(s.distance ?? ''),
      sortValue: (s) => numOrNull(s.distance),
    },
    { key: 'exp_arrival', label: 'Exp. arr', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.expected_arrival) },
    { key: 'exp_departure', label: 'Exp. dep', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.expected_departure) },
    {
      key: 'delay',
      label: 'Spot delay',
      class: 'w-44',
      value: (s) => [s.arrival_delay, s.departure_delay].filter((v) => String(v ?? '').trim()).join(' / ') || '—',
      sortValue: (s) => spotDelayMins(s.arrival_delay) ?? spotDelayMins(s.departure_delay) ?? null,
    },
  ]

  /* NTES spot strings ("On Time", "Delayed …") → sortable minutes when
     parseable; on-time sorts first, unparseable text last. */
  function spotDelayMins(v) {
    const str = String(v ?? '').trim()
    if (!str) return null
    if (/on time/i.test(str)) return 0
    const m = /(\d+)\s*min/i.exec(str)
    if (m) return Number(m[1])
    return avgDelayMins(v)
  }

  /* NTES average-delay strings: "" (unknown), "On Time" (0), "HH:MM"
     (hours:minutes late, e.g. "00:20"), or signed plain minutes ("+12").
     → signed minutes when parseable; null otherwise. */
  function avgDelayMins(v) {
    const str = String(v ?? '').trim()
    if (!str) return null
    if (/on time/i.test(str)) return 0
    let m = /^(\d{1,3}):(\d{2})$/.exec(str)
    if (m) return Number(m[1]) * 60 + Number(m[2])
    m = /^[+-]?\d+$/.exec(str)
    if (m) return Number(str)
    return null
  }

  function isCurrentStop(code) {
    return String(code ?? '') === String(mapData?.current_station?.code ?? '')
  }

  function maxDelay(stations) {
    let m = 0
    for (const s of stations ?? []) {
      for (const v of [s.arrival_delay, s.departure_delay]) {
        const n = avgDelayMins(v)
        if (n != null && n > m) m = n
      }
    }
    return m
  }

  function barPct(v, m) {
    const n = avgDelayMins(v)
    if (n == null || m <= 0) return n == null ? null : 0
    return Math.min(100, Math.max(0, (n / m) * 100))
  }

  function delayLabel(v) {
    const n = avgDelayMins(v)
    if (n == null) return '—'
    return `${Math.round(n)}m`
  }
</script>

{#snippet statusStationCell(s)}
  <span class="font-medium">{s.name}</span>
  <span class="ml-2"><StationCodeBadge code={s.code} name={s.name} /></span>
{/snippet}

{#snippet statusDelayCell(s)}
  <DelayBadge minutes={s.delay_minutes} />
{/snippet}

{#snippet statusStatusCell(s)}
  <HaltStatusBadge status={s.status} />
{/snippet}

{#snippet avgStationCell(s)}
  <span class="font-medium">{s.name}</span>
  <span class="ml-2"><StationCodeBadge code={s.code} name={s.name} /></span>
{/snippet}

{#snippet schedCodeCell(s)}
  <StationCodeBadge code={s.code} name={s.name} />
{/snippet}

{#snippet avgArrCell(s)}
  {@render avgDelayBar(s, 'arrival_delay')}
{/snippet}

{#snippet avgDepCell(s)}
  {@render avgDelayBar(s, 'departure_delay')}
{/snippet}

{#snippet avgDelayBar(s, key)}
  {@const pct = barPct(s[key], maxDelay(avgData?.stations))}
  <div class="flex items-center gap-2">
    <div class="h-2 flex-1 overflow-hidden rounded bg-muted">
      <div class="h-full rounded bg-primary" style={`width:${pct ?? 0}%`}></div>
    </div>
    <span class="w-12 shrink-0 text-right font-mono text-xs">{delayLabel(s[key])}</span>
  </div>
{/snippet}

{#snippet mapCodeCell(s)}
  <span class="inline-flex items-center gap-2">
    {#if isCurrentStop(s.code)}
      <span class="size-2 shrink-0 rounded-full bg-red-500" aria-label="current position"></span>
    {/if}
    <StationCodeBadge code={s.code} name={s.name} />
  </span>
{/snippet}

{#snippet mapDelayCell(s)}
  {@const pairs = [['Arr', s.arrival_delay], ['Dep', s.departure_delay]].filter(([, v]) => String(v ?? '').trim())}
  {#if pairs.length === 0}
    <span class="text-muted-foreground">—</span>
  {:else}
    <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
      {#each pairs as [label, v] (label)}
        <span class="text-xs text-muted-foreground">{label}</span>
        <StatusBadge tone={/on time/i.test(String(v)) ? 'success' : 'warning'} dot class="font-normal">{v}</StatusBadge>
      {/each}
    </div>
  {/if}
{/snippet}

<section class="grid gap-6" class:idle-center={!committed}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Live train status</h1>
    <p class="text-sm text-muted-foreground">Spot any train by number or name. Data refreshes honestly from the live API.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div
        class="grid min-w-48 flex-1 gap-2"
        onkeydown={(e) => {
          if (e.key === 'Enter' && !e.defaultPrevented) track()
        }}
      >
        <Label for="train-no">Train</Label>
        <AutoCompleteInput
          id="train-no"
          bind:value={query}
          kind="train"
          placeholder="Train number or name…"
          onpick={(item) => {
            query = String(item.number)
            track(item.number)
          }}
        />
      </div>
      <Button type="button" onclick={() => track()} disabled={phase === 'loading' || phase === 'refreshing'}>
        {phase === 'refreshing' ? 'Refreshing…' : 'Track'}
      </Button>
      <label class="mb-0.5 flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
        <input type="checkbox" bind:checked={auto} class="size-4 accent-[var(--primary)]" />
        Auto 30s
      </label>
    </Card.Content>
  </Card.Root>

  {#if !committed && recent.length > 0}
    <RecentSearches
      items={recent}
      onpick={(r) => {
        query = r.label
        track(r.label)
      }}
      onclear={() => {
        clearStored(RECENT_KEY)
        recent = []
      }}
    />
  {/if}

  <Tabs.Root class="min-w-0" bind:value={activeTab} onValueChange={onTabChange}>
    <Tabs.List class="w-full justify-start overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <Tabs.Trigger value="status"><ActivityIcon class="mr-2 size-4" />Status</Tabs.Trigger>
      <Tabs.Trigger value="schedule"><CalendarClockIcon class="mr-2 size-4" />Schedule</Tabs.Trigger>
      <Tabs.Trigger value="avg"><ChartColumnIcon class="mr-2 size-4" />Avg delay</Tabs.Trigger>
      <Tabs.Trigger value="map"><MapIcon class="mr-2 size-4" />Map</Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="status" class="mt-4 grid gap-4">
      {#if phase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if phase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load live status</Alert.Title>
          <Alert.Description>{errorMsg}</Alert.Description>
        </Alert.Root>
      {:else if data}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title class="flex flex-wrap items-center gap-2">
                <TrainNumberBadge number={data.train_number} name={data.train_name} />
                <span>{data.train_name ?? ''}</span>
              </Card.Title>
              <Card.Description>
                {#if runPosition}{runPosition}{:else}{statusRows.length} stations on this run{/if}
              </Card.Description>
            </div>
            <div class="flex items-center gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                onclick={() => navigate(`/insights/live_status/${encodeURIComponent(committed || data.train_number)}`)}
              >
                <LightbulbIcon class="mr-2 size-4" />Explain
              </Button>
              <StatusBadge tone={auto ? 'info' : 'outline'} dot={auto}>{auto ? 'auto 30s' : 'manual'}</StatusBadge>
            </div>
          </Card.Header>
          <Card.Content class="grid gap-3">
            {#if runInstances.length > 1}
              <div class="flex flex-wrap items-center gap-1.5" role="group" aria-label="Train run date">
                {#each runInstances as inst, i (inst.start_date ?? i)}
                  <Button
                    type="button"
                    size="sm"
                    variant={i === runIdx ? 'default' : 'outline'}
                    aria-pressed={i === runIdx}
                    onclick={() => pickRun(i)}
                  >
                    {runLabel(inst.start_date)}
                  </Button>
                {/each}
              </div>
            {/if}
            <DataTable
              columns={statusCols}
              rows={statusRows}
              rowKey={(s, i) => `${i}-${s?.code ?? ''}-${s?.name ?? ''}`}
              cells={{
                station: statusStationCell,
                delay: statusDelayCell,
                status: statusStatusCell,
              }}
              empty="No station data returned."
            />
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={ActivityIcon}
          title="No train tracked yet"
          hint="Enter a train number or name above and press Track to see live status."
        />
      {/if}
    </Tabs.Content>

    <Tabs.Content value="schedule" class="mt-4 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={CalendarClockIcon}
          title="No schedule loaded"
          hint="Enter a train number above to load its schedule."
        />
      {:else if schPhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3, 4] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if schPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load schedule</Alert.Title>
          <Alert.Description>{schErr}</Alert.Description>
        </Alert.Root>
      {:else if schData}
        <Card.Root>
          <Card.Header>
            <Card.Title class="flex flex-wrap items-center gap-2">
              <TrainNumberBadge number={schData.train_number} name={schData.train_name} />
              <span>{schData.train_name ?? ''}</span>
            </Card.Title>
            <Card.Description>{schData.stops?.length ?? 0} scheduled stops</Card.Description>
          </Card.Header>
          <Card.Content class="grid gap-4">
            <div class="flex flex-wrap items-center gap-2">
              <span class="text-xs text-muted-foreground">Runs on</span>
              <RunsOnBadges days={schData.running_days} />
            </div>
            <DataTable
              columns={schedCols}
              rows={schData.stops ?? []}
              rowKey={(s, i) => `${i}-${s?.code ?? ''}-${s?.name ?? ''}`}
              cells={{ code: schedCodeCell }}
              empty="No stops returned."
            />
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={CalendarClockIcon}
          title="No schedule loaded"
          hint="Track a train first, then its scheduled stops appear here."
        />
      {/if}
    </Tabs.Content>

    <Tabs.Content value="avg" class="mt-4 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={ChartColumnIcon}
          title="No delay data loaded"
          hint="Enter a train number above to load average delays."
        />
      {:else if avgPhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3, 4] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if avgPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load average delay</Alert.Title>
          <Alert.Description>{avgErr}</Alert.Description>
        </Alert.Root>
      {:else if avgData}
        {@const maxD = maxDelay(avgData.stations)}
        <Card.Root>
          <Card.Header>
            <Card.Title class="flex flex-wrap items-center gap-2">
              <TrainNumberBadge number={avgData.train_no} name={avgData.train_name} />
              <span>{avgData.train_name ?? ''}</span>
            </Card.Title>
            <Card.Description>
              Average arrival / departure delays{avgData.days_of_run ? ` · runs: ${avgData.days_of_run}` : ''}
            </Card.Description>
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={avgCols}
              rows={avgData.stations ?? []}
              rowKey={(s, i) => `${i}-${s?.sr ?? ''}-${s?.code ?? ''}`}
              cells={{
                station: avgStationCell,
                arrival_delay: avgArrCell,
                departure_delay: avgDepCell,
              }}
              empty="No station data returned."
            />
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={ChartColumnIcon}
          title="No delay data loaded"
          hint="Track a train first, then its average delays appear here."
        />
      {/if}
    </Tabs.Content>

    <Tabs.Content value="map" class="mt-4 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={MapIcon}
          title="No route loaded"
          hint="Enter a train number above to see its route on the map."
        />
      {:else}
        <Card.Root>
          <Card.Content class="flex flex-wrap items-end gap-3">
            <div class="grid min-w-48 flex-1 gap-2"
              onkeydown={(e) => {
                if (e.key === 'Enter' && !e.defaultPrevented) applyMapStation()
              }}
            >
              <Label for="map-station">Your boarding stop (optional)</Label>
              <Input id="map-station" bind:value={mapStation} placeholder="Station code, e.g. NDLS" autocomplete="off" />
            </div>
            <Button type="button" variant="outline" onclick={applyMapStation} disabled={mapPhase === 'loading'}>
              Show live spot
            </Button>
            {#if mapStation.trim() || mapData?.current_station || mapData?.journey_station}
              <Button
                type="button"
                variant="ghost"
                onclick={() => {
                  mapStation = ''
                  loadMap(committed, '')
                }}
                disabled={mapPhase === 'loading'}
              >
                Route only
              </Button>
            {/if}
          </Card.Content>
        </Card.Root>

        {#if mapPhase === 'loading'}
          <Skeleton class="h-[380px] w-full md:h-[440px]" />
        {:else if mapPhase === 'error'}
          <Alert.Root variant="destructive" role="alert">
            <Alert.Title>Could not load route map</Alert.Title>
            <Alert.Description>{mapErr}</Alert.Description>
          </Alert.Root>
        {:else if mapData}
          {#if mapData.current_station || mapData.journey_station}
            <Card.Root>
              <Card.Header>
                <Card.Title class="flex flex-wrap items-center gap-2">Live position</Card.Title>
                <Card.Description>Live spot view from NTES for this run.</Card.Description>
              </Card.Header>
              <Card.Content class="grid gap-3">
                {#if mapData.current_station?.code}
                  <div class="flex flex-wrap items-center gap-2">
                    <StatusBadge tone="info" dot>Current: {mapData.current_station.code}</StatusBadge>
                    {#if mapData.train_name}
                      <span class="text-sm text-muted-foreground">{mapData.train_no ?? ''} {mapData.train_name}</span>
                    {/if}
                  </div>
                {/if}
                {#if mapData.journey_station}
                  {@const j = mapData.journey_station}
                  <div class="grid gap-1.5 text-sm">
                    <div class="flex flex-wrap gap-x-8 gap-y-1">
                      <span><span class="text-muted-foreground">Stop: </span><span class="font-medium">{j.name} ({j.code})</span></span>
                      {#if j.label}<span><span class="text-muted-foreground">Status: </span><span class="font-medium">{j.label}</span></span>{/if}
                    </div>
                    <div class="flex flex-wrap gap-x-8 gap-y-1">
                      {#if j.expected_arrival}<span><span class="text-muted-foreground">Exp. arrival: </span><span class="font-mono">{j.expected_arrival}</span></span>{/if}
                      {#if j.actual_arrival}<span><span class="text-muted-foreground">Actual arrival: </span><span class="font-mono">{j.actual_arrival}</span></span>{/if}
                      {#if j.delay_status}<StatusBadge tone={/on time/i.test(String(j.delay_status)) ? 'success' : 'warning'} dot class="font-normal">{j.delay_status}</StatusBadge>{/if}
                      {#if j.platform}<span><span class="text-muted-foreground">Platform: </span><span class="font-medium">{j.platform}</span></span>{/if}
                    </div>
                  </div>
                {/if}
              </Card.Content>
            </Card.Root>
          {/if}

          <RouteMap data={mapData} />

          <Card.Root>
            <Card.Header>
              <Card.Title>Stations</Card.Title>
              <Card.Description>{mapData.route?.length ?? 0} halts on the route</Card.Description>
            </Card.Header>
            <Card.Content>
              <DataTable
                columns={mapCols}
                rows={mapData.route ?? []}
                rowKey={(s, i) => `${i}-${s?.code ?? ''}`}
                cells={{
                  code: mapCodeCell,
                  delay: mapDelayCell,
                }}
                empty="No station data returned."
              />
            </Card.Content>
          </Card.Root>
        {:else}
          <EmptyState
            icon={MapIcon}
            title="No route loaded"
            hint="Track a train first, then its route map appears here."
          />
        {/if}
      {/if}
    </Tabs.Content>
  </Tabs.Root>
</section>
