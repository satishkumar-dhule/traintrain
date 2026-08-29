<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import { viewport } from '$lib/media.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import DataTable from '$lib/components/DataTable.svelte'
import ChevronDownIcon from 'lucide-svelte/icons/chevron-down'
import EmptyState from '$lib/components/EmptyState.svelte'
import AsyncState from '$lib/components/AsyncState.svelte'
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
import PageShell from '$lib/components/PageShell.svelte'
import SignalDot from '$lib/components/SignalDot.svelte'
import EntityChip from '$lib/components/EntityChip.svelte'
import ResultMeta from '$lib/components/ResultMeta.svelte'
import StatPill from '$lib/components/StatPill.svelte'
import TabBar from '$lib/components/TabBar.svelte'
import BottomSpacer from '$lib/components/BottomSpacer.svelte'
import { asText, fmtDash, numOrNull, fmtExcDate, normDay } from '$lib/format.js'
import ActivityIcon from 'lucide-svelte/icons/activity'

  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'
  import CalendarX2Icon from 'lucide-svelte/icons/calendar-x-2'
  import ChartColumnIcon from 'lucide-svelte/icons/chart-no-axes-column'
  import MapIcon from 'lucide-svelte/icons/map'
  import SparklesIcon from 'lucide-svelte/icons/sparkles'
  import BedDoubleIcon from 'lucide-svelte/icons/bed-double'
  import LayoutGridIcon from 'lucide-svelte/icons/layout-grid'
  import { Input } from '$lib/components/ui/input/index.js'
  import { todayISO, DATE_RE } from '$lib/format.js'

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

  let chartPhase = $state('idle')
  let chartErr = $state(null)
  let chartData = $state(null)
  let chartFor = null
  let chartDate = $state(todayISO())
  let chartStation = $state('')
  let chartExpanded = $state({})
  const chartFriendlyErr = $derived.by(() => {
    const msg = asText(chartErr)
    if (!msg) return ''
    if (/geofenced|Akamai|403 Forbidden/i.test(msg)) return 'IRCTC is IP-geofenced to India — chart unavailable from this network. Try again from an Indian residential IP or near departure (~4h before, previous evening for early trains).'
    if (msg.length > 400) return msg.slice(0, 400) + '…'
    return msg
  })

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

  const VIEW_TO_TAB = { status: 'status', schedule: 'schedule', delay: 'avg', map: 'map', exceptions: 'exceptions', chart: 'chart' }
  const TAB_TO_VIEW = Object.fromEntries(Object.entries(VIEW_TO_TAB).map(([v, t]) => [t, v]))
  const RUN_MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec']

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

  async function loadChart(t) {
    const d = asText(chartDate).trim()
    const st = asText(chartStation).trim().toUpperCase()
    if (!t || !DATE_RE.test(d)) {
      chartPhase = 'error'
      chartErr = 'Enter a valid journey date (YYYY-MM-DD).'
      return
    }
    chartFor = `${t}|${d}|${st}`
    chartPhase = 'loading'
    chartErr = null
    const qs = `/rail-api/irctc/chart?train=${encodeURIComponent(t)}&date=${encodeURIComponent(d)}${st ? `&station=${encodeURIComponent(st)}` : ''}`
    const res = await api(qs)
    if (chartFor !== `${t}|${d}|${st}`) return
    if (res.ok) {
      chartData = res.data
      chartPhase = 'ok'
    } else {
      chartErr = res.error || `HTTP ${res.status}`
      chartPhase = 'error'
    }
  }

  function chartToggleCoach(code) {
    chartExpanded = { ...chartExpanded, [code]: !chartExpanded[code] }
  }
  function chartBerthTone(s) {
    const v = String(s ?? '').toLowerCase()
    if (v === 'vacant') return 'go'
    if (v === 'occupied') return 'stop'
    return 'idle'
  }
  function chartBerthClass(s) {
    const v = String(s ?? '').toLowerCase()
    if (v === 'vacant') return 'bg-signal-go text-white border-signal-go'
    if (v === 'occupied') return 'bg-muted text-muted-foreground'
    if (v === 'not_reserved' || v === 'notreserved') return 'bg-amber-100 text-amber-800 border-amber-200'
    return 'bg-card border'
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
    } else if (tab === 'chart') {
      if (`${chartFor}` !== `${t}|${chartDate}|${asText(chartStation).toUpperCase()}`) loadChart(t)
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

  const fmtTime = fmtDash

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

  async function pickRun(i) {
    runTouched = true
    runIdx = i
    const inst = runInstances[i]
    if (!inst?.start_date) return
    const d = normDay(inst.start_date)
    if (!d || !committed) return
    // Honest per-run fetch: the backend's select_run_for_date will
    // overlay that instance's real timeline (past run's actuals vs
    // upcoming run's "at origin") so switching dates is never faked.
    const t = committed
    const res = await api(`/rail-api/live-status?train=${encodeURIComponent(t)}&date=${encodeURIComponent(d)}`)
    if (`${committed}` !== `${t}`) return
    if (res.ok) {
      data = res.data
      // Keep the picked tab aligned if the new payload reorders instances
      const newIdx = Array.isArray(data.instances) ? data.instances.findIndex((x) => normDay(x.start_date) === d) : -1
      if (newIdx >= 0) runIdx = newIdx
      runTouched = true
    }
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
  <PageShell
    title="Live train status"
    description="Spot any train by number or name. Data refreshes honestly from the live API."
    breadcrumbs={[{ label: 'Home', href: '/' }, { label: 'Live Train', href: '/train' }, { label: 'Train ' + (committed || number) }]}
    routeFrom={committed}
    routeTo={data?.train_name ?? ''}
    onEdit={() => { query = ''; searchOpen = true }}
    showRouteBar={!!committed}
  />

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
          class="grid min-w-0 sm:min-w-48 flex-1"
          onkeydown={(e) => {
            if (e.key === 'Enter' && !e.defaultPrevented) track()
          }}
        >
          <AutoCompleteInput
            id="train-no"
            bind:value={query}
            kind="train"
            placeholder="Train number or name…"
            aria-label="Train number or name"
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
          Auto <span class="data-num">30s</span>
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
    <TabBar cols={6}>
      <Tabs.Trigger value="status" title="Status" aria-label="Live status" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><ActivityIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Status</span></Tabs.Trigger>
      <Tabs.Trigger value="schedule" title="Schedule" aria-label="Schedule" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><CalendarClockIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Schedule</span></Tabs.Trigger>
      <Tabs.Trigger value="avg" title="Avg delay" aria-label="Avg delay" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><ChartColumnIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Avg delay</span></Tabs.Trigger>
      <Tabs.Trigger value="map" title="Map" aria-label="Map" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><MapIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Map</span></Tabs.Trigger>
      <Tabs.Trigger value="exceptions" title="Exceptions" aria-label="Exceptions" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><CalendarX2Icon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Exceptions</span></Tabs.Trigger>
      <Tabs.Trigger value="chart" title="Chart" aria-label="Chart vacancy" class="max-lg:justify-center max-lg:px-1 max-lg:py-2.5 max-lg:h-9"><BedDoubleIcon class="size-4 max-lg:size-5 shrink-0" /><span class="max-lg:hidden">Chart</span></Tabs.Trigger>
    </TabBar>

    <Tabs.Content value="status" class="mt-3 grid gap-4">
      <AsyncState
        phase={phase}
        error={errorMsg}
        empty={!data}
        skeletonCount={4}
        emptyIcon={ActivityIcon}
        emptyTitle="No train tracked yet"
        emptyHint="Enter a train number or name above and press Track to see live status."
      >
        {#if data}
          <Card.Root>
          <Card.Header class="flex flex-col items-start justify-between gap-3 space-y-0 sm:flex-row sm:items-center">
            <div class="grid gap-1">
              <Card.Title class="flex flex-wrap items-center gap-2">
                <EntityChip type="train" code={data.train_number} name={data.train_name} />
                <span>{data.train_name ?? ''}</span>
                <TrainDelayBadge number={data.train_number} name={data.train_name} />
              </Card.Title>
              <Card.Description>
                {#if runPosition}{runPosition}{:else}<span class="data-num">{statusRows.length}</span> stations on this run{/if}
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
              <StatusBadge tone={auto ? 'info' : 'outline'} dot={auto} class="max-lg:hidden">{#if auto}auto <span class="data-num">30s</span>{:else}manual{/if}</StatusBadge>
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
        {/if}
      </AsyncState>
    </Tabs.Content>

    <Tabs.Content value="schedule" class="mt-3 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={CalendarClockIcon}
          title="No schedule loaded"
          hint="Enter a train number above to load its schedule."
        />
      {:else}
        <AsyncState
          phase={schPhase}
          error={schErr}
          empty={!schData}
          skeletonCount={5}
          emptyIcon={CalendarClockIcon}
          emptyTitle="No schedule loaded"
          emptyHint="Track a train first, then its scheduled stops appear here."
        >
          {#if schData}
            <Card.Root>
          <Card.Header>
            <Card.Title class="flex flex-wrap items-center gap-2">
              <EntityChip type="train" code={schData.train_number} name={schData.train_name} />
              <span>{schData.train_name ?? ''}</span>
              <TrainDelayBadge number={schData.train_number} name={schData.train_name} />
            </Card.Title>
            <Card.Description><span class="data-num">{schData.stops?.length ?? 0}</span> scheduled stops</Card.Description>
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
          {/if}
        </AsyncState>
      {/if}
    </Tabs.Content>

    <Tabs.Content value="avg" class="mt-3 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={ChartColumnIcon}
          title="No delay data loaded"
          hint="Enter a train number above to load average delays."
        />
      {:else}
        <AsyncState
          phase={avgPhase}
          error={avgErr}
          empty={!avgData}
          skeletonCount={5}
          emptyIcon={ChartColumnIcon}
          emptyTitle="No delay data loaded"
          emptyHint="Track a train first, then its average delays appear here."
        >
          {#if avgData}
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
              Average arrival / departure delays{#if avgData.days_of_run} · runs: <span class="data-num">{avgData.days_of_run}</span>{/if}
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
          {/if}
        </AsyncState>
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
            <div class="grid min-w-0 sm:min-w-48 flex-1"
              onkeydown={(e) => {
                if (e.key === 'Enter' && !e.defaultPrevented) applyMapStation()
              }}
            >
              <AutoCompleteInput
                id="map-station"
                kind="station"
                bind:value={mapStation}
                placeholder="Boarding stop (optional) e.g. NDLS"
                aria-label="Boarding stop"
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
              <span class="data-num">{entries.length}</span> exception{entries.length === 1 ? '' : 's'}{excRoute ? ` · ${excRoute}` : ''}
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

    <Tabs.Content value="chart" class="mt-3 grid gap-4">
      {#if !committed}
        <EmptyState
          icon={BedDoubleIcon}
          title="No chart loaded"
          hint="Enter a train number above to see its vacant berths."
        />
      {:else}
        <Card.Root>
          <Card.Content class="flex flex-wrap items-end gap-3 max-lg:p-3">
            <div class="grid min-w-0 sm:min-w-40 flex-1">
              <label for="train-chart-date" class="text-xs font-medium text-muted-foreground">Journey date</label>
              <Input id="train-chart-date" type="date" bind:value={chartDate} aria-label="Journey date" class="h-9" />
            </div>
            <div class="grid min-w-0 sm:min-w-40 flex-1" onkeydown={(e) => { if (e.key === 'Enter' && !e.defaultPrevented) loadChart(committed) }}>
              <label for="train-chart-station" class="text-xs font-medium text-muted-foreground">Boarding station (optional)</label>
              <AutoCompleteInput
                id="train-chart-station"
                kind="station"
                bind:value={chartStation}
                placeholder="e.g. NDLS"
                aria-label="Boarding station"
                onpick={(item) => { if (asText(item?.code)) chartStation = asText(item.code).toUpperCase() }}
              />
            </div>
            <Button type="button" onclick={() => loadChart(committed)} disabled={chartPhase === 'loading'} class="shrink-0 max-lg:min-h-11 max-lg:w-full sm:w-auto">
              {chartPhase === 'loading' ? 'Loading…' : 'Get chart'}
            </Button>
            <Button type="button" variant="outline" onclick={() => navigate(`/chart/${encodeURIComponent(committed)}/${encodeURIComponent(chartDate)}${asText(chartStation) ? `/${encodeURIComponent(asText(chartStation).toUpperCase())}` : ''}`)} title="Open full chart page" class="shrink-0 max-lg:w-full">
              <LayoutGridIcon class="size-4" /> Full chart
            </Button>
          </Card.Content>
        </Card.Root>

        {#if chartPhase === 'loading'}
          <div class="grid gap-2" aria-busy="true">
            {#each [0,1,2] as i (i)}<Skeleton class="h-20 w-full" />{/each}
          </div>
        {:else if chartPhase === 'error'}
          <Alert.Root variant="destructive" role="alert">
            <Alert.Title>Could not load chart</Alert.Title>
            <Alert.Description class="[overflow-wrap:anywhere]">{chartFriendlyErr || chartErr}</Alert.Description>
          </Alert.Root>
        {:else if chartPhase === 'ok' && chartData}
          {@const coaches = Array.isArray(chartData.coaches) ? chartData.coaches : []}
          {@const isLocal = String(chartData.data_source ?? '').toLowerCase() === 'local'}
          {@const notice = asText(chartData.notice)}
          <Card.Root>
            <Card.Header class="gap-3 space-y-0">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <Card.Title class="flex flex-wrap items-center gap-2">
                  <EntityChip type="train" code={chartData.train_number} name={chartData.train_name} />
                  <span>{chartData.train_name ?? ''}</span>
                </Card.Title>
                <ResultMeta source={chartData.data_source} />
              </div>
              <Card.Description class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
                <span>Date <span class="data-num font-medium">{chartData.journey_date ?? chartDate}</span></span>
                {#if chartData.boarding_station}<span>Boarding <span class="data-num font-medium">{chartData.boarding_station}</span></span>{/if}
                <span><span class="data-num font-semibold">{coaches.length}</span> coaches</span>
              </Card.Description>
              {#if notice}
                <Alert.Root variant={isLocal ? 'default' : undefined} class="mt-1">
                  <Alert.Title>{isLocal ? 'Static empty (IRCTC unavailable)' : 'Notice'}</Alert.Title>
                  <Alert.Description class="[overflow-wrap:anywhere]">{notice}</Alert.Description>
                </Alert.Root>
              {/if}
            </Card.Header>
            <Card.Content class="grid gap-3">
              {#if isLocal && coaches.length === 0}
                <EmptyState icon={LayoutGridIcon} title="Chart not available" hint={notice || 'IRCTC is geofenced to India or chart not yet published (~4h before departure).'} />
              {:else if coaches.length === 0}
                <EmptyState icon={LayoutGridIcon} title="No coaches returned" hint="IRCTC returned no coach list — chart may not be prepared yet." />
              {:else}
                <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                  <span class="inline-flex items-center gap-1"><span class="size-3 rounded-sm bg-signal-go border"></span> Vacant</span>
                  <span class="inline-flex items-center gap-1"><span class="size-3 rounded-sm bg-muted border"></span> Occupied</span>
                  <span class="inline-flex items-center gap-1"><span class="size-3 rounded-sm bg-amber-100 border border-amber-200"></span> Other</span>
                </div>
                <div class="grid gap-2.5">
                  {#each coaches as coach (coach.code)}
                    {@const berths = Array.isArray(coach.berths) ? coach.berths : []}
                    {@const total = berths.length}
                    {@const vacant = berths.filter((b) => String(b?.status ?? '').toLowerCase() === 'vacant').length}
                    {@const occupied = berths.filter((b) => String(b?.status ?? '').toLowerCase() === 'occupied').length}
                    {@const isOpen = !!chartExpanded[coach.code]}
                    <div class="rounded-lg border bg-card overflow-hidden">
                      <button
                        type="button"
                        class="flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left hover:bg-accent/50 transition-colors"
                        onclick={() => chartToggleCoach(coach.code)}
                        aria-expanded={isOpen}
                      >
                        <div class="flex flex-wrap items-center gap-2 min-w-0">
                          <span class="data-num font-semibold">{coach.code || '—'}</span>
                          <span class="rounded border bg-muted px-1.5 py-0.5 text-xs">{coach.class_code || '—'}</span>
                          <span class="text-xs text-muted-foreground"><span class="data-num font-medium text-signal-go">{vacant}</span> vacant · <span class="data-num">{occupied}</span> occupied · <span class="data-num">{total}</span> berths</span>
                        </div>
                        <span class="text-xs text-muted-foreground shrink-0">{isOpen ? 'Hide' : 'Show'} berths</span>
                      </button>
                      {#if isOpen}
                        <div class="border-t px-3 py-2.5">
                          {#if total === 0}
                            <p class="text-xs text-muted-foreground">No berths in this coach.</p>
                          {:else}
                            <div class="grid grid-cols-[repeat(auto-fill,minmax(3.25rem,1fr))] gap-1.5">
                              {#each berths as b (b.number)}
                                <div class={`flex flex-col items-center justify-center rounded border px-1 py-1.5 text-center ${chartBerthClass(b.status)}`}>
                                  <span class="data-num text-xs font-semibold leading-none">{b.number}</span>
                                  <span class="mt-0.5 text-xs uppercase leading-none tracking-wide opacity-80">{asText(b.status) || 'unknown'}</span>
                                </div>
                              {/each}
                            </div>
                          {/if}
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              {/if}
            </Card.Content>
          </Card.Root>
        {:else}
          <EmptyState icon={BedDoubleIcon} title="Chart vacancy" hint="Pick a journey date and boarding station (optional) and press Get chart. Live from IRCTC online-charts; chart appears only after preparation (~4h before, previous evening for early trains). IP-geofenced to India." />
        {/if}
      {/if}
    </Tabs.Content>
  </Tabs.Root>

  <BottomSpacer />
</section>
