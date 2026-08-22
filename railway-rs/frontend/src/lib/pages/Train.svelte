<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'

  let { number = '' } = $props()

  let query = $state(number)
  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let auto = $state(false)

  async function load(n) {
    const target = String(n || query).trim()
    if (!target) return
    if (phase === 'idle' || target !== data?.train_number) phase = 'loading'
    else phase = 'refreshing'
    errorMsg = null
    const res = await api(`/rail-api/live-status?train=${encodeURIComponent(target)}`)
    if (res.ok) {
      data = res.data
      phase = 'ok'
      if (`${target}` !== `${number}`) navigate(`/train/${target}`)
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  $effect(() => {
    if (number) load(number)
  })

  $effect(() => {
    if (!auto) return
    const t = setInterval(() => {
      if (query.trim()) load(query)
    }, 30000)
    return () => clearInterval(t)
  })

  function delayBadge(d) {
    if (d == null) return { variant: 'outline', text: '—' }
    if (d > 0) return { variant: 'destructive', text: `${d}m late` }
    return { variant: 'secondary', text: 'on time' }
  }

  function fmtTime(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Live train status</h1>
    <p class="text-sm text-muted-foreground">Spot any train by number. Data refreshes honestly from the live API.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div class="grid min-w-48 flex-1 gap-2">
        <Label for="train-no">Train number</Label>
        <Input
          id="train-no"
          bind:value={query}
          placeholder="e.g. 12951"
          inputmode="numeric"
          onkeydown={(e) => e.key === 'Enter' && load()}
        />
      </div>
      <button
        type="button"
        class="inline-flex h-9 items-center justify-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
        onclick={() => load()}
        disabled={phase === 'loading' || phase === 'refreshing'}
      >
        {phase === 'refreshing' ? 'Refreshing…' : 'Track'}
      </button>
      <label class="mb-0.5 flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
        <input type="checkbox" bind:checked={auto} class="size-4 accent-[var(--primary)]" />
        Auto 30s
      </label>
    </Card.Content>
  </Card.Root>

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
          <Card.Description>{data.stations?.length ?? 0} stations on the run</Card.Description>
        </div>
        <Badge variant={auto ? 'default' : 'outline'}>{auto ? 'auto 30s' : 'manual'}</Badge>
      </Card.Header>
      <Card.Content>
        <Table.Root>
          <Table.Header>
            <Table.Row>
              <Table.Head>Station</Table.Head>
              <Table.Head>Sched</Table.Head>
              <Table.Head>Actual</Table.Head>
              <Table.Head class="w-28">Delay</Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each data.stations ?? [] as s (s.code + s.name)}
              {@const d = delayBadge(s.delay_minutes)}
              <Table.Row>
                <Table.Cell>
                  <span class="font-medium">{s.name}</span>
                  <span class="ml-2 font-mono text-xs text-muted-foreground">{s.code}</span>
                </Table.Cell>
                <Table.Cell class="font-mono text-xs">{fmtTime(s.scheduled_arrival)}</Table.Cell>
                <Table.Cell class="font-mono text-xs">{fmtTime(s.actual_arrival)}</Table.Cell>
                <Table.Cell><Badge variant={d.variant}>{d.text}</Badge></Table.Cell>
              </Table.Row>
            {:else}
              <Table.Row>
                <Table.Cell colspan={4} class="text-muted-foreground">No station data returned.</Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </Card.Content>
    </Card.Root>
  {/if}
</section>
