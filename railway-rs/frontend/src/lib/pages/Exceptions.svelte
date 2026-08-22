<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
import CalendarX2Icon from 'lucide-svelte/icons/calendar-x-2'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'

  let { number = '', kind = '' } = $props()

  const KINDS = ['cancelled', 'rescheduled', 'diverted']
  const KIND_LABELS = {
    cancelled: 'Cancelled dates',
    rescheduled: 'Rescheduled dates',
    diverted: 'Diverted dates'
  }
  const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC']

  let query = $state('')
  let selectedKind = $state('')
  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let seq = 0

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

  function kindVariant(v) {
    if (v === 'cancelled') return 'destructive'
    if (v === 'rescheduled') return 'secondary'
    return 'outline'
  }

  async function load(n, k) {
    seq += 1
    const my = seq
    phase = 'loading'
    errorMsg = null
    const res = await api(
      `/rail-api/ntes/exceptional?train=${encodeURIComponent(n)}&type=${encodeURIComponent(k)}`
    )
    if (my !== seq) return
    if (res.ok) {
      data = res.data
      phase = 'ok'
    } else {
      data = null
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  function search() {
    const n = norm(query)
    const k = KINDS.includes(selectedKind) ? selectedKind : ''
    if (!n || !k) return
    if (norm(number) === n && kind === k) load(n, k)
    else navigate(`/exceptions/${encodeURIComponent(n)}/${k}`)
  }

  $effect(() => {
    const n = norm(number)
    const k = KINDS.includes(kind) ? kind : ''
    query = n
    selectedKind = k
    if (!n || !k) {
      phase = 'idle'
      data = null
      errorMsg = null
      return
    }
    load(n, k)
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
  <span class="font-mono text-xs">{fmtDate(e.date)}</span>
{/snippet}

{#snippet kindCell(e)}
  <Badge variant={kindVariant(e.kind)}>{e.kind || 'unknown'}</Badge>
{/snippet}

<section class="grid gap-6" class:idle-center={phase === 'idle'}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Exceptions</h1>
    <p class="text-sm text-muted-foreground">Cancelled, rescheduled or diverted dates for a train.</p>
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
      <div class="grid gap-2">
        <Label>Exception type</Label>
        <Select.Root type="single" bind:value={selectedKind}>
          <Select.Trigger class="w-44" aria-label="Exception type">
            {KIND_LABELS[selectedKind] ?? 'Select a type'}
          </Select.Trigger>
          <Select.Content>
            {#each KINDS as k (k)}
              <Select.Item value={k} label={KIND_LABELS[k]} />
            {/each}
          </Select.Content>
        </Select.Root>
      </div>
      <Button onclick={search} disabled={!norm(query) || !KINDS.includes(selectedKind)}>
        Search
      </Button>
    </Card.Content>
  </Card.Root>

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
      <Card.Header class="flex-row items-center justify-between space-y-0">
        <div class="grid gap-1">
          <Card.Title>
            {train.number ? `${train.number}` : norm(number)}
            {train.name ? ` · ${train.name}` : ''}
          </Card.Title>
          <Card.Description>
            {entries.length} exception{entries.length === 1 ? '' : 's'}{route ? ` · ${route}` : ''}
          </Card.Description>
        </div>
        <Badge variant="secondary">{KIND_LABELS[data.type] ?? KIND_LABELS[kind] ?? kind}</Badge>
      </Card.Header>
      <Card.Content class="grid gap-4">
        {#if entries.length > 0}
          <DataTable
            columns={cols}
            rows={entries}
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
            No {KIND_LABELS[data.type]?.toLowerCase() ?? 'exception'} records found for this train.
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
      hint="Pick a train and an exception type, then search to see its exceptional running days."
    />
  {/if}
</section>
