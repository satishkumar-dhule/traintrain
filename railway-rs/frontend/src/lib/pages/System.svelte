<script>
  import { api } from '$lib/api.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'

  let obs = $state({ phase: 'loading', data: null })
  let logs = $state({ phase: 'loading', data: null })

  $effect(() => {
    api('/rail-api/observability').then((res) => {
      obs = res.ok ? { phase: 'ok', data: res.data } : { phase: 'error', data: null }
    })
    api('/rail-api/logs?limit=25').then((res) => {
      logs = res.ok ? { phase: 'ok', data: res.data } : { phase: 'error', data: null }
    })
  })

  function fmtUptime(s) {
    if (!s && s !== 0) return '—'
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    return h > 0 ? `${h}h ${m}m` : `${m}m`
  }

  function levelVariant(level) {
    const l = String(level ?? '').toUpperCase()
    if (l.includes('ERROR')) return 'destructive'
    if (l.includes('WARN')) return 'outline'
    return 'secondary'
  }

  function logLine(l) {
    const f = l.fields ?? {}
    const bits = []
    if (f.method) bits.push(f.method)
    if (f.path) bits.push(f.path)
    if (f.status_code) bits.push(`→ ${f.status_code}`)
    if (f.latency_ms) bits.push(`${f.latency_ms}ms`)
    return bits.length ? `${l.message} · ${bits.join(' ')}` : l.message
  }

  function ts(t) {
    return new Date(Number(t)).toLocaleTimeString()
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">System</h1>
    <p class="text-sm text-muted-foreground">Runtime metrics and recent request logs — real numbers only.</p>
  </div>

  {#if obs.phase === 'loading'}
    <div class="grid grid-cols-2 gap-3 sm:grid-cols-4" aria-busy="true">
      {#each [0, 1, 2, 3] as i (i)}
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
      {#each [['Requests', obs.data.requests_total], ['RPS', obs.data.req_per_sec], ['Latency', `${obs.data.latency_ms} ms`], ['Uptime', fmtUptime(obs.data.uptime_secs)], ['In-flight', obs.data.active_connections], ['CPU', `${obs.data.cpu_usage ?? 0}%`], ['Mem MB', obs.data.mem_usage], ['Bytes out', obs.data.bytes_out]] as [label, value] (label)}
        <Card.Root class="gap-1 py-4">
          <Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</Card.Title>
          <Card.Description class="font-mono text-xl font-semibold">{value ?? '—'}</Card.Description>
        </Card.Root>
      {/each}
    </div>

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
      {#if logs.phase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2] as i (i)}
            <Skeleton class="h-8 w-full" />
          {/each}
        </div>
      {:else if logs.phase === 'error'}
        <p class="text-sm text-muted-foreground">Logs endpoint unavailable.</p>
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
            {#each logs.data.logs ?? [] as l (l.ts + l.message)}
              <Table.Row>
                <Table.Cell class="font-mono text-xs">{ts(l.ts)}</Table.Cell>
                <Table.Cell><Badge variant={levelVariant(l.level)}>{String(l.level).toLowerCase()}</Badge></Table.Cell>
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
