<script>
  import { api } from './api.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import { Label } from '$lib/components/ui/label/index.js'

  let query = $state('')
  let results = $state([])
  let phase = $state('idle')
  let errorMsg = $state(null)
  let activeController = null
  let debounceTimer = null

  let stations = $derived(results.filter((r) => r.type === 'station'))
  let trains = $derived(results.filter((r) => r.type === 'train'))

  function abortActive() {
    if (activeController) {
      activeController.abort()
      activeController = null
    }
  }

  async function lookup(q) {
    abortActive()
    const controller = new AbortController()
    activeController = controller
    const res = await api(`/rail-api/search/suggest?q=${encodeURIComponent(q)}`)
    if (controller.signal.aborted) return
    activeController = null
    if (res.ok) {
      results = Array.isArray(res.data) ? res.data : []
      phase = 'results'
      errorMsg = null
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  $effect(() => {
    const q = query.trim()
    clearTimeout(debounceTimer)
    if (!q) {
      abortActive()
      results = []
      phase = 'idle'
      errorMsg = null
      return
    }
    phase = 'searching'
    debounceTimer = setTimeout(() => lookup(q), 300)
    return () => clearTimeout(debounceTimer)
  })

  function pick(item) {
    query =
      item.type === 'station'
        ? item.name + (item.code ? ` (${item.code})` : '')
        : item.name + (item.number ? ` (${item.number})` : '')
    abortActive()
    clearTimeout(debounceTimer)
    phase = 'idle'
    results = []
  }
</script>

<Card.Root aria-label="Station and train search">
  <Card.Header>
    <Card.Title>Search</Card.Title>
    <Card.Description>Live suggestions from the backend, debounced and abort-safe.</Card.Description>
  </Card.Header>
  <Card.Content class="grid gap-4">
    <div class="grid gap-2">
      <Label for="suggest-input">Station or train</Label>
      <Input
        id="suggest-input"
        type="search"
        bind:value={query}
        placeholder="e.g. NDLS or 12951"
        autocomplete="off"
        aria-busy={phase === 'searching'}
      />
    </div>

    {#if phase === 'idle'}
      <p class="m-0 text-sm text-muted-foreground">Type at least a few characters to search stations and trains.</p>
    {:else if phase === 'searching'}
      <div class="grid gap-2" aria-busy="true">
        <Skeleton class="h-4 w-full" />
        <Skeleton class="h-4 w-full" />
        <Skeleton class="h-4 w-3/5" />
      </div>
    {:else if phase === 'error'}
      <Alert.Root variant="destructive" role="alert">
        <Alert.Title>Search failed</Alert.Title>
        <Alert.Description>{errorMsg}</Alert.Description>
      </Alert.Root>
    {:else if results.length === 0}
      <p class="m-0 text-sm text-muted-foreground">No matches found.</p>
    {:else}
      <div class="grid gap-4" aria-busy="false">
        {#if stations.length > 0}
          <div class="grid gap-1">
            <span class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Stations</span>
            {#each stations as s (s.code ?? s.name)}
              <button
                type="button"
                class="flex min-h-9 w-full items-center justify-between gap-3 rounded-md px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
                onclick={() => pick(s)}
              >
                <span class="truncate">{s.name}</span>
                {#if s.code}<Badge variant="outline" class="font-mono">{s.code}</Badge>{/if}
              </button>
            {/each}
          </div>
        {/if}
        {#if trains.length > 0}
          <div class="grid gap-1">
            <span class="text-xs font-medium uppercase tracking-wide text-muted-foreground">Trains</span>
            {#each trains as t (t.number ?? t.name)}
              <button
                type="button"
                class="flex min-h-9 w-full items-center justify-between gap-3 rounded-md px-3 py-1.5 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
                onclick={() => pick(t)}
              >
                <span class="truncate">{t.name}</span>
                {#if t.number}<Badge variant="outline" class="font-mono">{t.number}</Badge>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </Card.Content>
</Card.Root>
