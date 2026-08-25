<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import { viewport } from '$lib/media.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import DataTable from '$lib/components/DataTable.svelte'
  import EmptyState from '$lib/components/EmptyState.svelte'
  import PageHeader from '$lib/components/PageHeader.svelte'
  import Breadcrumbs from '$lib/components/Breadcrumbs.svelte'
  import RouteContextBar from '$lib/components/RouteContextBar.svelte'
  import PageShell from '$lib/components/PageShell.svelte'
  import AsyncState from '$lib/components/AsyncState.svelte'
  import EntityChip from '$lib/components/EntityChip.svelte'
  import ResultMeta from '$lib/components/ResultMeta.svelte'
  import StatPill from '$lib/components/StatPill.svelte'
  import { ExceptionKindBadge } from '$lib/components/badges/index.js'
  import SignalDot from '$lib/components/SignalDot.svelte'
  import CalendarX2Icon from 'lucide-svelte/icons/calendar-x-2'
  import TrackRule from '$lib/components/TrackRule.svelte'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'
  import { fmtExcDate } from '$lib/format.js'

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
  <span class="data-num text-xs max-lg:text-sm">{fmtExcDate(e.date)}</span>
{/snippet}

{#snippet excKindCell(e)}
  <ExceptionKindBadge kind={e.kind} />
{/snippet}

<section class="grid gap-4 md:gap-6">
  <PageShell
    title="Service alerts"
    description="Cancelled, rescheduled or diverted dates for any train."
    breadcrumbs={[{ label: 'Home', href: '/' }, { label: 'Service Alerts' }]}
    routeFrom={trainFilter || 'All'}
    routeTo={'Service Alerts'}
    onEdit={() => { trainFilter = ''; data = null; phase = 'idle' }}
    showRouteBar={!!(hasData || trainFilter)}
  />

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3 max-lg:p-3">
      <div
        class="grid min-w-0 sm:min-w-48 flex-1"
        onkeydown={(e) => {
          if (e.key === 'Enter' && !e.defaultPrevented) search()
        }}
      >
        <AutoCompleteInput
          id="train-filter"
          bind:value={trainFilter}
          kind="train"
          placeholder="Train number or name…"
          aria-label="Train number or name"
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

  <AsyncState
    phase={phase}
    error={errorMsg}
    empty={!hasData}
    skeletonCount={5}
    emptyIcon={CalendarX2Icon}
    emptyTitle="No exceptions loaded"
    emptyHint="Enter a train number above to see its cancelled, rescheduled or diverted dates."
  >
    {#snippet children()}
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
            <span class="inline-flex items-center gap-1.5">
              <SignalDot tone={entries.length === 0 ? 'go' : 'stop'} pulse={entries.length > 0} />
              <span class="data-num">{entries.length}</span>
              exception{entries.length === 1 ? '' : 's'}{excRoute ? ` · ${excRoute}` : ''}
            </span>
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
    {/snippet}
  </AsyncState>

  <BottomSpacer />
</section>
