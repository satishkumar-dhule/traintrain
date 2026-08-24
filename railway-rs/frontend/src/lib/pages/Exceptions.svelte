<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import { viewport } from '$lib/media.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import DataTable from '$lib/components/DataTable.svelte'
  import EmptyState from '$lib/components/EmptyState.svelte'
  import PageHeader from '$lib/components/PageHeader.svelte'
  import Breadcrumbs from '$lib/components/Breadcrumbs.svelte'
  import RouteContextBar from '$lib/components/RouteContextBar.svelte'
  import EntityChip from '$lib/components/EntityChip.svelte'
  import ResultMeta from '$lib/components/ResultMeta.svelte'
  import StatPill from '$lib/components/StatPill.svelte'
  import { ExceptionKindBadge } from '$lib/components/badges/index.js'
  import CalendarX2Icon from 'lucide-svelte/icons/calendar-x-2'

  const RUN_MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec']

  let trainFilter = $state('')
  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)

  async function loadExceptions(t) {
    const trimmed = String(t ?? '').trim()
    if (!trimmed) return
    phase = 'loading'
    errorMsg = null
    const res = await api(`/rail-api/ntes/exceptional?train=${encodeURIComponent(trimmed)}`)
    if (`${trainFilter}` !== `${trimmed}`) return
    if (res.ok) {
      data = res.data
      phase = 'ok'
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  function search() {
    const t = String(trainFilter ?? '').trim()
    if (!t) return
    const want = `/exceptions/${encodeURIComponent(t)}`
    if (route.path !== want) navigate(want)
    loadExceptions(t)
  }

  $effect(() => {
    const segs = route.path.split('/').filter(Boolean)
    const t = segs[1] ? decodeURIComponent(segs[1]) : ''
    if (t && t !== trainFilter) {
      trainFilter = t
      loadExceptions(t)
    }
  })

  function fmtExcDate(iso) {
    const raw = String(iso ?? '').trim()
    if (!raw) return '—'
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(raw)
    if (!m) return raw
    const mo = (RUN_MONTHS[Number(m[2]) - 1] ?? '').toUpperCase()
    return mo ? `${m[3]}-${mo}-${m[1]}` : raw
  }

  const excCols = [
    {
      key: 'date',
      label: 'Date',
      class: 'w-36',
      value: (e) => fmtExcDate(e.date),
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

  const hasData = $derived(phase === 'ok' && data)
</script>

{#snippet excDateCell(e)}
  <span class="font-mono text-xs max-lg:text-sm">{fmtExcDate(e.date)}</span>
{/snippet}

{#snippet excKindCell(e)}
  <ExceptionKindBadge kind={e.kind} />
{/snippet}

<section class="grid gap-4 md:gap-6">
  {#if hasData || trainFilter}
    <RouteContextBar
      from={trainFilter || 'All'}
      to={'Service Alerts'}
      onEdit={() => { trainFilter = ''; data = null; phase = 'idle' }}
    />
  {/if}

  {#if !viewport.narrow}
    <PageHeader title="Service alerts" description="Cancelled, rescheduled or diverted dates for any train.">
      {#snippet children()}
        <Breadcrumbs items={[{ label: 'Home', href: '/' }, { label: 'Service Alerts' }]} />
      {/snippet}
    </PageHeader>
  {/if}

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3 max-lg:p-3">
      <div
        class="grid min-w-0 sm:min-w-48 flex-1 gap-2"
        onkeydown={(e) => {
          if (e.key === 'Enter' && !e.defaultPrevented) search()
        }}
      >
        <Label for="train-filter" class="max-lg:hidden">Train</Label>
        <AutoCompleteInput
          id="train-filter"
          bind:value={trainFilter}
          kind="train"
          placeholder="Train number or name…"
          onpick={(item) => {
            trainFilter = String(item.number)
            search()
          }}
        />
      </div>
      <Button type="button" onclick={search} disabled={phase === 'loading'} class="shrink-0 max-lg:min-h-9 max-lg:w-full sm:w-auto">
        {phase === 'loading' ? 'Loading…' : 'Search'}
      </Button>
    </Card.Content>
  </Card.Root>

  {#if phase === 'loading'}
    <div class="grid gap-2" aria-busy="true">
      {#each [0, 1, 2, 3, 4] as i (i)}
        <Skeleton class="h-10 w-full" />
      {/each}
    </div>
  {:else if phase === 'error'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not load exceptions</Alert.Title>
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {:else if hasData}
    {@const train = data.train ?? {}}
    {@const entries = Array.isArray(data.exceptions) ? data.exceptions : []}
    {@const msg = typeof data.message === 'string' ? data.message.trim() : ''}
    {@const excRoute =
      [train.source, train.destination].filter((s) => s && String(s).trim()).join(' → ')}
    <Card.Root>
      <Card.Header class="min-w-0 max-lg:p-4">
        <Card.Title class="flex flex-wrap items-center gap-x-2 break-words max-lg:text-base">
          <EntityChip type="train" code={train.number || trainFilter} name={train.name} />
          {#if train.name}<span class="break-words">{train.name}</span>{/if}
        </Card.Title>
        <Card.Description class="break-words [overflow-wrap:anywhere] max-lg:text-xs">
          {entries.length} exception{entries.length === 1 ? '' : 's'}{excRoute ? ` · ${excRoute}` : ''}
        </Card.Description>
        <ResultMeta source={data?.data_source}>
          <StatPill label="Records" value={entries.length} />
        </ResultMeta>
      </Card.Header>
      <Card.Content class="grid gap-4 max-lg:gap-3 max-lg:p-4">
        {#if entries.length > 0}
          <DataTable
            columns={excCols}
            rows={entries}
            primary="date"
            rowKey={(e, i) => `${e.date ?? ''}|${e.kind ?? ''}|${i}`}
            cells={{ date: excDateCell, kind: excKindCell }}
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
      </Card.Content>
    </Card.Root>
  {:else}
    <EmptyState
      icon={CalendarX2Icon}
      title="No exceptions loaded"
      hint="Enter a train number above to see its cancelled, rescheduled or diverted dates."
    />
  {/if}

  <div class="h-20 lg:hidden"></div>
</section>
