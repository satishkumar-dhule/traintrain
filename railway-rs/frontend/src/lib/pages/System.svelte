<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import SourceStatus from '$lib/SourceStatus.svelte'
  import SignalDot from '$lib/components/SignalDot.svelte'
  import PageHeader from '$lib/components/PageHeader.svelte'
  import FilterChipGroup from '$lib/components/FilterChipGroup.svelte'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { LogLevelBadge, CountBadge } from '$lib/components/badges/index.js'
  import Activity from 'lucide-svelte/icons/activity'
  import RefreshCw from 'lucide-svelte/icons/refresh-cw'
  import Clock from 'lucide-svelte/icons/clock'
  import Gauge from 'lucide-svelte/icons/gauge'
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle'
  import DataTable from '$lib/components/DataTable.svelte'
  import TrackRule from '$lib/components/TrackRule.svelte'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'
  import { fmtUptime, humanBytes } from '$lib/format.js'
  import {
    num,
    memMb,
    pctFromFrac,
    latest,
    seriesRange,
    sparkPoints,
    logLine,
    tsTime,
    sortedLogs,
    codeClass,
    cacheValue,
    updatedTime
  } from '$lib/metrics.js'

  let obs = $state({ phase: 'loading', data: null })
  let logsState = $state({ phase: 'loading', data: null })
  let auto = $state(false)
  let busy = $state(false)
  let note = $state('')
  let updatedAt = $state(null)
  let logFilter = $state('all')

  async function loadAll() {
    busy = true
    const [o, l] = await Promise.all([
      api('/rail-api/observability'),
      api('/rail-api/logs?limit=25')
    ])
    const failures = []
    if (o.ok) {
      obs = { phase: 'ok', data: o.data }
      updatedAt = new Date().toISOString()
    } else {
      failures.push(`observability: ${o.error}`)
      if (obs?.data == null) obs = { phase: 'error', data: null }
    }
    if (l.ok) {
      logsState = { phase: 'ok', data: l.data }
    } else {
      failures.push(`logs: ${l.error}`)
      if (logsState?.data == null) logsState = { phase: 'error', data: null }
    }
    note = failures.length ? `last refresh failed · ${failures.join(' · ')}` : ''
    busy = false
  }

  $effect(() => {
    untrack(() => loadAll())
  })

  $effect(() => {
    if (!auto) return
    const timer = setInterval(() => {
      untrack(() => loadAll())
    }, 10000)
    return () => clearInterval(timer)
  })

  // ---- shared spark config (single source, fan-out to 4 cards) ----
  const sparks = [
    {
      key: 'rps',
      label: 'Request rate',
      fmt: (v) => (num(v) === null ? '—' : String(num(v))),
      rangeFmt: (v) => String(v),
      bar: 'bg-chart-1'
    },
    {
      key: 'latency_ms',
      label: 'Latency',
      fmt: (v) => (num(v) === null ? '—' : `${num(v)} ms`),
      rangeFmt: (v) => `${v} ms`,
      bar: 'bg-chart-2'
    },
    {
      key: 'mem_mb',
      label: 'Memory',
      fmt: (v) => (num(v) === null ? '—' : `${num(v)} MB`),
      rangeFmt: (v) => `${v} MB`,
      bar: 'bg-chart-3'
    },
    {
      key: 'cpu_frac',
      label: 'CPU',
      fmt: pctFromFrac,
      rangeFmt: (v) => `${(v * 100).toFixed(1)}%`,
      bar: 'bg-chart-4'
    }
  ]

  const LOG_FILTER_OPTS = [
    { value: 'all', label: 'All' },
    { value: 'warn', label: 'Warn+' },
    { value: 'errors', label: 'Errors' }
  ]

  /* Aggregated status-code distribution + derived error rate. All values
     come straight from the server counters; guards handle empty data. */
  const codeAgg = $derived.by(() => {
    if (obs.phase !== 'ok') return { entries: [], total: 0, errors: 0 }
    const raw = obs.data?.status_codes
    const arr = Array.isArray(raw) ? raw : []
    const acc = new Map()
    for (const item of arr) {
      let code = null
      let count = null
      if (Array.isArray(item) && item.length >= 2) {
        code = item[0]
        count = item[1]
      } else if (item && typeof item === 'object') {
        code = item.code ?? item.status ?? item.key
        count = item.count ?? item.value
      }
      const c = parseInt(String(code), 10)
      const n = Number(count)
      if (!Number.isFinite(c) || !Number.isFinite(n) || n <= 0) continue
      acc.set(c, (acc.get(c) ?? 0) + n)
    }
    let total = 0
    let errors = 0
    for (const [c, n] of acc) {
      total += n
      if (c >= 500) errors += n
    }
    const entries = [...acc.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([code, count]) => ({
        code,
        count,
        cls: codeClass(code),
        pct: total > 0 ? (count / total) * 100 : 0
      }))
    return { entries, total, errors }
  })

  const statusBars = $derived(codeAgg.entries)

  const errorRatePct = $derived(
    codeAgg.total > 0 ? (codeAgg.errors / codeAgg.total) * 100 : null
  )

  const cacheHitRate = $derived.by(() => {
    if (obs.phase !== 'ok') return null
    const c = obs.data?.cache
    if (!c || typeof c !== 'object') return null
    const hits = num(c.hits) ?? 0
    const misses = num(c.misses) ?? 0
    const lookups = hits + misses
    if (lookups <= 0) return null
    return (hits / lookups) * 100
  })

  /* Primary overview band: uptime, traffic, speed, failures. */
  const heroTiles = $derived.by(() => {
    if (obs.phase !== 'ok' || !obs.data || typeof obs.data !== 'object') return []
    const d = obs.data
    const errTone =
      errorRatePct === null ? 'muted' : errorRatePct > 5 ? 'stop' : errorRatePct > 0 ? 'hold' : 'go'
    return [
      {
        icon: Clock,
        label: 'Uptime',
        value: fmtUptime(d.uptime_secs),
        sub: 'since process start',
        tone: 'primary'
      },
      {
        icon: Activity,
        label: 'Total requests',
        value: num(d.requests_total)?.toLocaleString() ?? '—',
        sub: 'counted by the metrics middleware',
        tone: 'chart'
      },
      {
        icon: Gauge,
        label: 'Avg req / sec',
        value: num(d.req_per_sec)?.toFixed(2) ?? '—',
        sub: 'averaged over uptime',
        tone: 'go'
      },
      {
        icon: AlertTriangle,
        label: 'Error rate',
        value: errorRatePct === null ? '—' : `${errorRatePct.toFixed(1)}%`,
        sub:
          codeAgg.total > 0
            ? `${codeAgg.errors.toLocaleString()} of ${codeAgg.total.toLocaleString()} are 5xx`
            : 'no responses recorded yet',
        tone: errTone
      }
    ]
  })

  /* Dense secondary strip beneath the overview band. */
  const runtimeStats = $derived.by(() => {
    if (obs.phase !== 'ok' || !obs.data) return []
    const d = obs.data
    return [
      ['CPU', pctFromFrac(d.cpu_usage)],
      ['Memory', num(d.mem_usage) !== null ? `${memMb(d.mem_usage)} MB` : '—'],
      ['In-flight', num(d.active_connections)?.toLocaleString() ?? '—'],
      ['Bytes out', humanBytes(d.bytes_out)],
      ['Cache entries', num(d.cache?.entries)?.toLocaleString() ?? '—'],
      ['Cache hit rate', cacheHitRate === null ? '—' : `${cacheHitRate.toFixed(1)}%`]
    ]
  })

  const paths = $derived(
    obs.phase === 'ok' && Array.isArray(obs.data?.top_paths)
      ? obs.data.top_paths.slice(0, 8).filter((p) => Array.isArray(p) && p.length >= 2)
      : []
  )

  const origins = $derived(
    obs.phase === 'ok' && Array.isArray(obs.data?.origins) ? obs.data.origins : []
  )

  const maxOriginLatency = $derived(Math.max(0, ...origins.map((o) => num(o.latency) ?? 0)))

  function originLatPct(v) {
    const lat = num(v)
    if (lat === null || maxOriginLatency <= 0) return 0
    return Math.min(100, (lat / maxOriginLatency) * 100)
  }

  const logsNewest = $derived(logsState.phase === 'ok' ? sortedLogs(logsState.data?.logs) : [])

  const cacheEntries = $derived.by(() => {
    if (obs.phase !== 'ok') return []
    const c = obs.data?.cache
    if (Array.isArray(c)) {
      return c
        .filter((p) => Array.isArray(p) && p.length >= 2)
        .map((p) => ({ key: String(p[0]), label: String(p[0]).replace(/_/g, ' '), value: cacheValue(p[0], p[1]) }))
    }
    if (c && typeof c === 'object') {
      return Object.entries(c).map(([k, v]) => ({
        key: k,
        label: String(k).replace(/_/g, ' '),
        value: cacheValue(k, v)
      }))
    }
    return []
  })

  const logsFiltered = $derived.by(() => {
    if (logFilter === 'all') return logsNewest
    return logsNewest.filter((l) => {
      const lvl = String(l?.level ?? '').toLowerCase()
      if (logFilter === 'errors') return lvl.includes('error') || lvl.includes('fatal')
      return lvl.includes('warn') || lvl.includes('error') || lvl.includes('fatal')
    })
  })

  const originCols = [
    { key: 'name', label: 'Origin', cellClass: 'font-medium', value: (o) => o.name },
    {
      key: 'requests',
      label: 'Requests',
      class: 'w-28',
      cellClass: 'data-num text-xs',
      value: (o) => (num(o.requests) ?? 0).toLocaleString(),
      sortValue: (o) => num(o.requests) ?? 0
    },
    {
      key: 'latency',
      label: 'Latency',
      class: 'w-44',
      sortValue: (o) => num(o.latency) ?? 0
    },
    { key: 'status', label: 'Status', class: 'w-28', value: (o) => String(o.status ?? '') }
  ]

  const logCols = [
    { key: 'time', label: 'Time', class: 'w-28', cellClass: 'data-num text-xs max-lg:text-sm', value: (l) => tsTime(l?.ts), sortValue: (l) => num(l?.ts) },
    { key: 'level', label: 'Level', class: 'w-24', value: (l) => String(l?.level ?? '').toLowerCase() },
    { key: 'event', label: 'Event', cellClass: 'max-w-md truncate data-num text-xs', value: (l) => logLine(l) }
  ]

  /* Icon-chip tones keyed by identity tokens: primary, charts, signal lamps. */
  const toneChip = {
    primary: 'bg-primary/10 text-primary',
    chart: 'bg-chart-3/10 text-chart-3',
    go: 'bg-signal-go/10 text-signal-go-ink',
    hold: 'bg-signal-hold/10 text-signal-hold-ink',
    stop: 'bg-signal-stop/10 text-signal-stop-ink',
    muted: 'bg-muted text-muted-foreground'
  }

  // reusable compact card pattern (establishes reusability, single source)
  const COMPACT_HERO_CARD = 'gap-1 py-2 lg:gap-2 lg:py-4'
  // section band headers share one micro-label treatment
  const MICRO_LABEL = 'text-xs font-semibold uppercase tracking-wider text-muted-foreground'
  const COMPACT_HERO_TITLE = 'text-[10px] font-medium uppercase tracking-wide text-muted-foreground lg:text-xs'
  const COMPACT_HERO_VALUE = 'data-num text-lg font-semibold tabular-nums lg:text-2xl'

  // ---- super n2 deep delegation: ONE handler fans out to N groups × M items = N·M elements ----
  // Instead of N·M closures (one per row/tile/chip), a single listener at the section root
  // reads data-* attributes and dispatches. This is O(1) listeners vs O(N·M) closures:
  // 4 hero tiles + 6 stat tiles + 8 path rows + 5 status chips = 23 elements → 1 handler.
  // Each branch returns early, so dispatch is O(1) per event (no loops beyond closest()).
  function handleSystemAction(e) {
    // 1) top paths — copy path string (8 rows)
    const pathEl = e.target.closest?.('[data-path]')
    if (pathEl) {
      const p = pathEl.getAttribute('data-path')
      if (p && navigator.clipboard?.writeText) navigator.clipboard.writeText(p).catch(() => {})
      return
    }
    // 2) runtime stat tile — copy value (6 tiles)
    const statEl = e.target.closest?.('[data-stat]')
    if (statEl) {
      const v = statEl.getAttribute('data-stat-value')
      if (v && v !== '—' && navigator.clipboard?.writeText) navigator.clipboard.writeText(v).catch(() => {})
      return
    }
    // 3) status legend — filter logs (N chips)
    const statusEl = e.target.closest?.('[data-status-code]')
    if (statusEl) {
      const code = Number(statusEl.getAttribute('data-status-code'))
      if (Number.isFinite(code)) {
        if (code >= 500) logFilter = 'errors'
        else if (code >= 400) logFilter = 'warn'
        else logFilter = 'all'
        document.getElementById('system-logs')?.scrollIntoView({ behavior: 'smooth', block: 'start' })
      }
      return
    }
    // 4) hero tile — copy hero value (4 tiles)
    const heroEl = e.target.closest?.('[data-hero]')
    if (heroEl) {
      const v = heroEl.getAttribute('data-hero-value')
      if (v && v !== '—' && navigator.clipboard?.writeText) navigator.clipboard.writeText(v).catch(() => {})
      return
    }
  }

  function handleSystemKey(e) {
    if (e.key !== 'Enter' && e.key !== ' ') return
    // delegate the same as click but via keyboard on focused data-* element
    const target = e.target?.closest?.('[data-path],[data-stat],[data-status-code],[data-hero]') ?? e.target
    if (!target) return
    // synthesize a click-like dispatch through the same handler
    const fake = { target, closest: target.closest?.bind(target) }
    // manually check which data attr the focused element carries
    if (target.hasAttribute?.('data-path') || target.closest?.('[data-path]')) {
      e.preventDefault()
      handleSystemAction(e)
    } else if (target.hasAttribute?.('data-stat') || target.closest?.('[data-stat]')) {
      e.preventDefault()
      handleSystemAction(e)
    } else if (target.hasAttribute?.('data-status-code') || target.closest?.('[data-status-code]')) {
      e.preventDefault()
      handleSystemAction(e)
    } else if (target.hasAttribute?.('data-hero') || target.closest?.('[data-hero]')) {
      e.preventDefault()
      handleSystemAction(e)
    }
  }
</script>

{#snippet latencyCell(o)}
  <div class="flex items-center gap-2">
    <span
      class={`w-16 shrink-0 text-right data-num text-xs ${(num(o.latency) ?? 0) <= 0 ? 'text-muted-foreground' : ''}`}
    >
      {o.latency} ms
    </span>
    <div class="h-1.5 min-w-12 flex-1 overflow-hidden rounded-full bg-muted">
      <div class="h-full rounded-full bg-primary" style={`width:${originLatPct(o.latency)}%`}></div>
    </div>
  </div>
{/snippet}

{#snippet originStatusCell(o)}
  <span class="inline-flex items-center gap-1.5 text-xs">
    {#if o.status === 'up' || o.status === 'reachable'}
      <SignalDot tone="go" pulse />
      <span class="text-signal-go-ink">up</span>
    {:else}
      <SignalDot tone="stop" />
      <span class="text-signal-stop-ink">{o.status}</span>
    {/if}
  </span>
{/snippet}

{#snippet logLevelCell(l)}
  <LogLevelBadge level={l?.level} />
{/snippet}

<!-- super deep delegation root: ONE click + ONE keydown fan-out to ~23 interactive elements -->
<section class="grid gap-6" onclick={handleSystemAction} onkeydown={handleSystemKey}>
  <!-- Unified header via PageHeader — single responsive primitive, no viewport duplication (fan-out) -->
  <PageHeader title="System" description="Runtime metrics and recent request logs — real numbers only.">
    {#snippet children()}
      {#if note}
        <p class="text-xs text-destructive">{note}</p>
      {/if}
    {/snippet}
    {#snippet actions()}
      <div class="flex flex-wrap items-center gap-2">
        {#if updatedAt && !note}
          <span class="hidden text-xs text-muted-foreground data-num sm:inline">
            Updated {updatedTime(updatedAt)}
          </span>
        {/if}
        <Button type="button" variant="outline" size="sm" onclick={() => loadAll()} disabled={busy}>
          <RefreshCw class={`mr-2 size-4${busy ? ' animate-spin' : ''}`} />
          {busy ? 'Refreshing…' : 'Refresh'}
        </Button>
        <label class="flex min-h-11 cursor-pointer items-center gap-2 text-sm text-muted-foreground">
          <input type="checkbox" bind:checked={auto} class="size-5 accent-[var(--primary)]" />
          <span class="hidden sm:inline">Auto 10s</span><span class="sm:hidden">Auto</span>
        </label>
      </div>
    {/snippet}
  </PageHeader>

  <TrackRule />

  <div class="grid gap-2">
    <h2 class={MICRO_LABEL}>Overview</h2>
    {#if obs.phase === 'loading'}
      <div class="grid grid-cols-2 gap-3 lg:grid-cols-4" aria-busy="true">
        {#each Array.from({ length: 4 }, (_, i) => i) as i (i)}
          <Skeleton class="h-24" />
        {/each}
      </div>
    {:else if obs.phase === 'error'}
      <Alert.Root variant="destructive">
        <Alert.Title>Observability unavailable</Alert.Title>
        <Alert.Description>Could not load /rail-api/observability.</Alert.Description>
      </Alert.Root>
    {:else}
      <div class="grid grid-cols-2 gap-2 lg:grid-cols-4 lg:gap-3">
        {#each heroTiles as t (t.label)}
          <Card.Root
            class={`${COMPACT_HERO_CARD} cursor-pointer hover:ring-1 hover:ring-primary/20 transition-shadow`}
            data-hero={t.label}
            data-hero-value={t.value}
            role="button"
            tabindex="0"
            title="Copy {t.label}: {t.value}"
          >
            <Card.Content class="flex items-start justify-between gap-2 px-3 lg:px-4">
              <div class="grid gap-0.5 lg:gap-1">
                <Card.Title class={COMPACT_HERO_TITLE}>{t.label}</Card.Title>
                <Card.Description class={COMPACT_HERO_VALUE}>{t.value}</Card.Description>
              </div>
              <span class={`rounded-md p-1.5 lg:p-2 ${toneChip[t.tone] ?? toneChip.slate}`} aria-hidden="true">
                <t.icon class="size-3.5 lg:size-4" />
              </span>
            </Card.Content>
            <Card.Content class="px-3 lg:px-4">
              <p class="text-[10px] leading-tight text-muted-foreground lg:text-[11px]">{t.sub}</p>
            </Card.Content>
          </Card.Root>
        {/each}
      </div>

      {#if runtimeStats.length}
        <div
          class="grid grid-cols-2 gap-px overflow-hidden rounded-lg border sm:grid-cols-3 lg:grid-cols-6"
          style="background:var(--border);border-color:var(--border);"
          role="group"
          aria-label="Runtime stats — click a value to copy (delegated)"
        >
          {#each runtimeStats as [label, value] (label)}
            <div
              class="grid gap-0.5 bg-card px-2 py-1.5 lg:px-3 lg:py-2.5 cursor-pointer hover:bg-muted/50 transition-colors"
              data-stat={label}
              data-stat-value={value}
              title="Copy {label}: {value}"
              role="button"
              tabindex="0"
            >
              <span class="text-[9px] font-medium uppercase tracking-wide text-muted-foreground lg:text-[10px]">{label}</span>
              <span class="truncate data-num text-xs font-semibold lg:text-sm" title={value}>{value}</span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  <div class="grid gap-2">
    <h2 class={MICRO_LABEL}>Traffic</h2>
    {#if obs.phase === 'loading'}
      <div class="grid grid-cols-2 gap-2 lg:grid-cols-4 lg:gap-3" aria-busy="true">
        {#each Array.from({ length: 4 }, (_, i) => i) as i (i)}
          <Skeleton class="h-28 lg:h-32" />
        {/each}
      </div>
    {:else if obs.phase === 'ok'}
      <div class="grid grid-cols-2 gap-2 lg:grid-cols-4 lg:gap-3">
        {#each sparks as s (s.key)}
          {@const seriesArr = obs.data?.series?.[s.key]}
          {@const pts = sparkPoints(seriesArr)}
          {@const lastVal = latest(seriesArr)}
          {@const range = seriesRange(seriesArr)}
          <Card.Root class="gap-1 py-2 lg:gap-2 lg:py-4">
            <Card.Title class="px-3 text-[10px] font-medium uppercase tracking-wide text-muted-foreground lg:px-4 lg:text-xs">{s.label}</Card.Title>
            <Card.Description class="px-3 data-num text-base font-semibold lg:px-4 lg:text-lg">{s.fmt(lastVal)}</Card.Description>
            <Card.Content class="px-3 lg:px-4">
              {#if pts.length}
                <div class="flex h-10 items-end gap-[2px] lg:h-14" role="img" aria-label={`${s.label} sparkline, last ${pts.length} samples`}>
                  {#each pts as p, i (i)}
                    <div
                      class={`min-w-[2px] flex-1 rounded-sm ${s.bar}`}
                      style={`height:${p.pct}%;opacity:${p.op};${p.pct > 0 ? 'min-height:2px;' : ''}`}
                    ></div>
                  {/each}
                </div>
                <p class="mt-1 text-[9px] text-muted-foreground lg:mt-2 lg:text-[10px]">
                  {#if range}
                    min <span class="data-num">{s.rangeFmt(range.min)}</span> · max
                    <span class="data-num">{s.rangeFmt(range.max)}</span> · last
                    <span class="data-num">{pts.length}</span> samples
                  {:else}
                    last <span class="data-num">{pts.length}</span> samples ·
                    <span class="data-num">2s</span> interval
                  {/if}
                </p>
              {:else}
                <p class="py-2 text-center text-[10px] text-muted-foreground lg:py-4 lg:text-xs">no series data yet</p>
              {/if}
            </Card.Content>
          </Card.Root>
        {/each}
      </div>
    {/if}
  </div>

  <div class="grid gap-2">
    <h2 class={MICRO_LABEL}>Upstream</h2>
    {#if origins.length}
      <Card.Root>
        <Card.Header class="max-lg:p-3">
          <Card.Title class="text-sm lg:text-base">Upstream origins</Card.Title>
          <Card.Description class="max-lg:text-xs">Served request counts, latency and reachability per data source.</Card.Description>
        </Card.Header>
        <Card.Content class="max-lg:p-3">
          <DataTable
            columns={originCols}
            rows={origins}
            primary="name"
            rowKey={(o) => o.name}
            cells={{ status: originStatusCell, latency: latencyCell }}
            empty="No upstream origins reported."
          />
        </Card.Content>
      </Card.Root>
    {/if}
    <SourceStatus />
  </div>

  {#if obs.phase === 'ok'}
    <div class="grid gap-2">
      <h2 class={MICRO_LABEL}>Application</h2>
      <div class="grid gap-2 lg:grid-cols-2 lg:gap-3">
        <Card.Root>
          <Card.Header class="max-lg:p-3">
            <Card.Title class="text-sm lg:text-base">Top paths</Card.Title>
            <Card.Description class="max-lg:text-xs">Most requested routes since process start. Click a path to copy.</Card.Description>
          </Card.Header>
          <Card.Content class="max-lg:p-3">
            {#if paths.length}
              <ol class="grid gap-1.5" role="list" aria-label="Top paths — click to copy (delegated)">
                {#each paths as p, i (p[0])}
                  <li
                    class="flex items-center gap-3 rounded-md px-2 py-1 -mx-2 cursor-pointer hover:bg-muted/50 transition-colors"
                    data-path={String(p[0])}
                    title="Copy {String(p[0])}"
                    role="listitem"
                    tabindex="0"
                  >
                    <span class="w-5 text-right data-num text-xs text-muted-foreground">{i + 1}.</span>
                    <span class="min-w-0 flex-1 truncate data-num text-xs">{String(p[0])}</span>
                    <CountBadge value={Number(p[1])} />
                  </li>
                {/each}
              </ol>
            {:else}
              <p class="text-sm text-muted-foreground">No request counts recorded yet.</p>
            {/if}
          </Card.Content>
        </Card.Root>

        <Card.Root>
          <Card.Header class="max-lg:p-3">
            <Card.Title class="text-sm lg:text-base">Status distribution</Card.Title>
            <Card.Description class="max-lg:text-xs">Response counts per HTTP status code. Click a code to filter logs.</Card.Description>
          </Card.Header>
          <Card.Content class="grid gap-2 p-3 lg:gap-3 lg:p-6">
            {#if statusBars.length}
              <div class="flex h-3 w-full overflow-hidden rounded-full" role="img" aria-label="Status code distribution stacked bar">
                {#each statusBars as s (s.code)}
                  <div
                    class={`${s.cls} min-w-[2px]`}
                    style={`width:${Math.max(s.pct, 0.5)}%`}
                    title={`${s.code} × ${s.count.toLocaleString()} (${s.pct.toFixed(1)}%)`}
                  ></div>
                {/each}
              </div>
              <div class="flex flex-wrap gap-x-3 gap-y-1" role="group" aria-label="Filter logs by status code (delegated)">
                {#each statusBars as s (`legend-${s.code}`)}
                  <button
                    type="button"
                    class="flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                    data-status-code={s.code}
                    title="Filter logs for {s.code}"
                  >
                    <span class={`inline-block size-2 rounded-sm ${s.cls}`} aria-hidden="true"></span>
                    <span class="data-num">{s.code}</span> × <span class="data-num">{s.count.toLocaleString()}</span>
                  </button>
                {/each}
              </div>
              <p class="text-xs text-muted-foreground"><span class="data-num">{codeAgg.total.toLocaleString()}</span> responses recorded</p>
            {:else}
              <p class="text-sm text-muted-foreground">No responses recorded yet.</p>
            {/if}
          </Card.Content>
        </Card.Root>
      </div>

      <Card.Root>
        <Card.Header class="max-lg:p-3">
          <Card.Title class="text-sm lg:text-base">Cache</Card.Title>
          <Card.Description class="max-lg:text-xs">Live cache counters reported by the server.</Card.Description>
        </Card.Header>
        <Card.Content class="max-lg:p-3">
          {#if cacheEntries.length}
            <dl class="grid gap-x-4 gap-y-1.5 sm:grid-cols-2 lg:gap-x-6 lg:gap-y-2 lg:grid-cols-3">
              {#each cacheEntries as e (e.key)}
                <div class="flex items-baseline justify-between gap-2 border-b pb-1 lg:gap-3">
                  <dt class="text-[10px] uppercase tracking-wide text-muted-foreground lg:text-xs">{e.label}</dt>
                  <dd class="truncate data-num text-xs font-medium max-lg:whitespace-normal max-lg:break-words lg:text-sm" title={e.value}>{e.value}</dd>
                </div>
              {/each}
            </dl>
          {:else}
            <p class="text-xs text-muted-foreground lg:text-sm">no cache stats reported</p>
          {/if}
        </Card.Content>
      </Card.Root>
    </div>
  {/if}

  <div class="grid gap-2" id="system-logs">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <h2 class={MICRO_LABEL}>Logs</h2>
      <!-- super fan-out: reusable FilterChipGroup owns its own n2 delegation internally; System fans out to it -->
      <FilterChipGroup options={LOG_FILTER_OPTS} active={logFilter} onToggle={(v) => (logFilter = v)} />
    </div>
    <Card.Root>
      <Card.Header class="max-lg:p-3">
        <Card.Title class="text-sm lg:text-base">Recent logs</Card.Title>
        <Card.Description class="max-lg:text-xs">Newest first, in-memory ring buffer.</Card.Description>
      </Card.Header>
      <Card.Content class="max-lg:p-3">
        {#if logsState.phase === 'loading'}
          <div class="grid gap-2" aria-busy="true">
            {#each Array.from({ length: 4 }, (_, i) => i) as i (i)}
              <Skeleton class="h-8 w-full" />
            {/each}
          </div>
        {:else if logsState.phase === 'error'}
          <Alert.Root variant="destructive">
            <Alert.Title>Logs unavailable</Alert.Title>
            <Alert.Description>Could not load /rail-api/logs?limit=25.</Alert.Description>
          </Alert.Root>
        {:else}
          <DataTable
            columns={logCols}
            rows={logsFiltered}
            primary="event"
            titleText={(l) => logLine(l)}
            rowKey={(l, i) => `${tsTime(l?.ts)}-${i}`}
            cells={{ level: logLevelCell }}
            empty={logFilter === 'all' ? 'No log entries.' : `No ${logFilter === 'errors' ? 'error' : 'warning'} entries.`}
          />
        {/if}
      </Card.Content>
    </Card.Root>
  </div>
</section>
<BottomSpacer />
