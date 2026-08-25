<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import { viewport } from '$lib/media.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import DataTable from '$lib/components/DataTable.svelte'
import ChevronDownIcon from 'lucide-svelte/icons/chevron-down'
import EmptyState from '$lib/components/EmptyState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'
import {
  StationCodeBadge,
  DelayBadge,
  TrainDelayBadge,
  RunsOnBadges,
  HaltStatusBadge,
  StatusBadge,
  ExceptionKindBadge,
  parseDelayMinutes,
  haltStatusKind
} from '$lib/components/badges/index.js'
import { primeTrainDelay } from '$lib/trainDelay.svelte.js'
import RouteMap from '$lib/components/RouteMap.svelte'
import PageHeader from '$lib/components/PageHeader.svelte'
import SignalDot from '$lib/components/SignalDot.svelte'
import Breadcrumbs from '$lib/components/Breadcrumbs.svelte'
import RouteContextBar from '$lib/components/RouteContextBar.svelte'
import EntityChip from '$lib/components/EntityChip.svelte'
import ResultMeta from '$lib/components/ResultMeta.svelte'
import StatPill from '$lib/components/StatPill.svelte'
import ActivityIcon from 'lucide-svelte/icons/activity'

  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'
  import CalendarX2Icon from 'lucide-svelte/icons/calendar-x-2'
  import ChartColumnIcon from 'lucide-svelte/icons/chart-no-axes-column'
  import MapIcon from 'lucide-svelte/icons/map'
  import SparklesIcon from 'lucide-svelte/icons/sparkles'

  let { number = '', view = '' } = $props()

  let query = $state('')
  let auto = $state(loadAutoPref(asText(number)))
  let nextIn = $state(30)
  let activeTab = $state('status')
  let committed = $state('')

  /* On phones the search form collapses to a one-line bar once tracking
     starts, so live station data leads the page. */
  let searchOpen = $state(true)
  $effect(() => {
    if (committed && viewport.narrow) searchOpen = false
  })

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

  let excPhase = $state('idle')
  let excErr = $state(null)
  let excData = $state(null)
  let excFor = null

  /* Run-instance selection (the NTES "start date" tabs): index into
     data.instances, plus whether the user manually picked one so a 30s
     auto-refresh doesn't yank them back to the active run. */
  let runIdx = $state(0)
  let runTouched = $state(false)

  const RECENT_KEY = 'rc-train-recent'
  const AUTO_KEY = 'rc-train-auto'
  let recent = $state(loadRecent(RECENT_KEY))

  function loadAutoPref(t) {
    try {
      const map = JSON.parse(localStorage.getItem(AUTO_KEY) ?? '{}')
      return map?.[t] === true
    } catch {
      return false
    }
  }

  function saveAutoPref(t, v) {
    try {
      const map = JSON.parse(localStorage.getItem(AUTO_KEY) ?? '{}')
      map[t] = v
      localStorage.setItem(AUTO_KEY, JSON.stringify(map))
    } catch {}
  }

  function setAuto(v) {
    auto = v
    if (committed) saveAutoPref(committed, v)
  }

  function rememberTrain(t, d) {
    const number = String(d?.train_number ?? t).trim()
    if (!number) return
    recent = rememberRecent(
      RECENT_KEY,
      { id: number, label: number, sub: String(d?.train_name ?? '') },
      (r) => r && typeof r?.id === 'string',
    )
  }

  const VIEW_TO_TAB = { status: 'status', schedule: 'schedule', delay: 'avg', map: 'map', exceptions: 'exceptions' }
  const TAB_TO_VIEW = Object.fromEntries(Object.entries(VIEW_TO_TAB).map(([v, t]) => [t, v]))
  const RUN_MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec']

  function asText(v) {
    return String(v ?? '').trim()
  }

  async function loadStatus(t) {
    const fresh = `${data?.train_number}` !== `${t}`
    phase = fresh ? 'loading' : 'refreshing'
    errorMsg = null
    nextIn = 30
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
      primeTrainDelay(res.data)
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

  async function loadExceptions(t) {
    excFor = t
    excPhase = 'loading'
    excErr = null
    const res = await api(`/rail-api/ntes/exceptional?train=${encodeURIComponent(t)}`)
    if (`${excFor}` !== `${t}`) return
    if (res.ok) {
      excData = res.data
      excPhase = 'ok'
    } else {
      excErr = res.error || `HTTP ${res.status}`
      excPhase = 'error'
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
    } else if (tab === 'exceptions') {
      if (`${excFor}` !== `${t}`) loadExceptions(t)
    }
  })

  $effect(() => {
    if (!auto) return
    nextIn = 30
    const timer = setInterval(() => {
      nextIn -= 1
      if (nextIn <= 0) {
        nextIn = 30
        if (committed) loadStatus(committed)
      }
    }, 1000)
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

  /* Short label for mobile: fits 3-4 items in 1 line without wrapping. */
  function runShortLabel(sd) {
    const d = parseRunDate(sd)
    if (!d) return String(sd ?? '').trim() || 'Run'
    const now = new Date()
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
    const target = new Date(d.getFullYear(), d.getMonth(), d.getDate())
    const diff = Math.round((target - today) / 86400000)
    if (diff === 0) return 'Today'
    if (diff === 1) return 'Tmrw'
    if (diff === -1) return 'Yday'
    return `${d.getDate()} ${d.toLocaleDateString('en-US', { month: 'short' })}`
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
      { key: 'sched', label: 'Sched', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.scheduled_arrival) },
      { key: 'actual', label: 'Actual', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.actual_arrival) },
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
      cols.splice(1, 0, { key: 'pf', label: 'PF', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => String(s.platform ?? '') })
    }
    return cols
  })

  const schedCols = [
    { key: 'code', label: 'Code', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => s.code },
    { key: 'station', label: 'Station', cellClass: 'font-medium', value: (s) => s.name },
    { key: 'arrival', label: 'Arr', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.arrival) },
    { key: 'departure', label: 'Dep', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.departure) },
    {
      key: 'distance',
      label: 'Dist',
      class: 'w-20',
      cellClass: 'data-num text-xs max-lg:text-sm',
      value: (s) => (s.distance_km == null ? '' : `${s.distance_km}`),
      sortValue: (s) => numOrNull(s.distance_km),
    },
    {
      key: 'day',
      label: 'Day',
      class: 'w-16',
      cellClass: 'data-num text-xs max-lg:text-sm',
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
    { key: 'code', label: 'Code', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => s.code },
    { key: 'station', label: 'Station', cellClass: 'font-medium', value: (s) => s.name },
    { key: 'arrival', label: 'Arr', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.arrival) },
    { key: 'departure', label: 'Dep', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.departure) },
    {
      key: 'day',
      label: 'Day',
      class: 'w-16',
      cellClass: 'data-num text-xs max-lg:text-sm',
      value: (s) => String(s.day ?? ''),
      sortValue: (s) => numOrNull(s.day),
    },
    {
      key: 'distance',
      label: 'Dist (km)',
      class: 'w-24',
      cellClass: 'data-num text-xs max-lg:text-sm',
      value: (s) => String(s.distance ?? ''),
      sortValue: (s) => numOrNull(s.distance),
    },
    { key: 'exp_arrival', label: 'Exp. arr', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.expected_arrival) },
    { key: 'exp_departure', label: 'Exp. dep', cellClass: 'data-num text-xs max-lg:text-sm', value: (s) => fmtTime(s.expected_departure) },
    {
      key: 'delay',
      label: 'Spot delay',
      class: 'w-44',
      value: (s) => [s.arrival_delay, s.departure_delay].filter((v) => String(v ?? '').trim()).join(' / ') || '—',
      sortValue: (s) => spotDelayMins(s.arrival_delay) ?? spotDelayMins(s.departure_delay) ?? null,
    },
  ]

  const excCols = [
    {
      key: 'date',
      label: 'Date',
      class: 'w-36',
      value: (e) => fmtExcDate(e.date),
      sortValue: (e) => String(e.date ?? '').trim() || null,
    },
    { key: 'kind', label: 'Kind', class: 'w-40', value: (e) => e.kind || 'unknown' },
    {
      key: 'note',
      label: 'Note',
      cellClass: 'max-w-md whitespace-normal break-words text-sm text-muted-foreground',
      value: (e) => e.note || '',
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

  /* ISO exception dates → NTES-style DD-MMM-YYYY for display. */
  function fmtExcDate(iso) {
    const raw = String(iso ?? '').trim()
    if (!raw) return '—'
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(raw)
    if (!m) return raw
    const mo = (RUN_MONTHS[Number(m[2]) - 1] ?? '').toUpperCase()
    return mo ? `${m[3]}-${mo}-${m[1]}` : raw
  }
</script>

{#snippet statusStationCell(s)}
  <span class="flex flex-wrap items-center gap-2">
    {#if haltStatusKind(s.status) === 'live'}<SignalDot tone="go" pulse />{/if}
    <span class="font-medium">{s.name}</span>
    <span class="ml-0.5"><StationCodeBadge code={s.code} name={s.name} /></span>
  </span>
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
    <span class="data-num w-12 shrink-0 text-right text-xs">{delayLabel(s[key])}</span>
  </div>
{/snippet}

{#snippet mapCodeCell(s)}
  <span class="inline-flex items-center gap-2">
    {#if isCurrentStop(s.code)}
      <span class="inline-flex" aria-label="current position">
        <SignalDot tone="go" pulse />
      </span>
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

{#snippet excDateCell(e)}
  <span class="data-num text-xs max-lg:text-sm">{fmtExcDate(e.date)}</span>
{/snippet}

{#snippet excKindCell(e)}
  <ExceptionKindBadge kind={e.kind} />
{/snippet}

<section class="grid gap-4 overflow-hidden md:gap-6" class:idle-center={!committed}>
  {#if committed && viewport.narrow}
    <RouteContextBar
      from={committed}
      to={data?.train_name ?? ''}
      onEdit={() => { query = ''; searchOpen = true }}
    />
  {:else}
    <PageHeader title="Live train status" description="Spot any train by number or name. Data refreshes honestly from the live API.">
      {#snippet children()}
        <Breadcrumbs items={[{ label: 'Home', href: '/' }, { label: 'Live Train', href: '/train' }, { label: 'Train ' + (committed || number) }]} />
      {/snippet}
    </PageHeader>
  {/if}

  <div class="track-rule" aria-hidden="true"></div>

  {#if committed && viewport.narrow && !searchOpen}
    <button
      type="button"
      class="flex min-h-9 w-full items-center gap-2 rounded-lg border bg-card px-3 text-left text-sm transition-colors hover:bg-accent/50"
      onclick={() => (searchOpen = true)}
      aria-expanded="false"
    >
      <span class="data-num min-w-0 flex-1 truncate font-medium">{committed}</span>
      <span class="text-xs font-medium text-primary">Change</span>
      <ChevronDownIcon class="size-4 shrink-0 text-muted-foreground" />
    </button>
  {:else}
    <Card.Root>
      <Card.Content class="flex flex-wrap items-end gap-3 max-lg:p-3">
        <div
          class="grid min-w-0 sm:min-w-48 flex-1 gap-2"
          onkeydown={(e) => {
            if (e.key === 'Enter' && !e.defaultPrevented) track()
          }}
        >
          <Label for="train-no" class="max-lg:hidden">Train</Label>
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
        <Button type="button" onclick={() => track()} disabled={phase === 'loading' || phase === 'refreshing'} class="shrink-0 max-lg:min-h-9 max-lg:w-full sm:w-auto">
          {phase === 'refreshing' ? 'Refreshing…' : 'Track'}
        </Button>
        <label class="mb-0.5 flex min-h-9 cursor-pointer items-center gap-2 py-2 text-sm text-muted-foreground shrink-0">
          <input
            type="checkbox"
            checked={auto}
            onchange={(e) => setAuto(e.currentTarget.checked)}
            class="size-4 accent-primary"
          />
          Auto 30s
          {#if auto}<span class="data-num text-xs">next {nextIn}s</span>{/if}
        </label>
      </Card.Content>
    </Card.Root>
  {/if}

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
    <Tabs.List
      class="w-full justify-start overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden max-lg:grid max-lg:grid-cols-5 max-lg:gap-1 max-lg:overflow-visible max-lg:h-auto max-lg:p-1 max-lg:bg-muted max-lg:rounded-lg max-lg:border"
    >
      <Tabs.Trigger value="status" title="Status" aria-label="Live status" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><ActivityIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Status</span></Tabs.Trigger>
      <Tabs.Trigger value="schedule" title="Schedule" aria-label="Schedule" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><CalendarClockIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Schedule</span></Tabs.Trigger>
      <Tabs.Trigger value="avg" title="Avg delay" aria-label="Avg delay" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><ChartColumnIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Avg delay</span></Tabs.Trigger>
      <Tabs.Trigger value="map" title="Map" aria-label="Map" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><MapIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Map</span></Tabs.Trigger>
      <Tabs.Trigger value="exceptions" title="Exceptions" aria-label="Exceptions" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><CalendarX2Icon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Exceptions</span></Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="status" class="mt-3 grid gap-4">
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
          <Card.Header class="flex flex-col items-start justify-between gap-3 space-y-0 sm:flex-row sm:items-center">
            <div class="grid gap-1">
              <Card.Title class="flex flex-wrap items-center gap-2">
                <EntityChip type="train" code={data.train_number} name={data.train_name} />
                <span>{data.train_name ?? ''}</span>
                <TrainDelayBadge number={data.train_number} name={data.train_name} />
              </Card.Title>
              <Card.Description>
                {#if runPosition}{runPosition}{:else}{statusRows.length} stations on this run{/if}
              </Card.Description>
              <ResultMeta source={data?.data_source}>
                <StatPill label="Stations" value={statusRows.length} />
              </ResultMeta>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                aria-label="Ask Train Bro"
                title="Ask the assistant about this train"
                onclick={() => navigate(`/assistant/${encodeURIComponent('live status of ' + number)}`)}
              >
                <SparklesIcon class="size-4 max-lg:mr-0" /><span class="max-lg:hidden">Ask Train Bro</span>
              </Button>
              <StatusBadge tone={auto ? 'info' : 'outline'} dot={auto} class="max-lg:hidden">{auto ? 'auto 30s' : 'manual'}</StatusBadge>
            </div>
          </Card.Header>
          <Card.Content class="grid gap-3">
            {#if runInstances.length > 1}
              <div
                class="flex w-full items-center gap-0.5 overflow-hidden rounded-lg border bg-muted p-0.5 max-lg:gap-0.5"
                role="group"
                aria-label="Train run date"
              >
                {#each runInstances as inst, i (inst.start_date ?? i)}
                  <button
                    type="button"
                    aria-pressed={i === runIdx}
                    onclick={() => pickRun(i)}
                    title={runLabel(inst.start_date)}
                    class={`data-num min-w-0 flex-1 truncate rounded-md px-2 py-1.5 text-xs font-medium transition-colors max-lg:px-1 max-lg:py-2 max-lg:text-[11px] ${i === runIdx ? 'bg-card text-foreground shadow-sm border' : 'text-muted-foreground hover:text-foreground hover:bg-card/50'}`}
                  >
                    <span class="hidden sm:inline">{runLabel(inst.start_date)}</span>
                    <span class="sm:hidden">{runShortLabel(inst.start_date)}</span>
                  </button>
                {/each}
              </div>
            {/if}
            <DataTable
              columns={statusCols}
              rows={statusRows}
              primary="station"
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

    <Tabs.Content value="schedule" class="mt-3 grid gap-4">
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
              <EntityChip type="train" code={schData.train_number} name={schData.train_name} />
              <span>{schData.train_name ?? ''}</span>
              <TrainDelayBadge number={schData.train_number} name={schData.train_name} />
            </Card.Title>
            <Card.Description>{schData.stops?.length ?? 0} scheduled stops</Card.Description>
            <ResultMeta source={schData?.data_source}>
              <StatPill label="Stops" value={schData.stops?.length ?? 0} />
            </ResultMeta>
          </Card.Header>
          <Card.Content class="grid gap-4">
            <div class="flex flex-wrap items-center gap-2">
              <span class="text-xs text-muted-foreground">Runs on</span>
              <RunsOnBadges days={schData.running_days} />
            </div>
            <DataTable
              columns={schedCols}
              rows={schData.stops ?? []}
              primary="station"
              titleText={(s) => s?.name ?? s?.code ?? ''}
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

    <Tabs.Content value="avg" class="mt-3 grid gap-4">
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
              <EntityChip type="train" code={avgData.train_no} name={avgData.train_name} />
              <span>{avgData.train_name ?? ''}</span>
              <TrainDelayBadge
                number={avgData.train_no}
                name={avgData.train_name}
                type={avgData.train_type ?? ''}
                data={avgData}
              />
            </Card.Title>
            <Card.Description>
              Average arrival / departure delays{avgData.days_of_run ? ` · runs: ${avgData.days_of_run}` : ''}
            </Card.Description>
            <ResultMeta source={avgData?.data_source}>
              <StatPill label="Stations" value={avgData.stations?.length ?? 0} />
            </ResultMeta>
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={avgCols}
              rows={avgData.stations ?? []}
              primary="station"
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

    <Tabs.Content value="map" class="mt-3 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={MapIcon}
          title="No route loaded"
          hint="Enter a train number above to see its route on the map."
        />
      {:else}
        <Card.Root>
          <Card.Content class="flex flex-wrap items-end gap-3 max-lg:gap-2">
            <div class="grid min-w-0 sm:min-w-48 flex-1 gap-2"
              onkeydown={(e) => {
                if (e.key === 'Enter' && !e.defaultPrevented) applyMapStation()
              }}
            >
              <Label for="map-station">Your boarding stop (optional)</Label>
              <AutoCompleteInput
                id="map-station"
                kind="station"
                bind:value={mapStation}
                placeholder="Station code, e.g. NDLS"
              />
            </div>
            <Button type="button" variant="outline" onclick={applyMapStation} disabled={mapPhase === 'loading'} class="shrink-0 max-lg:min-h-11 max-lg:w-full sm:w-auto">
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
                    <StatusBadge tone="info" dot>Current: <EntityChip type="station" code={mapData.current_station.code} name={mapData.current_station.name} /></StatusBadge>
                    {#if mapData.train_name}
                      <span class="text-sm text-muted-foreground"><span class="data-num">{mapData.train_no ?? ''}</span> {mapData.train_name}</span>
                      <TrainDelayBadge number={mapData.train_no} name={mapData.train_name} />
                    {/if}
                  </div>
                {/if}
                {#if mapData.journey_station}
                  {@const j = mapData.journey_station}
                  <div class="grid gap-1.5 text-sm">
                    <div class="flex flex-wrap gap-x-8 gap-y-1">
                      <span><span class="text-muted-foreground">Stop: </span><span class="font-medium">{j.name} <EntityChip type="station" code={j.code} name={j.name} size="sm" /></span></span>
                      {#if j.label}<span><span class="text-muted-foreground">Status: </span><span class="font-medium">{j.label}</span></span>{/if}
                    </div>
                    <div class="flex flex-wrap gap-x-8 gap-y-1">
                      {#if j.expected_arrival}<span><span class="text-muted-foreground">Exp. arrival: </span><span class="data-num">{j.expected_arrival}</span></span>{/if}
                      {#if j.actual_arrival}<span><span class="text-muted-foreground">Actual arrival: </span><span class="data-num">{j.actual_arrival}</span></span>{/if}
                      {#if j.delay_status}<StatusBadge tone={/on time/i.test(String(j.delay_status)) ? 'success' : 'warning'} dot class="font-normal">{j.delay_status}</StatusBadge>{/if}
                      {#if j.platform}<span><span class="text-muted-foreground">Platform: </span><span class="data-num font-medium">{j.platform}</span></span>{/if}
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
              <ResultMeta source={mapData?.data_source}>
                <StatPill label="Halts" value={mapData.route?.length ?? 0} />
              </ResultMeta>
            </Card.Header>
            <Card.Content>
              <DataTable
                columns={mapCols}
                rows={mapData.route ?? []}
                primary="station"
                titleText={(s) => s?.name ?? s?.code ?? ''}
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

    <Tabs.Content value="exceptions" class="mt-3 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={CalendarX2Icon}
          title="No exceptions loaded"
          hint="Enter a train number above to see its cancelled, rescheduled or diverted dates."
        />
      {:else if excPhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3, 4] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if excPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load exceptions</Alert.Title>
          <Alert.Description>{excErr}</Alert.Description>
        </Alert.Root>
      {:else if excPhase === 'ok' && excData}
        {@const train = excData.train ?? {}}
        {@const entries = Array.isArray(excData.exceptions) ? excData.exceptions : []}
        {@const msg = typeof excData.message === 'string' ? excData.message.trim() : ''}
        {@const excRoute =
          [train.source, train.destination].filter((s) => s && String(s).trim()).join(' → ')}
        <Card.Root>
          <Card.Header class="min-w-0">
            <Card.Title class="flex flex-wrap items-center gap-x-2 break-words">
              <EntityChip type="train" code={train.number || committed} name={train.name} />
              {#if train.name}<span class="break-words">{train.name}</span>{/if}
            </Card.Title>
            <Card.Description class="break-words [overflow-wrap:anywhere]">
              {entries.length} exception{entries.length === 1 ? '' : 's'}{excRoute ? ` · ${excRoute}` : ''}
            </Card.Description>
            <ResultMeta source={excData?.data_source}>
              <StatPill label="Records" value={entries.length} />
            </ResultMeta>
          </Card.Header>
          <Card.Content class="grid gap-4">
            {#if entries.length > 0}
              <DataTable
                columns={excCols}
                rows={entries}
                primary="date"
                rowKey={(e, i) => `${e.date ?? ''}|${e.kind ?? ''}|${i}`}
                cells={{ date: excDateCell, kind: excKindCell }}
                empty="No exception records found."
              />
            {:else if msg}
              <Alert.Root role="status">
                <Alert.Title>Nothing to report</Alert.Title>
                <Alert.Description>{msg}</Alert.Description>
              </Alert.Root>
            {:else}
              <div class="flex flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
                <CalendarX2Icon class="size-5" />
                No exception records found for this train.
              </div>
            {/if}
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={CalendarX2Icon}
          title="No exceptions loaded"
          hint="Track a train first, then its cancelled / rescheduled / diverted dates appear here."
        />
      {/if}
    </Tabs.Content>
  </Tabs.Root>

  <div class="h-20 lg:hidden"></div>
</section>
