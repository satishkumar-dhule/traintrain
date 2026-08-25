<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
import ArrowDownUpIcon from 'lucide-svelte/icons/arrow-down-up'
import RouteIcon from 'lucide-svelte/icons/route'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import { loadRecent, rememberRecent, clearStored } from '$lib/recent.js'
import {
  TrainNumberBadge,
  RunsOnBadges,
  TrainDelayBadge,
  AvailabilityStatusBadge,
  STATUS_TONES,
  availabilityStatusKind,
  DAY_LETTERS,
  daysSummary,
  dayFlags
} from '$lib/components/badges/index.js'
import { availabilityHref, trainHref } from '$lib/utils.js'
import { norm, todayISO, DATE_RE, asText, fmtDash, numOrNull } from '$lib/format.js'
import CalendarDaysIcon from 'lucide-svelte/icons/calendar-days'
import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'

  let { src = '', dst = '', date = '', embedded = false, onSelectRoute = null } = $props()

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
    if (embedded && typeof onSelectRoute === 'function') {
      const dt = DATE_RE.test(String(date ?? '')) ? String(date) : todayISO()
      onSelectRoute(s, d, dt)
      return
    }
    from = s
    to = d
    commit()
  }

  let fromCode = $derived(norm(from))
  let toCode = $derived(norm(to))
  let canSearch = $derived(fromCode.length > 0 && toCode.length > 0)
  let sameCode = $derived(canSearch && fromCode === toCode)
  let loading = $derived(phase === 'loading')
  let trains = $derived(Array.isArray(result?.trains) ? result.trains : [])

  // inline availability expansion (embedded trains tab) — avoids misleading filtered tab
  let expandedTrain = $state(null)
  let inlinePhase = $state('idle')
  let inlineError = $state(null)
  let inlineData = $state(null)
  let inlineKey = ''

  function classCode(row) {
    return asText(row?.class).toUpperCase()
  }
  function quotaLabel(row) {
    const q = asText(row?.quota)
    return q && !/^(gn|general)$/i.test(q) ? q.toUpperCase() : ''
  }

  const runsCache = new Map()
  function fmtRuns(runsOn) {
    if (!Array.isArray(runsOn)) return ''
    const k = runsOn.join(',')
    if (runsCache.has(k)) return runsCache.get(k)
    const flags = dayFlags(runsOn)
    const out = daysSummary(flags ?? runsOn)
    runsCache.set(k, out)
    return out
  }

  const cols = [
    {
      key: 'train',
      label: 'Train',
      class: 'uppercase tracking-wide text-muted-foreground',
      value: (t) => `${t.number ?? ''} ${t.name ?? ''}`,
    },
    {
      key: 'dep',
      label: 'Departs',
      class: 'uppercase tracking-wide text-muted-foreground',
      cellClass: 'data-num text-xs max-lg:text-sm',
      value: (t) => t.departure_time,
    },
    {
      key: 'arr',
      label: 'Arrives',
      class: 'uppercase tracking-wide text-muted-foreground',
      cellClass: 'data-num text-xs max-lg:text-sm',
      value: (t) => t.arrival_time,
    },
    {
      key: 'runs',
      label: 'Runs on',
      class: 'uppercase tracking-wide text-muted-foreground',
      value: (t) => fmtRuns(t.runs_on),
      sortValue: (t) => (Array.isArray(t.runs_on) ? t.runs_on.filter(Boolean).length : null),
    },
  ]

  function trainKey(t, i) {
    return `${i}-${t?.number ?? ''}-${t?.name ?? ''}`
  }

  function commit() {
    const s = norm(from)
    const d = norm(to)
    if (!s || !d) return
    if (embedded) {
      if (typeof onSelectRoute === 'function') {
        const dt = DATE_RE.test(String(date ?? '')) ? String(date) : todayISO()
        onSelectRoute(s, d, dt)
      }
      return
    }
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

  async function toggleAvailability(t) {
    const num = String(t?.number ?? '').trim()
    if (!num) return
    if (expandedTrain === num) {
      expandedTrain = null
      inlineData = null
      inlinePhase = 'idle'
      inlineError = null
      inlineKey = ''
      return
    }
    expandedTrain = num
    const s = norm(src)
    const d = norm(dst)
    const dt = DATE_RE.test(String(date ?? '')) ? String(date) : todayISO()
    if (!s || !d || !DATE_RE.test(dt)) {
      inlinePhase = 'error'
      inlineError = 'Select source, destination and date first'
      return
    }
    const k = `${s}|${d}|${dt}|${num}`
    if (inlineKey === k && inlineData) {
      inlinePhase = 'ok'
      return
    }
    inlineKey = k
    inlinePhase = 'loading'
    inlineError = null
    inlineData = null
    const res = await api(
      `/rail-api/availability?src=${encodeURIComponent(s)}&dst=${encodeURIComponent(d)}&date=${encodeURIComponent(dt)}&source=auto`
    )
    if (inlineKey !== k) return
    if (res.ok) {
      const list = Array.isArray(res.data?.trains) ? res.data.trains : []
      const found = list.find((tr) => String(tr?.number ?? '').trim() === num)
      if (found) {
        inlineData = found
        inlinePhase = 'ok'
      } else {
        inlineData = null
        inlinePhase = 'empty'
        inlineError = `No availability for ${num} on ${dt}. Try the Availability tab for all trains.`
      }
    } else if (res.status === 404) {
      inlinePhase = 'empty'
      inlineError = res.error || `No direct trains for ${s} → ${d} on ${dt}.`
    } else {
      inlinePhase = 'error'
      inlineError = res.error || `HTTP ${res.status}`
    }
  }

  // n2 super fan-out: single delegated handler for all per-train actions (no per-row closures)
  function handleRowActionsClick(e) {
    const btn = e.target.closest?.('[data-jt-action]')
    if (!btn) return
    const action = btn.getAttribute('data-jt-action')
    const num = btn.getAttribute('data-train-number') ?? ''
    if (!action || !num) {
      // inline close is the only action without train number
      if (action === 'close-inline') {
        expandedTrain = null
        inlineData = null
        inlinePhase = 'idle'
        inlineError = null
        inlineKey = ''
      }
      return
    }
    if (action === 'availability') {
      if (embedded) {
        const t = trains.find((tr) => String(tr?.number ?? '').trim() === num)
        toggleAvailability(t ?? { number: num })
      } else {
        const dt = DATE_RE.test(String(date ?? '')) ? String(date) : todayISO()
        navigate(availabilityHref(src, dst, dt))
      }
    } else if (action === 'schedule') {
      navigate(trainHref(num, 'schedule'))
    } else if (action === 'close-inline') {
      expandedTrain = null
      inlineData = null
      inlinePhase = 'idle'
      inlineError = null
      inlineKey = ''
    }
  }

  $effect(() => {
    const s = norm(src)
    const d = norm(dst)
    // reset inline expansion when route changes
    const dt = DATE_RE.test(String(date ?? '')) ? String(date) : ''
    // collapse inline if route/date changed
    if (expandedTrain) {
      const expectedPrefix = `${s}|${d}|${dt}|`
      if (!inlineKey.startsWith(expectedPrefix)) {
        expandedTrain = null
        inlineData = null
        inlinePhase = 'idle'
        inlineError = null
        inlineKey = ''
      }
    }
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
  {@const isExpanded = expandedTrain === String(t?.number ?? '').trim()}
  <Button
    type="button"
    variant={isExpanded ? 'default' : 'outline'}
    size="xs"
    data-jt-action="availability"
    data-train-number={t.number}
    title={embedded
      ? `Show availability for ${t?.number ?? ''} on ${DATE_RE.test(String(date ?? '')) ? date : todayISO()} (inline)`
      : `Seat availability for ${norm(src)} → ${norm(dst)}${DATE_RE.test(String(date ?? '')) ? ` on ${date}` : ' (today)'}`}
    aria-pressed={isExpanded}
  >
    <CalendarDaysIcon class="size-3" />
    {isExpanded ? 'Hide' : 'Availability'}
  </Button>
  <Button
    type="button"
    variant="ghost"
    size="xs"
    data-jt-action="schedule"
    data-train-number={t.number}
    title={`Timetable & stops of ${t.number}`}
  >
    <CalendarClockIcon class="size-3" />
    Schedule
  </Button>
{/snippet}

{#snippet inlineAvlChip(row)}
  {@const kind = availabilityStatusKind(row?.status)}
  {@const tone =
    kind === 'available'
      ? STATUS_TONES.success
      : kind === 'rac'
        ? STATUS_TONES.warning
        : kind === 'waitlist' || kind === 'closed'
          ? STATUS_TONES.danger
          : STATUS_TONES.neutral}
  {@const fare = numOrNull(row?.fare)}
  <div class={`overflow-hidden rounded-md border px-2 py-1 ${tone}`}>
    <div class="flex items-baseline justify-between gap-2">
      <span class="flex min-w-0 items-baseline gap-1">
        <span class="data-num text-[11px] max-lg:text-xs font-semibold">{fmtDash(classCode(row))}</span>
        {#if quotaLabel(row)}
          <span class="rounded border border-border bg-muted px-1 text-[9px] leading-tight font-medium tracking-wide uppercase text-muted-foreground" title={`${quotaLabel(row)} quota`}>
            {quotaLabel(row)}
          </span>
        {/if}
      </span>
      <span class="data-num text-[11px] max-lg:text-xs">{fare != null ? `₹${fare.toLocaleString('en-IN')}` : ''}</span>
    </div>
    <div class="flex min-w-0 items-center gap-1 text-[10px] max-lg:text-sm max-lg:font-medium">
      <span class="size-1.5 shrink-0 rounded-full bg-current opacity-80"></span>
      <span class="min-w-0 truncate font-medium" title={asText(row?.status)}>{asText(row?.status) || '—'}</span>
    </div>
  </div>
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
          <div class="grid min-w-32 sm:min-w-44 flex-1">
            <AutoCompleteInput id="journeys-from" kind="station" placeholder="From station" aria-label="From station" bind:value={from} onpick={commit} />
          </div>
          <Button type="button" variant="outline" size="icon" aria-label="Swap stations" onclick={swap} disabled={loading} class="shrink-0">
            <ArrowDownUpIcon />
          </Button>
          <div class="grid min-w-32 sm:min-w-44 flex-1">
            <AutoCompleteInput id="journeys-to" kind="station" placeholder="To station" aria-label="To station" bind:value={to} onpick={commit} />
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
      <Card.Content onclick={handleRowActionsClick}>
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
          {#if embedded && expandedTrain}
            <div class="mt-4 rounded-lg border bg-muted/20 p-3">
              <div class="flex items-center justify-between gap-2 border-b pb-2">
                <div class="flex min-w-0 items-center gap-2">
                  <TrainNumberBadge number={inlineData?.number ?? expandedTrain} name={inlineData?.name ?? ''} />
                  <span class="truncate text-sm font-medium">{inlineData?.name ?? 'Loading…'}</span>
                  <span class="data-num text-xs text-muted-foreground">{norm(src)} → {norm(dst)} · {DATE_RE.test(String(date ?? '')) ? date : todayISO()}</span>
                </div>
                <Button type="button" variant="ghost" size="xs" data-jt-action="close-inline">
                  Close
                </Button>
              </div>
              {#if inlinePhase === 'loading'}
                <div class="grid gap-2 py-3">
                  <Skeleton class="h-4 w-48" />
                  <div class="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-1.5">
                    {#each [0,1,2,3] as i (i)}<Skeleton class="h-16" />{/each}
                  </div>
                </div>
              {:else if inlinePhase === 'error' || inlinePhase === 'empty'}
                <Alert.Root variant={inlinePhase === 'empty' ? 'default' : 'destructive'} class="mt-3">
                  <Alert.Title>{inlinePhase === 'empty' ? 'No availability' : 'Could not load availability'}</Alert.Title>
                  <Alert.Description>{inlineError}</Alert.Description>
                </Alert.Root>
                <p class="mt-2 text-xs text-muted-foreground">Open the <span class="font-medium">Availability</span> tab to see all trains.</p>
              {:else if inlineData}
                {@const rows = Array.isArray(inlineData?.availability) ? inlineData.availability : []}
                {#if rows.length === 0}
                  <p class="py-6 text-center text-sm text-muted-foreground">No class-level availability returned for this train.</p>
                {:else}
                  <div class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] max-lg:grid-cols-[repeat(auto-fill,minmax(8.25rem,1fr))] gap-1.5 pt-3">
                    {#each rows as r (r.class + (r.quota ?? ''))}
                      {@render inlineAvlChip(r)}
                    {/each}
                  </div>
                  <p class="mt-2 text-xs text-muted-foreground">Showing only {expandedTrain} — switch to Availability tab for all {trains.length} trains.</p>
                {/if}
              {/if}
            </div>
          {/if}
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
