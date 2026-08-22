<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import SourceStatus from '$lib/SourceStatus.svelte'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import Activity from 'lucide-svelte/icons/activity'
  import RefreshCw from 'lucide-svelte/icons/refresh-cw'

  let obs = $state({ phase: 'loading', data: null })
  let logsState = $state({ phase: 'loading', data: null })
  let auto = $state(false)
  let busy = $state(false)
  let note = $state('')

  async function loadAll() {
    busy = true
    const [o, l] = await Promise.all([
      api('/rail-api/observability'),
      api('/rail-api/logs?limit=25')
    ])
    const failures = []
    if (o.ok) {
      obs = { phase: 'ok', data: o.data }
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
    const h = Math.floor(t / 3600)
    const m = Math.floor((t % 3600) / 60)
    if (h >= 24) {
      const d = Math.floor(h / 24)
      return `${d}d ${h % 24}h`
    }
    return h > 0 ? `${h}h ${m}m` : `${m}m`
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
    { key: 'rps', label: 'Requests / sec', fmt: (v) => (num(v) === null ? '—' : String(num(v))) },
    { key: 'latency_ms', label: 'Latency', fmt: (v) => (num(v) === null ? '—' : `${num(v)} ms`) },
    { key: 'mem_mb', label: 'Memory', fmt: (v) => (num(v) === null ? '—' : `${num(v)} MB`) },
    { key: 'cpu_frac', label: 'CPU', fmt: pctFromFrac }
  ]

  function levelVariant(level) {
    const l = String(level ?? '').toUpperCase()
    if (l.includes('ERROR') || l.includes('FATAL')) return 'destructive'
    if (l.includes('WARN')) return 'outline'
    return 'secondary'
  }

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

  const tiles = $derived.by(() => {
    if (obs.phase !== 'ok' || !obs.data || typeof obs.data !== 'object') return []
    const d = obs.data
    return [
      ['Requests', num(d.requests_total)?.toLocaleString() ?? '—'],
      ['RPS', num(d.req_per_sec)?.toFixed(2) ?? '—'],
      ['Latency ms', num(d.latency_ms) !== null ? `${num(d.latency_ms)} ms` : '—'],
      ['Uptime', fmtUptime(d.uptime_secs)],
      ['In-flight', num(d.active_connections)?.toLocaleString() ?? '—'],
      ['CPU %', pctFromFrac(d.cpu_usage)],
      ['Mem MB', memMb(d.mem_usage)],
      ['Bytes out', humanBytes(d.bytes_out)]
    ]
  })

  const paths = $derived(
    obs.phase === 'ok' && Array.isArray(obs.data?.top_paths)
      ? obs.data.top_paths.slice(0, 8).filter((p) => Array.isArray(p) && p.length >= 2)
      : []
  )

  const statusBars = $derived.by(() => {
    if (obs.phase !== 'ok') return []
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
    const total = [...acc.values()].reduce((s, v) => s + v, 0)
    if (total <= 0) return []
    return [...acc.entries()]
      .sort((a, b) => a[0] - b[0])
      .map(([code, count]) => ({
        code,
        count,
        cls: codeClass(code),
        pct: (count / total) * 100
      }))
  })

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

  const logsNewest = $derived(logsState.phase === 'ok' ? sortedLogs(logsState.data?.logs) : [])
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
        <Activity class="size-5 text-muted-foreground" />
        System
      </h1>
      <div class="flex items-center gap-4">
        <Button type="button" variant="outline" size="sm" onclick={() => loadAll()} disabled={busy}>
          <RefreshCw class={`mr-2 size-4${busy ? ' animate-spin' : ''}`} />
          {busy ? 'Refreshing…' : 'Refresh'}
        </Button>
        <label class="mb-0.5 flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
          <input type="checkbox" bind:checked={auto} class="size-4 accent-[var(--primary)]" />
          Auto 10s
        </label>
      </div>
    </div>
    <p class="text-sm text-muted-foreground">Runtime metrics and recent request logs — real numbers only.</p>
    {#if note}
      <p class="text-xs text-destructive">{note}</p>
    {/if}
  </div>

  <SourceStatus />

  {#if obs.phase === 'loading'}
    <div class="grid grid-cols-2 gap-3 sm:grid-cols-4" aria-busy="true">
      {#each Array.from({ length: 8 }, (_, i) => i) as i (i)}
        <Skeleton class="h-20" />
      {/each}
    </div>
  {:else if obs.phase === 'error'}
    <Alert.Root variant="destructive">
      <Alert.Title>Observability unavailable</Alert.Title>
      <Alert.Description>Could not load /rail-api/observability.</Alert.Description>
    </Alert.Root>
  {:else}
    <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
      {#each tiles as [label, value] (label)}
        <Card.Root class="gap-1 py-4">
          <Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</Card.Title>
          <Card.Description class="font-mono text-xl font-semibold">{value}</Card.Description>
        </Card.Root>
      {/each}
    </div>

    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      {#each sparks as s (s.key)}
        {@const seriesArr = obs.data?.series?.[s.key]}
        {@const pts = sparkPoints(seriesArr)}
        {@const lastVal = latest(seriesArr)}
        <Card.Root class="gap-2 py-4">
          <Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{s.label}</Card.Title>
          <Card.Description class="font-mono text-lg font-semibold">{s.fmt(lastVal)}</Card.Description>
          <Card.Content class="px-4">
            {#if pts.length}
              <div class="flex h-16 items-end gap-[2px]" role="img" aria-label={`${s.label} sparkline, last ${pts.length} samples`}>
                {#each pts as p, i (i)}
                  <div
                    class="min-w-[2px] flex-1 rounded-sm bg-primary"
                    style={`height:${p.pct}%;opacity:${p.op};${p.pct > 0 ? 'min-height:2px;' : ''}`}
                  ></div>
                {/each}
              </div>
              <p class="mt-2 text-[10px] text-muted-foreground">last {pts.length} samples · 2s interval</p>
            {:else}
              <p class="py-5 text-center text-xs text-muted-foreground">no series data yet — sampler warms up within seconds</p>
            {/if}
          </Card.Content>
        </Card.Root>
      {/each}
    </div>

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
                  <Badge variant="secondary">{Number(p[1]).toLocaleString()}</Badge>
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
        <Card.Content>
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
            <div class="mt-3 flex flex-wrap gap-x-4 gap-y-1">
              {#each statusBars as s (`legend-${s.code}`)}
                <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <span class={`inline-block size-2 rounded-sm ${s.cls}`} aria-hidden="true"></span>
                  <span class="font-mono">{s.code}</span> × {s.count.toLocaleString()}
                </span>
              {/each}
            </div>
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
                <dd class="truncate font-mono text-sm font-medium" title={e.value}>{e.value}</dd>
              </div>
            {/each}
          </dl>
        {:else}
          <p class="text-sm text-muted-foreground">no cache stats reported</p>
        {/if}
      </Card.Content>
    </Card.Root>

    {#if obs.data.origins?.length}
      <Card.Root>
        <Card.Header>
          <Card.Title class="text-base">Upstream origins</Card.Title>
          <Card.Description>Latency and reachability per data source.</Card.Description>
        </Card.Header>
        <Card.Content>
          <Table.Root>
            <Table.Header>
              <Table.Row>
                <Table.Head>Name</Table.Head>
                <Table.Head class="w-28">Latency</Table.Head>
                <Table.Head class="w-32">Status</Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each obs.data.origins as o (o.name)}
                <Table.Row>
                  <Table.Cell class="font-medium">{o.name}</Table.Cell>
                  <Table.Cell class="font-mono text-xs">{o.latency} ms</Table.Cell>
                  <Table.Cell>
                    {#if o.status === 'up' || o.status === 'reachable'}
                      <Badge>up</Badge>
                    {:else}
                      <Badge variant="destructive">{o.status}</Badge>
                    {/if}
                  </Table.Cell>
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        </Card.Content>
      </Card.Root>
    {/if}
  {/if}

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
        <Table.Root>
          <Table.Header>
            <Table.Row>
              <Table.Head class="w-24">Time</Table.Head>
              <Table.Head class="w-20">Level</Table.Head>
              <Table.Head>Event</Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each logsNewest as l, i (i)}
              <Table.Row>
                <Table.Cell class="font-mono text-xs">{tsTime(l?.ts)}</Table.Cell>
                <Table.Cell><Badge variant={levelVariant(l?.level)}>{String(l?.level ?? 'unknown').toLowerCase()}</Badge></Table.Cell>
                <Table.Cell class="max-w-md truncate font-mono text-xs">{logLine(l)}</Table.Cell>
              </Table.Row>
            {:else}
              <Table.Row>
                <Table.Cell colspan={3} class="text-muted-foreground">No log entries.</Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      {/if}
    </Card.Content>
  </Card.Root>
</section>
