<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import ArrowDownUpIcon from 'lucide-svelte/icons/arrow-down-up'

  let { src = '', dst = '' } = $props()

  const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

  let from = $state('')
  let to = $state('')
  let phase = $state('idle')
  let errorMsg = $state(null)
  let result = $state(null)

  let key = ''

  function norm(v) {
    return String(v ?? '').trim().toUpperCase()
  }

  let fromCode = $derived(norm(from))
  let toCode = $derived(norm(to))
  let canSearch = $derived(fromCode.length > 0 && toCode.length > 0)
  let sameCode = $derived(canSearch && fromCode === toCode)
  let loading = $derived(phase === 'loading')
  let trains = $derived(Array.isArray(result?.trains) ? result.trains : [])

  function commit() {
    const s = norm(from)
    const d = norm(to)
    if (!s || !d) return
    const target = `/journeys/${encodeURIComponent(s)}/${encodeURIComponent(d)}`
    if (route.path === target) return
    navigate(target)
  }

  function swap() {
    const a = from
    from = to
    to = a
    commit()
  }

  function onSubmit(e) {
    e.preventDefault()
    commit()
  }

  $effect(() => {
    const s = norm(src)
    const d = norm(dst)
    if (!s || !d) {
      key = ''
      phase = 'idle'
      errorMsg = null
      result = null
      return
    }
    untrack(() => {
      if (norm(from) !== s) from = s
      if (norm(to) !== d) to = d
    })
    const k = `${s}|${d}`
    if (k === key) return
    key = k
    phase = 'loading'
    errorMsg = null
    result = null
    api(`/rail-api/ntes/trains-between?src=${encodeURIComponent(s)}&dst=${encodeURIComponent(d)}`).then((res) => {
      if (key !== k) return
      if (res.ok) {
        result = res.data
        phase = 'ok'
      } else {
        phase = 'error'
        errorMsg = res.error || `HTTP ${res.status}`
      }
    })
  })
</script>

<div class="flex flex-col gap-4">
  <Card.Root>
    <Card.Header>
      <Card.Title>Trains between stations</Card.Title>
      <Card.Description>Enter station codes, e.g. NDLS to DLI</Card.Description>
    </Card.Header>
    <Card.Content>
      <form class="flex flex-wrap items-end gap-2" onsubmit={onSubmit}>
        <div class="grid min-w-44 flex-1 gap-1.5">
          <Label for="journeys-from">From</Label>
          <AutoCompleteInput id="journeys-from" kind="station" placeholder="NDLS" bind:value={from} onpick={commit} />
        </div>
        <Button type="button" variant="outline" size="icon" aria-label="Swap stations" onclick={swap} disabled={loading}>
          <ArrowDownUpIcon />
        </Button>
        <div class="grid min-w-44 flex-1 gap-1.5">
          <Label for="journeys-to">To</Label>
          <AutoCompleteInput id="journeys-to" kind="station" placeholder="DLI" bind:value={to} onpick={commit} />
        </div>
        <Button type="submit" disabled={loading || !canSearch}>Search</Button>
      </form>
      {#if sameCode}
        <p class="mt-2 text-xs text-muted-foreground">
          From and To are the same station — pick two different codes for a meaningful search.
        </p>
      {/if}
    </Card.Content>
  </Card.Root>

  {#if loading}
    <Card.Root>
      <Card.Content class="space-y-3 pt-6">
        {#each [0, 1, 2] as row (row)}
          <div class="flex items-center gap-4">
            <Skeleton class="h-5 w-16" />
            <Skeleton class="h-4 w-48" />
            <Skeleton class="h-4 w-14" />
            <Skeleton class="h-4 w-14" />
            <div class="ml-auto flex gap-1">
              {#each [0, 1, 2, 3, 4, 5, 6] as d (d)}
                <Skeleton class="h-5 w-5 rounded-full" />
              {/each}
            </div>
          </div>
        {/each}
      </Card.Content>
    </Card.Root>
  {:else if phase === 'error'}
    <Alert.Root variant="destructive">
      <Alert.Title>Could not load trains</Alert.Title>
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {:else if result}
    <Card.Root>
      <Card.Header>
        <Card.Title>{norm(src)} → {norm(dst)}</Card.Title>
        <Card.Description>{trains.length} trains found</Card.Description>
      </Card.Header>
      <Card.Content>
        {#if trains.length === 0}
          <p class="text-sm text-muted-foreground">
            No trains found between {norm(src)} and {norm(dst)}.
          </p>
        {:else}
          <Table.Root>
            <Table.Header>
              <Table.Row>
                <Table.Head>Train</Table.Head>
                <Table.Head>Departs</Table.Head>
                <Table.Head>Arrives</Table.Head>
                <Table.Head>Runs on</Table.Head>
              </Table.Row>
            </Table.Header>
            <Table.Body>
              {#each trains as t, i (t.number ?? i)}
                <Table.Row>
                  <Table.Cell>
                    <span class="flex items-center gap-2">
                      <Badge variant="secondary">{t.number}</Badge>
                      <span>{t.name}</span>
                    </span>
                  </Table.Cell>
                  <Table.Cell>{t.departure_time}</Table.Cell>
                  <Table.Cell>{t.arrival_time}</Table.Cell>
                  <Table.Cell>
                    <span class="flex flex-wrap items-center gap-1">
                      {#each t.runs_on ?? [] as active, di (di)}
                        <Badge
                          variant={active ? 'secondary' : 'outline'}
                          class={`flex h-5 w-5 items-center justify-center px-1 text-[10px]${active ? '' : ' opacity-40'}`}
                        >
                          {DAY_LETTERS[di] ?? ''}
                        </Badge>
                      {/each}
                    </span>
                  </Table.Cell>
                </Table.Row>
              {/each}
            </Table.Body>
          </Table.Root>
        {/if}
      </Card.Content>
    </Card.Root>
  {/if}
</div>
