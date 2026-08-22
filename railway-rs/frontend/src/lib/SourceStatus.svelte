<script>
  import { api } from './api.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import DataTable from '$lib/components/DataTable.svelte'

  let state = $state({ phase: 'loading', data: null, error: null })
  let inflight = $state(false)

  async function load() {
    if (inflight) return
    inflight = true
    state = { ...state, phase: state.data ? 'refreshing' : 'loading', error: null }
    const res = await api('/rail-api/source-status')
    if (res.ok) {
      state = { phase: 'success', data: res.data, error: null }
    } else {
      state = { phase: 'error', data: null, error: res.error || `HTTP ${res.status}` }
    }
    inflight = false
  }

  $effect(() => {
    load()
  })

  let sources = $derived(state.data?.sources ?? [])

  const cols = [
    { key: 'name', label: 'Source', value: (s) => s.name },
    {
      key: 'reachable',
      label: 'Reachable',
      class: 'w-32',
      value: (s) => (s.reachable ? 'yes' : 'no'),
      sortValue: (s) => (s.reachable ? 1 : 0),
    },
  ]
</script>

{#snippet sourceCell(s)}
  <span class="font-medium">{s.name}</span>
{/snippet}

{#snippet reachableCell(s)}
  {#if s.reachable}
    <Badge>yes</Badge>
  {:else}
    <Badge variant="destructive">no</Badge>
  {/if}
{/snippet}

<Card.Root aria-label="Data source status">
  <Card.Header class="flex-row items-center justify-between space-y-0">
    <div class="grid gap-1">
      <Card.Title>Data sources</Card.Title>
      <Card.Description>Upstream reachability reported by the Rust backend.</Card.Description>
    </div>
    <Button variant="outline" size="sm" onclick={load} disabled={inflight}>
      {inflight ? 'Refreshing…' : 'Refresh'}
    </Button>
  </Card.Header>
  <Card.Content class="grid gap-4">
    {#if state.phase === 'loading' || state.phase === 'refreshing'}
      <div class="grid gap-3" aria-busy="true">
        <div class="grid grid-cols-3 gap-3">
          {#each [0, 1, 2] as i (i)}
            <Skeleton class="h-16" />
          {/each}
        </div>
        <Skeleton class="h-4 w-full" />
        <Skeleton class="h-4 w-full" />
        <Skeleton class="h-4 w-3/5" />
      </div>
    {:else if state.phase === 'error'}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>Could not load source status</Alert.Title>
        <Alert.Description class="flex flex-wrap items-center gap-3">
          <span>{state.error}</span>
          <Button variant="destructive" size="xs" onclick={load} disabled={inflight}>Retry</Button>
        </Alert.Description>
      </Alert.Root>
    {:else}
      <div class="grid grid-cols-3 gap-3" aria-busy="false">
        <Card.Root class="gap-1 py-3">
          <Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Live enabled</Card.Title>
          <Card.Description
            class={`text-lg font-semibold ${state.data.live_enabled ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400'}`}
          >
            {state.data.live_enabled ? 'Yes' : 'No'}
          </Card.Description>
        </Card.Root>
        <Card.Root class="gap-1 py-3">
          <Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Mode</Card.Title>
          <Card.Description class="font-mono text-lg font-semibold">{state.data.mode ?? '—'}</Card.Description>
        </Card.Root>
        <Card.Root class="gap-1 py-3">
          <Card.Title class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Cache TTL</Card.Title>
          <Card.Description class="font-mono text-lg font-semibold">{state.data.cache_ttl_seconds ?? '—'}s</Card.Description>
        </Card.Root>
      </div>

      {#if state.data.notice}
        <Alert.Root>
          <Alert.Title>Notice</Alert.Title>
          <Alert.Description>{state.data.notice}</Alert.Description>
        </Alert.Root>
      {/if}

      <DataTable
        columns={cols}
        rows={sources}
        rowKey={(s) => s.name}
        cells={{ name: sourceCell, reachable: reachableCell }}
        empty="No sources reported."
      />
    {/if}
  </Card.Content>
</Card.Root>
