<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
import ArrowDownUpIcon from 'lucide-svelte/icons/arrow-down-up'
import RouteIcon from 'lucide-svelte/icons/route'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'
import { TrainNumberBadge, RunsOnBadges, TrainDelayBadge } from '$lib/components/badges/index.js'
import { availabilityHref, trainHref } from '$lib/utils.js'
import CalendarDaysIcon from 'lucide-svelte/icons/calendar-days'
import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'

  let { src = '', dst = '', embedded = false } = $props()

  const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

  let from = $state('')
  let to = $state('')
  let phase = $state('idle')
  let errorMsg = $state(null)
  let result = $state(null)

  let key = ''

  const RECENT_KEY = 'rc-journeys-recent'
  let recent = $state(loadRecent(RECENT_KEY))

  function pickRecent(r) {
    const [s, d] = String(r?.id ?? '').split('|')
    if (!s || !d) return
    from = s
    to = d
    commit()
  }

  function norm(v) {
    return String(v ?? '').trim().toUpperCase()
  }

  let fromCode = $derived(norm(from))
  let toCode = $derived(norm(to))
  let canSearch = $derived(fromCode.length > 0 && toCode.length > 0)
  let sameCode = $derived(canSearch && fromCode === toCode)
  let loading = $derived(phase === 'loading')
  let trains = $derived(Array.isArray(result?.trains) ? result.trains : [])

  const cols = [
    { key: 'train', label: 'Train', value: (t) => `${t.number ?? ''} ${t.name ?? ''}` },
    { key: 'dep', label: 'Departs', cellClass: 'font-mono text-xs max-lg:text-sm', value: (t) => t.departure_time },
    { key: 'arr', label: 'Arrives', cellClass: 'font-mono text-xs max-lg:text-sm', value: (t) => t.arrival_time },
    {
      key: 'runs',
      label: 'Runs on',
      value: (t) => daysText(t.runs_on),
      sortValue: (t) => (Array.isArray(t.runs_on) ? t.runs_on.filter(Boolean).length : null),
    },
  ]

  function daysText(runsOn) {
    if (!Array.isArray(runsOn)) return ''
    return runsOn.map((on, i) => (on ? DAY_LETTERS[i] ?? '' : '-')).join('')
  }

  function trainKey(t, i) {
    return `${i}-${t?.number ?? ''}-${t?.name ?? ''}`
  }

  function commit() {
    if (embedded) return
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
        recent = rememberRecent(
          RECENT_KEY,
          { id: `${s}|${d}`, label: `${s} → ${d}` },
          (r) => r && typeof r?.id === 'string',
        )
      } else {
        phase = 'error'
        errorMsg = res.error || `HTTP ${res.status}`
      }
    })
  })
</script>

{#snippet trainCell(t)}
  <span class="flex items-center gap-2">
    <TrainNumberBadge number={t.number} name={t.name} />
    <span>{t.name}</span>
    <TrainDelayBadge number={t.number} name={t.name} compact />
  </span>
{/snippet}

{#snippet runsCell(t)}
  <RunsOnBadges days={t.runs_on ?? []} />
{/snippet}

{#snippet rowActions(t)}
  <Button
    type="button"
    variant="outline"
    size="xs"
    onclick={() => navigate(availabilityHref(src, dst))}
    title={`Seat availability for ${norm(src)} → ${norm(dst)} (today)`}
  >
    <CalendarDaysIcon class="size-3" />
    Availability
  </Button>
  <Button
    type="button"
    variant="ghost"
    size="xs"
    onclick={() => navigate(trainHref(t.number, 'schedule'))}
    title={`Timetable & stops of ${t.number}`}
  >
    <CalendarClockIcon class="size-3" />
    Schedule
  </Button>
{/snippet}

<div class="flex flex-col gap-4">
  {#if !embedded}
    <Card.Root>
      <Card.Header>
        <Card.Title>Trains between stations</Card.Title>
        <Card.Description>Enter station codes, e.g. NDLS to DLI</Card.Description>
      </Card.Header>
      <Card.Content>
        <form class="flex flex-wrap items-end gap-2" onsubmit={onSubmit}>
          <div class="grid min-w-32 sm:min-w-44 flex-1 gap-1.5">
            <Label for="journeys-from">From</Label>
            <AutoCompleteInput id="journeys-from" kind="station" placeholder="NDLS" bind:value={from} onpick={commit} />
          </div>
          <Button type="button" variant="outline" size="icon" aria-label="Swap stations" onclick={swap} disabled={loading} class="shrink-0">
            <ArrowDownUpIcon />
          </Button>
          <div class="grid min-w-32 sm:min-w-44 flex-1 gap-1.5">
            <Label for="journeys-to">To</Label>
            <AutoCompleteInput id="journeys-to" kind="station" placeholder="DLI" bind:value={to} onpick={commit} />
          </div>
          <Button type="submit" disabled={loading || !canSearch} class="shrink-0 max-lg:min-h-11 max-lg:w-full sm:w-auto">Search</Button>
        </form>
        {#if sameCode}
          <p class="mt-2 text-xs text-muted-foreground">
            From and To are the same station — pick two different codes for a meaningful search.
          </p>
        {/if}
      </Card.Content>
    </Card.Root>
  {/if}

  {#if loading}
    <Card.Root>
      <Card.Content class="space-y-3 pt-6">
        {#each [0, 1, 2] as row (row)}
          <div class="flex flex-wrap items-center gap-2 sm:gap-4 min-w-0 overflow-hidden">
            <Skeleton class="h-5 w-16 shrink-0" />
            <Skeleton class="h-4 w-48 max-w-[60vw] min-w-0 flex-1" />
            <Skeleton class="h-4 w-14 shrink-0" />
            <Skeleton class="h-4 w-14 shrink-0" />
            <div class="ml-auto flex flex-wrap gap-1 shrink-0">
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
          <EmptyState
            icon={RouteIcon}
            title="No trains found"
            hint={`Nothing runs between ${norm(src)} and ${norm(dst)}.`}
          />
        {:else}
          <DataTable
            columns={cols}
            rows={trains}
            rowKey={trainKey}
            primary="train"
            cells={{ train: trainCell, runs: runsCell }}
            actions={rowActions}
            empty={`No trains found between ${norm(src)} and ${norm(dst)}.`}
          />
        {/if}
      </Card.Content>
    </Card.Root>
  {:else if phase === 'idle'}
    {#if recent.length > 0}
      <RecentSearches
        items={recent}
        onpick={pickRecent}
        onclear={() => {
          clearStored(RECENT_KEY)
          recent = []
        }}
      />
    {/if}
    <EmptyState
      icon={RouteIcon}
      title="Pick two stations"
      hint="Choose From and To station codes to list every train between them."
    />
  {/if}
</div>
