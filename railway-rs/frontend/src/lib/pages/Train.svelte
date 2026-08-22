<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import {
  TrainNumberBadge,
  StationCodeBadge,
  DelayBadge,
  RunsOnBadges,
  HaltStatusBadge,
  parseDelayMinutes
} from '$lib/components/badges/index.js'
import ActivityIcon from 'lucide-svelte/icons/activity'
  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'
  import ChartColumnIcon from 'lucide-svelte/icons/chart-no-axes-column'

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

  /* Run-instance selection (the NTES "start date" tabs): index into
     data.instances, plus whether the user manually picked one so a 30s
     auto-refresh doesn't yank them back to the active run. */
  let runIdx = $state(0)
  let runTouched = $state(false)

  const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
  const VIEW_TO_TAB = { status: 'status', schedule: 'schedule', delay: 'avg' }
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
      if (fresh) runTouched = false
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

  function track(n) {
    const t = String(n ?? query ?? '').trim()
    if (!t) return
    committed = t
    const want = `/train/${encodeURIComponent(t)}/${VIEW_TO_TAB[activeTab] ?? 'status'}`
    if (route.path !== want) navigate(want)
    loadStatus(t)
  }

  function onTabChange(v) {
    if (!committed) return
    navigate(`/train/${committed}/${VIEW_TO_TAB[v] ?? 'status'}`)
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

  const statusCols = [
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

  const schedCols = [
    { key: 'code', label: 'Code', cellClass: 'font-mono text-xs', value: (s) => s.code },
    { key: 'station', label: 'Station', cellClass: 'font-medium', value: (s) => s.name },
    { key: 'arrival', label: 'Arr', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.arrival) },
    { key: 'departure', label: 'Dep', cellClass: 'font-mono text-xs', value: (s) => fmtTime(s.departure) },
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
      sortValue: (s) => numOrNull(s.arrival_delay),
    },
    {
      key: 'departure_delay',
      label: 'Departure delay',
      value: (s) => delayLabel(s.departure_delay),
      sortValue: (s) => numOrNull(s.departure_delay),
    },
  ]

  function maxDelay(stations) {
    let m = 0
    for (const s of stations ?? []) {
      for (const v of [s.arrival_delay, s.departure_delay]) {
        const n = Number(v)
        if (Number.isFinite(n) && n > m) m = n
      }
    }
    return m
  }

  function barPct(v, m) {
    const n = Number(v)
    if (!Number.isFinite(n)) return null
    if (m <= 0) return 0
    return Math.min(100, (n / m) * 100)
  }

  function delayLabel(v) {
    const n = Number(v)
    if (!Number.isFinite(n)) return '—'
    return `${Math.round(n)}m`
  }
</script>

{#snippet statusStationCell(s)}
  <span class="font-medium">{s.name}</span>
  <span class="ml-2 font-mono text-xs text-muted-foreground">{s.code}</span>
{/snippet}

{#snippet statusDelayCell(s)}
  {@const d = delayBadge(s.delay_minutes)}
  <Badge variant={d.variant}>{d.text}</Badge>
{/snippet}

{#snippet statusStatusCell(s)}
  {@const st = String(s.status ?? '')}
  {#if st === 'departed'}
    <Badge variant="secondary">departed</Badge>
  {:else if st === 'expected'}
    <Badge>expected</Badge>
  {:else if st}
    <Badge variant="outline">{st}</Badge>
  {:else}
    <span class="text-muted-foreground">—</span>
  {/if}
{/snippet}

{#snippet avgStationCell(s)}
  <span class="font-medium">{s.name}</span>
  <span class="ml-2 font-mono text-xs text-muted-foreground">{s.code}</span>
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

  <Tabs.Root bind:value={activeTab} onValueChange={onTabChange}>
    <Tabs.List class="w-full justify-start">
      <Tabs.Trigger value="status"><ActivityIcon class="mr-2 size-4" />Status</Tabs.Trigger>
      <Tabs.Trigger value="schedule"><CalendarClockIcon class="mr-2 size-4" />Schedule</Tabs.Trigger>
      <Tabs.Trigger value="avg"><ChartColumnIcon class="mr-2 size-4" />Avg delay</Tabs.Trigger>
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
              <Card.Title>
                <span class="font-mono">{data.train_number ?? '—'}</span> · {data.train_name ?? ''}
              </Card.Title>
              <Card.Description>
                {#if runPosition}{runPosition}{:else}{statusRows.length} stations on this run{/if}
              </Card.Description>
            </div>
            <Badge variant={auto ? 'default' : 'outline'}>{auto ? 'auto 30s' : 'manual'}</Badge>
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
            <Card.Title>
              <span class="font-mono">{schData.train_number ?? '—'}</span> · {schData.train_name ?? ''}
            </Card.Title>
            <Card.Description>{schData.stops?.length ?? 0} scheduled stops</Card.Description>
          </Card.Header>
          <Card.Content class="grid gap-4">
            <div class="flex flex-wrap items-center gap-1.5">
              {#each DAY_LETTERS as letter, i (i)}
                {#if schData.running_days?.[i]}
                  <Badge variant="default">{letter}</Badge>
                {:else}
                  <Badge variant="outline" class="text-muted-foreground opacity-60">{letter}</Badge>
                {/if}
              {/each}
            </div>
            <DataTable
              columns={schedCols}
              rows={schData.stops ?? []}
              rowKey={(s, i) => `${i}-${s?.code ?? ''}-${s?.name ?? ''}`}
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
            <Card.Title>
              <span class="font-mono">{avgData.train_no ?? '—'}</span> · {avgData.train_name ?? ''}
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
  </Tabs.Root>
</section>
