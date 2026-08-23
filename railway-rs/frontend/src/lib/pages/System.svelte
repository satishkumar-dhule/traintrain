<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import SourceStatus from '$lib/SourceStatus.svelte'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { StatusBadge, LogLevelBadge, CountBadge } from '$lib/components/badges/index.js'
  import Activity from 'lucide-svelte/icons/activity'
  import RefreshCw from 'lucide-svelte/icons/refresh-cw'
  import Clock from 'lucide-svelte/icons/clock'
  import Gauge from 'lucide-svelte/icons/gauge'
  import AlertTriangle from 'lucide-svelte/icons/alert-triangle'
  import DataTable from '$lib/components/DataTable.svelte'

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

  function num(v) {
    const n = Number(v)
    return Number.isFinite(n) ? n : null
  }

  function fmtUptime(s) {
    const t = num(s)
    if (t === null || t < 0) return '—'
    const d = Math.floor(t / 86400)
    const h = Math.floor((t % 86400) / 3600)
    const m = Math.floor((t % 3600) / 60)
    if (d >= 1) return `${d}d ${h % 24}h ${m}m`
    if (h > 0) return `${h}h ${m}m`
    return `${m}m ${Math.floor(t % 60)}s`
  }

  function humanBytes(n) {
    const b = num(n)
    if (b === null || b < 0) return '—'
    if (b < 1024) return `${b} B`
    const kb = b / 1024
    if (kb < 1024) return `${kb.toFixed(1)} KB`
    const mb = kb / 1024
    if (mb < 1024) return `${mb.toFixed(1)} MB`
    return `${(mb / 1024).toFixed(2)} GB`
  }

  function memMb(n) {
    const b = num(n)
    if (b === null) return '—'
    return (b / (1024 * 1024)).toFixed(1)
  }

  function pctFromFrac(v) {
    const f = num(v)
    if (f === null) return '—'
    return `${(f * 100).toFixed(1)}%`
  }

  function latest(arr) {
    if (!Array.isArray(arr)) return null
    for (let i = arr.length - 1; i >= 0; i--) {
      const n = num(arr[i])
      if (n !== null) return n
    }
    return null
  }

  function seriesRange(arr) {
    if (!Array.isArray(arr)) return null
    const vals = arr.slice(-60).map(num).filter((v) => v !== null)
    if (!vals.length) return null
    return { min: Math.min(...vals), max: Math.max(...vals) }
  }

  function sparkPoints(arr) {
    if (!Array.isArray(arr)) return []
    const tail = arr.slice(-60)
    const vals = tail.map((v) => num(v)).map((v) => (v === null ? 0 : Math.max(v, 0)))
    const max = Math.max(...vals)
    return vals.map((v, i) => ({
      pct: max > 0 ? Math.min(100, (v / max) * 100) : 0,
      op: vals.length > 1 ? 0.2 + 0.8 * (i / (vals.length - 1)) : 1,
      val: tail[i]
    }))
  }

  const sparks = [
    {
      key: 'rps',
      label: 'Request rate',
      fmt: (v) => (num(v) === null ? '—' : String(num(v))),
      rangeFmt: (v) => String(v),
      bar: 'bg-sky-500'
    },
    {
      key: 'latency_ms',
      label: 'Latency',
      fmt: (v) => (num(v) === null ? '—' : `${num(v)} ms`),
      rangeFmt: (v) => `${v} ms`,
      bar: 'bg-violet-500'
    },
    {
      key: 'mem_mb',
      label: 'Memory',
      fmt: (v) => (num(v) === null ? '—' : `${num(v)} MB`),
      rangeFmt: (v) => `${v} MB`,
      bar: 'bg-emerald-500'
    },
    {
      key: 'cpu_frac',
      label: 'CPU',
      fmt: pctFromFrac,
      rangeFmt: (v) => `${(v * 100).toFixed(1)}%`,
      bar: 'bg-amber-500'
    }
  ]

  function logLine(l) {
    const f = l && typeof l === 'object' ? l.fields ?? {} : {}
    const msg = l?.message != null ? String(l.message) : ''
    const bits = []
    if (f.method) bits.push(f.method)
    if (f.path) bits.push(f.path)
    if (f.status_code != null) bits.push(`→ ${f.status_code}`)
    if (f.latency_ms != null) bits.push(`${f.latency_ms}ms`)
    return bits.length ? `${msg} · ${bits.join(' ')}` : msg
  }

  function tsTime(t) {
    const ms = num(t)
    if (ms === null) return '—'
    const d = new Date(ms)
    if (Number.isNaN(d.getTime())) return '—'
    return d.toLocaleTimeString('en-GB', { hour12: false })
  }

  function sortedLogs(raw) {
    const arr = Array.isArray(raw) ? raw.slice() : []
    arr.sort((a, b) => {
      const ta = num(a?.ts) ?? 0
      const tb = num(b?.ts) ?? 0
      return tb - ta
    })
    return arr
  }

  function codeClass(code) {
    if (code >= 200 && code < 300) return 'bg-emerald-500'
    if (code >= 300 && code < 400) return 'bg-sky-500'
    if (code >= 400 && code < 500) return 'bg-amber-500'
    if (code >= 500 && code < 600) return 'bg-red-500'
    return 'bg-zinc-500'
  }

  function cacheValue(key, v) {
    if (/bytes/i.test(String(key))) {
      const b = num(v)
      if (b !== null) return humanBytes(b)
    }
    if (v === null || v === undefined || v === '') return '—'
    if (typeof v === 'object') {
      try {
        return JSON.stringify(v)
      } catch {
        return String(v)
      }
    }
    return String(v)
  }

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
      errorRatePct === null ? 'slate' : errorRatePct > 5 ? 'rose' : errorRatePct > 0 ? 'amber' : 'emerald'
    return [
      {
        icon: Clock,
        label: 'Uptime',
        value: fmtUptime(d.uptime_secs),
        sub: 'since process start',
        tone: 'sky'
      },
      {
        icon: Activity,
        label: 'Total requests',
        value: num(d.requests_total)?.toLocaleString() ?? '—',
        sub: 'counted by the metrics middleware',
        tone: 'violet'
      },
      {
        icon: Gauge,
        label: 'Avg req / sec',
        value: num(d.req_per_sec)?.toFixed(2) ?? '—',
        sub: 'averaged over uptime',
        tone: 'emerald'
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
      cellClass: 'font-mono text-xs tabular-nums',
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
    { key: 'time', label: 'Time', class: 'w-28', cellClass: 'font-mono text-xs max-lg:text-sm', value: (l) => tsTime(l?.ts), sortValue: (l) => num(l?.ts) },
    { key: 'level', label: 'Level', class: 'w-24', value: (l) => String(l?.level ?? '').toLowerCase() },
    { key: 'event', label: 'Event', cellClass: 'max-w-md truncate font-mono text-xs', value: (l) => logLine(l) }
  ]

  const toneChip = {
    sky: 'bg-sky-500/10 text-sky-600 dark:text-sky-400',
    violet: 'bg-violet-500/10 text-violet-600 dark:text-violet-400',
    emerald: 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
    amber: 'bg-amber-500/10 text-amber-600 dark:text-amber-400',
    rose: 'bg-red-500/10 text-red-600 dark:text-red-400',
    slate: 'bg-muted text-muted-foreground'
  }

  function updatedTime(iso) {
    if (!iso) return ''
    const d = new Date(iso)
    return Number.isNaN(d.getTime()) ? '' : d.toLocaleTimeString('en-GB', { hour12: false })
  }
</script>

{#snippet latencyCell(o)}
  <div class="flex items-center gap-2">
    <span
      class={`w-16 shrink-0 text-right font-mono text-xs tabular-nums ${(num(o.latency) ?? 0) <= 0 ? 'text-muted-foreground' : ''}`}
    >
      {o.latency} ms
    </span>
    <div class="h-1.5 min-w-12 flex-1 overflow-hidden rounded-full bg-muted">
      <div class="h-full rounded-full bg-primary" style={`width:${originLatPct(o.latency)}%`}></div>
    </div>
  </div>
{/snippet}

{#snippet originStatusCell(o)}
  {#if o.status === 'up' || o.status === 'reachable'}
    <StatusBadge tone="success" dot>up</StatusBadge>
  {:else}
    <StatusBadge tone="danger" dot>{o.status}</StatusBadge>
  {/if}
{/snippet}

{#snippet logLevelCell(l)}
  <LogLevelBadge level={l?.level} />
{/snippet}

<section class="grid gap-6">
  <div class="grid gap-1">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
        <Activity class="size-5 text-muted-foreground" />
        System
      </h1>
      <div class="flex items-center gap-4">
        {#if updatedAt && !note}
          <span class="hidden text-xs text-muted-foreground sm:inline">
            Updated {updatedTime(updatedAt)}
          </span>
        {/if}
        <Button type="button" variant="outline" size="sm" onclick={() => loadAll()} disabled={busy}>
          <RefreshCw class={`mr-2 size-4${busy ? ' animate-spin' : ''}`} />
          {busy ? 'Refreshing…' : 'Refresh'}
        </Button>
        <label class="mb-0.5 flex min-h-11 cursor-pointer items-center gap-2 py-2 text-sm text-muted-foreground">
          <input type="checkbox" bind:checked={auto} class="size-5 accent-[var(--primary)]" />
          Auto 10s
        </label>
      </div>
    </div>
    <p class="max-lg:hidden text-sm text-muted-foreground">Runtime metrics and recent request logs — real numbers only.</p>
    {#if note}
      <p class="text-xs text-destructive max-lg:text-sm">{note}</p>
    {/if}
  </div>

  <div class="grid gap-2">
    <h2 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Overview</h2>
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
      <div class="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {#each heroTiles as t (t.label)}
          <Card.Root class="gap-2 py-4">
            <Card.Content class="flex items-start justify-between gap-2 px-4">
              <div class="grid gap-1">
                <Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{t.label}</Card.Title>
                <Card.Description class="font-mono text-2xl font-semibold tabular-nums">{t.value}</Card.Description>
              </div>
              <span class={`rounded-md p-2 ${toneChip[t.tone] ?? toneChip.slate}`} aria-hidden="true">
                <t.icon class="size-4" />
              </span>
            </Card.Content>
            <Card.Content class="px-4">
              <p class="text-[11px] leading-tight text-muted-foreground">{t.sub}</p>
            </Card.Content>
          </Card.Root>
        {/each}
      </div>

      {#if runtimeStats.length}
        <div
          class="grid grid-cols-2 gap-px overflow-hidden rounded-lg border sm:grid-cols-3 lg:grid-cols-6"
          style="background:var(--border);border-color:var(--border);"
        >
          {#each runtimeStats as [label, value] (label)}
            <div class="grid gap-0.5 bg-card px-3 py-2.5">
              <span class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">{label}</span>
              <span class="truncate font-mono text-sm font-semibold tabular-nums max-lg:whitespace-normal max-lg:break-words" title={value}>{value}</span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  <div class="grid gap-2">
    <h2 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Traffic</h2>
    {#if obs.phase === 'loading'}
      <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4" aria-busy="true">
        {#each Array.from({ length: 4 }, (_, i) => i) as i (i)}
          <Skeleton class="h-32" />
        {/each}
      </div>
    {:else if obs.phase === 'ok'}
      <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        {#each sparks as s (s.key)}
          {@const seriesArr = obs.data?.series?.[s.key]}
          {@const pts = sparkPoints(seriesArr)}
          {@const lastVal = latest(seriesArr)}
          {@const range = seriesRange(seriesArr)}
          <Card.Root class="gap-2 py-4">
            <Card.Title class="px-4 text-xs font-medium uppercase tracking-wide text-muted-foreground">{s.label}</Card.Title>
            <Card.Description class="px-4 font-mono text-lg font-semibold tabular-nums">{s.fmt(lastVal)}</Card.Description>
            <Card.Content class="px-4">
              {#if pts.length}
                <div class="flex h-14 items-end gap-[2px]" role="img" aria-label={`${s.label} sparkline, last ${pts.length} samples`}>
                  {#each pts as p, i (i)}
                    <div
                      class={`min-w-[2px] flex-1 rounded-sm ${s.bar}`}
                      style={`height:${p.pct}%;opacity:${p.op};${p.pct > 0 ? 'min-height:2px;' : ''}`}
                    ></div>
                  {/each}
                </div>
                <p class="mt-2 text-[10px] text-muted-foreground">
                  {#if range}
                    min {s.rangeFmt(range.min)} · max {s.rangeFmt(range.max)} · last {pts.length} samples
                  {:else}
                    last {pts.length} samples · 2s interval
                  {/if}
                </p>
              {:else}
                <p class="py-4 text-center text-xs text-muted-foreground">no series data yet — sampler warms up within seconds</p>
              {/if}
            </Card.Content>
          </Card.Root>
        {/each}
      </div>
    {/if}
  </div>

  <div class="grid gap-2">
    <h2 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Upstream</h2>
    {#if origins.length}
      <Card.Root>
        <Card.Header>
          <Card.Title class="text-base">Upstream origins</Card.Title>
          <Card.Description>Served request counts, latency and reachability per data source.</Card.Description>
        </Card.Header>
        <Card.Content>
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
      <h2 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Application</h2>
      <div class="grid gap-3 lg:grid-cols-2">
        <Card.Root>
          <Card.Header>
            <Card.Title class="text-base">Top paths</Card.Title>
            <Card.Description>Most requested routes since process start.</Card.Description>
          </Card.Header>
          <Card.Content>
            {#if paths.length}
              <ol class="grid gap-2">
                {#each paths as p, i (p[0])}
                  <li class="flex items-center gap-3">
                    <span class="w-5 text-right font-mono text-xs text-muted-foreground">{i + 1}.</span>
                    <span class="min-w-0 flex-1 truncate font-mono text-xs">{String(p[0])}</span>
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
          <Card.Header>
            <Card.Title class="text-base">Status distribution</Card.Title>
            <Card.Description>Response counts per HTTP status code.</Card.Description>
          </Card.Header>
          <Card.Content class="grid gap-3">
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
              <div class="flex flex-wrap gap-x-4 gap-y-1">
                {#each statusBars as s (`legend-${s.code}`)}
                  <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                    <span class={`inline-block size-2 rounded-sm ${s.cls}`} aria-hidden="true"></span>
                    <span class="font-mono">{s.code}</span> × {s.count.toLocaleString()}
                  </span>
                {/each}
              </div>
              <p class="text-xs text-muted-foreground">{codeAgg.total.toLocaleString()} responses recorded</p>
            {:else}
              <p class="text-sm text-muted-foreground">No responses recorded yet.</p>
            {/if}
          </Card.Content>
        </Card.Root>
      </div>

      <Card.Root>
        <Card.Header>
          <Card.Title class="text-base">Cache</Card.Title>
          <Card.Description>Live cache counters reported by the server.</Card.Description>
        </Card.Header>
        <Card.Content>
          {#if cacheEntries.length}
            <dl class="grid gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
              {#each cacheEntries as e (e.key)}
                <div class="flex items-baseline justify-between gap-3 border-b pb-1">
                  <dt class="text-xs uppercase tracking-wide text-muted-foreground">{e.label}</dt>
                  <dd class="truncate font-mono text-sm font-medium max-lg:whitespace-normal max-lg:break-words" title={e.value}>{e.value}</dd>
                </div>
              {/each}
            </dl>
          {:else}
            <p class="text-sm text-muted-foreground">no cache stats reported</p>
          {/if}
        </Card.Content>
      </Card.Root>
    </div>
  {/if}

  <div class="grid gap-2">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <h2 class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Logs</h2>
      <div class="flex items-center gap-1" role="group" aria-label="Filter logs by level">
        {#each [['all', 'All'], ['warn', 'Warn+'], ['errors', 'Errors']] as [value, label] ([value, label])}
          <Button
            type="button"
            size="xs"
            class="max-lg:h-11 max-lg:px-3.5"
            variant={logFilter === value ? 'default' : 'ghost'}
            onclick={() => (logFilter = value)}
            aria-pressed={logFilter === value}
          >
            {label}
          </Button>
        {/each}
      </div>
    </div>
    <Card.Root>
      <Card.Header>
        <Card.Title class="text-base">Recent logs</Card.Title>
        <Card.Description>Newest first, in-memory ring buffer.</Card.Description>
      </Card.Header>
      <Card.Content>
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
