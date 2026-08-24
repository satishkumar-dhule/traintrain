<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
import CalendarX2Icon from 'lucide-svelte/icons/calendar-x-2'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import { ExceptionKindBadge, StatusBadge, TrainNumberBadge } from '$lib/components/badges/index.js'
import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'

  let { number = '' } = $props()

  const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC']

  let query = $state('')
  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let seq = 0

  const RECENT_KEY = 'rc-exceptions-recent'
  let recent = $state(loadRecent(RECENT_KEY))

  function rememberException(n) {
    recent = rememberRecent(
      RECENT_KEY,
      { id: n, label: n, sub: 'All exceptions' },
      (r) => r && typeof r?.id === 'string',
    )
  }

  function pickRecent(r) {
    const n = String(r?.id ?? '')
    if (!n) return
    query = n
    search()
  }

  function norm(v) {
    try {
      return decodeURIComponent(String(v ?? '')).trim()
    } catch {
      return String(v ?? '').trim()
    }
  }

  function fmtDate(iso) {
    const raw = String(iso ?? '').trim()
    if (!raw) return '—'
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(raw)
    if (!m) return raw
    return `${m[3]}-${MONTHS[Number(m[2]) - 1] ?? m[2]}-${m[1]}`
  }

  async function load(n) {
    seq += 1
    const my = seq
    phase = 'loading'
    errorMsg = null
    const res = await api(
      `/rail-api/ntes/exceptional?train=${encodeURIComponent(n)}`
    )
    if (my !== seq) return
    if (res.ok) {
      data = res.data
      phase = 'ok'
      rememberException(n)
    } else {
      data = null
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  function search() {
    const n = norm(query)
    if (!n) return
    if (norm(number) === n) load(n)
    else navigate(`/exceptions/${encodeURIComponent(n)}`)
  }

  $effect(() => {
    const n = norm(number)
    query = n
    if (!n) {
      phase = 'idle'
      data = null
      errorMsg = null
      return
    }
    load(n)
  })

  const cols = [
    {
      key: 'date',
      label: 'Date',
      class: 'w-36',
      value: (e) => fmtDate(e.date),
      sortValue: (e) => String(e.date ?? '').trim() || null,
    },
    { key: 'kind', label: 'Kind', class: 'w-40', value: (e) => e.kind || 'unknown' },
    {
      key: 'note',
      label: 'Note',
      cellClass: 'max-w-md whitespace-normal break-words text-sm text-muted-foreground',
      value: (e) => e.note || '',
    },
  ]
</script>

{#snippet dateCell(e)}
  <span class="font-mono text-xs max-lg:text-sm">{fmtDate(e.date)}</span>
{/snippet}

{#snippet kindCell(e)}
  <ExceptionKindBadge kind={e.kind} />
{/snippet}

<section class="grid gap-6" class:idle-center={phase === 'idle'}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Exceptions</h1>
    <p class="max-lg:hidden text-sm text-muted-foreground">Cancelled, rescheduled or diverted dates for a train.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div class="grid min-w-56 flex-1 gap-2">
        <Label for="exc-train">Train</Label>
        <AutoCompleteInput
          id="exc-train"
          kind="train"
          placeholder="Train number or name, e.g. 12951"
          bind:value={query}
        />
      </div>
      <Button onclick={search} disabled={!norm(query)}>
        Search
      </Button>
    </Card.Content>
  </Card.Root>

  {#if phase === 'idle' && recent.length > 0}
    <RecentSearches
      items={recent}
      onpick={pickRecent}
      onclear={() => {
        clearStored(RECENT_KEY)
        recent = []
      }}
    />
  {/if}

  {#if phase === 'loading'}
    <Card.Root>
      <Card.Content class="grid gap-2" aria-busy="true">
        {#each [0, 1, 2, 3, 4] as i (i)}
          <Skeleton class="h-10 w-full" />
        {/each}
      </Card.Content>
    </Card.Root>
  {:else if phase === 'error'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not load exceptions</Alert.Title>
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {:else if phase === 'ok' && data}
    {@const train = data.train ?? {}}
    {@const entries = Array.isArray(data.exceptions) ? data.exceptions : []}
    {@const msg = typeof data.message === 'string' ? data.message.trim() : ''}
    {@const route =
      [train.source, train.destination].filter((s) => s && String(s).trim()).join(' → ')}
    <Card.Root>
      <Card.Header class="flex flex-col items-start justify-between gap-3 space-y-0 sm:flex-row sm:items-center">
        <div class="grid gap-1">
          <Card.Title class="flex flex-wrap items-center gap-x-2">
            <TrainNumberBadge number={train.number || norm(number)} name={train.name} />
            {#if train.name}<span>{train.name}</span>{/if}
          </Card.Title>
          <Card.Description>
            {entries.length} exception{entries.length === 1 ? '' : 's'}{route ? ` · ${route}` : ''}
          </Card.Description>
        </div>
        <StatusBadge tone="neutral">All exceptions</StatusBadge>
      </Card.Header>
      <Card.Content class="grid gap-4">
        {#if entries.length > 0}
          <DataTable
            columns={cols}
            rows={entries}
            primary="date"
            rowKey={(e, i) => `${e.date ?? ''}|${e.kind ?? ''}|${i}`}
            cells={{ date: dateCell, kind: kindCell }}
            empty="No exception records found."
          />
        {:else if msg}
          <Alert.Root role="status">
            <Alert.Title>Nothing to report</Alert.Title>
            <Alert.Description>{msg}</Alert.Description>
          </Alert.Root>
        {:else}
          <div class="flex flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            <CalendarX2Icon class="size-5" />
            No exception records found for this train.
          </div>
        {/if}
        {#if Number(data.cache_ttl) > 0}
          <p class="text-xs text-muted-foreground">Source NTES · cached for {data.cache_ttl}s</p>
        {/if}
      </Card.Content>
    </Card.Root>
  {:else}
    <EmptyState
      icon={CalendarX2Icon}
      title="Nothing searched yet"
      hint="Pick a train number, then search to see all its exceptional running days."
    />
  {/if}
</section>
