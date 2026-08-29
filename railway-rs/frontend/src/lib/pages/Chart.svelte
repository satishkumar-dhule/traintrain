<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import PageShell from '$lib/components/PageShell.svelte'
  import ResultMeta from '$lib/components/ResultMeta.svelte'
  import StatPill from '$lib/components/StatPill.svelte'
  import EntityChip from '$lib/components/EntityChip.svelte'
  import EmptyState from '$lib/components/EmptyState.svelte'
  import AsyncState from '$lib/components/AsyncState.svelte'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'
  import { asText, todayISO, DATE_RE } from '$lib/format.js'
  import BedDoubleIcon from 'lucide-svelte/icons/bed-double'
  import LayoutGridIcon from 'lucide-svelte/icons/layout-grid'
  import SearchIcon from 'lucide-svelte/icons/search'
  import InfoIcon from 'lucide-svelte/icons/info'

  let { train = '', date = '', station = '' } = $props()

  let queryTrain = $state(asText(train) || '')
  let journeyDate = $state(DATE_RE.test(asText(date)) ? asText(date) : todayISO())
  let boardingStation = $state(asText(station).toUpperCase())
  let committed = $state('')

  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let expanded = $state({})

  function toggleCoach(code) {
    expanded = { ...expanded, [code]: !expanded[code] }
  }

  function berthTone(status) {
    const s = String(status ?? '').toLowerCase()
    if (s === 'vacant') return 'go'
    if (s === 'occupied') return 'stop'
    if (s === 'not_reserved' || s === 'notreserved') return 'warning'
    return 'idle'
  }

  function berthClass(status) {
    const s = String(status ?? '').toLowerCase()
    if (s === 'vacant') return 'bg-signal-go text-white border-signal-go'
    if (s === 'occupied') return 'bg-muted text-muted-foreground'
    if (s === 'not_reserved' || s === 'notreserved') return 'bg-amber-100 text-amber-800 border-amber-200'
    return 'bg-card border'
  }

  const coaches = $derived(Array.isArray(data?.coaches) ? data.coaches : [])
  const canSearch = $derived(Boolean(asText(queryTrain)) && DATE_RE.test(asText(journeyDate)))
  const notice = $derived(asText(data?.notice))
  const isLocal = $derived(String(data?.data_source ?? '').toLowerCase() === 'local')

  async function loadChart(t, d, st) {
    const key = `${t}|${d}|${st}`
    committed = key
    phase = 'loading'
    errorMsg = null
    data = null
    expanded = {}
    const qs = `/rail-api/irctc/chart?train=${encodeURIComponent(t)}&date=${encodeURIComponent(d)}${st ? `&station=${encodeURIComponent(st)}` : ''}`
    const res = await api(qs)
    if (committed !== key) return
    if (res.ok) {
      data = res.data
      phase = 'ok'
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  function search() {
    const t = asText(queryTrain).trim()
    const d = asText(journeyDate).trim()
    const st = asText(boardingStation).trim().toUpperCase()
    if (!t || !DATE_RE.test(d)) return
    const url = `/chart/${encodeURIComponent(t)}/${encodeURIComponent(d)}${st ? `/${encodeURIComponent(st)}` : ''}`
    if (route.path !== url) navigate(url)
    loadChart(t, d, st)
  }

  $effect(() => {
    const t = asText(train)
    const d = DATE_RE.test(asText(date)) ? asText(date) : todayISO()
    const st = asText(station).toUpperCase()
    if (t) queryTrain = t
    if (DATE_RE.test(d)) journeyDate = d
    if (st) boardingStation = st
    if (t && DATE_RE.test(d)) {
      const key = `${t}|${d}|${st}`
      if (committed !== key && phase !== 'loading') {
        loadChart(t, d, st)
      }
    }
  })

  function coachStats(c) {
    const berths = Array.isArray(c?.berths) ? c.berths : []
    const total = berths.length
    const vacant = berths.filter((b) => String(b?.status ?? '').toLowerCase() === 'vacant').length
    const occupied = berths.filter((b) => String(b?.status ?? '').toLowerCase() === 'occupied').length
    return { total, vacant, occupied, other: total - vacant - occupied }
  }

  const friendlyErr = $derived.by(() => {
    const msg = asText(errorMsg)
    if (!msg) return ''
    if (/geofenced|Akamai|403 Forbidden/i.test(msg)) {
      return 'IRCTC is IP-geofenced to India — chart unavailable from this network. Try again from an Indian residential IP or near departure (~4h before, previous evening for early trains). IRCTC online-charts is blocked outside India; after 3 failures the app shows a static empty with notice.'
    }
    if (msg.length > 400) return msg.slice(0, 400) + '…'
    return msg
  })
</script>

<section class="grid gap-4 md:gap-6">
  <PageShell
    title="Chart vacancy"
    description="IRCTC prepared-chart berth map — vacant vs occupied per coach after charting. Free, no login."
    breadcrumbs={[{ label: 'Home', href: '/' }, { label: 'Chart', href: '/chart' }]}
    routeFrom={queryTrain}
    routeTo={data?.train_name ?? ''}
  />

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3 max-lg:p-3">
      <div class="grid min-w-0 sm:min-w-40 flex-1"
        onkeydown={(e) => { if (e.key === 'Enter' && !e.defaultPrevented) search() }}
      >
        <AutoCompleteInput
          id="chart-train"
          bind:value={queryTrain}
          kind="train"
          placeholder="Train number (e.g. 12951)"
          aria-label="Train number"
          onpick={(item) => { if (asText(item?.number)) { queryTrain = asText(item.number); search() } }}
        />
      </div>
      <div class="grid min-w-32 sm:min-w-36">
        <Input id="chart-date" type="date" bind:value={journeyDate} aria-label="Journey date" class="h-9" />
      </div>
      <div class="grid min-w-0 sm:min-w-32 flex-1"
        onkeydown={(e) => { if (e.key === 'Enter' && !e.defaultPrevented) search() }}
      >
        <AutoCompleteInput
          id="chart-station"
          bind:value={boardingStation}
          kind="station"
          placeholder="Boarding station (e.g. NDLS)"
          aria-label="Boarding station"
          onpick={(item) => { if (asText(item?.code)) boardingStation = asText(item.code).toUpperCase() }}
        />
      </div>
      <Button type="button" onclick={search} disabled={!canSearch || phase === 'loading'} class="shrink-0 max-lg:min-h-11 max-lg:w-full sm:w-auto">
        <SearchIcon class="size-4" /> {phase === 'loading' ? 'Loading…' : 'Get chart'}
      </Button>
    </Card.Content>
    {#if !boardingStation}
      <div class="px-6 pb-3 text-xs text-muted-foreground max-lg:px-3">Boarding station is optional — some trains require it after charting; leave blank to try source chart. Chart appears ~4h before departure (previous evening for early trains).</div>
    {/if}
  </Card.Root>

  <AsyncState
    phase={phase}
    error={friendlyErr || errorMsg}
    empty={!data}
    skeletonCount={3}
    emptyIcon={BedDoubleIcon}
    emptyTitle="No chart loaded yet"
    emptyHint="Enter train number, date and boarding station above and press Get chart. Data comes live from IRCTC online-charts (IP-geofenced to India; honest 502 outside India or before chart preparation)."
  >
    {#snippet children()}
      {#if data}
        <Card.Root>
          <Card.Header class="gap-3 space-y-0">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <Card.Title class="flex flex-wrap items-center gap-2">
                <EntityChip type="train" code={data.train_number} name={data.train_name} />
                <span>{data.train_name ?? ''}</span>
              </Card.Title>
              <ResultMeta source={data.data_source} />
            </div>
            <Card.Description class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
              <span>Train <span class="data-num font-medium">{data.train_number ?? '—'}</span></span>
              {#if data.journey_date}<span>Date <span class="data-num font-medium">{data.journey_date}</span></span>{/if}
              {#if data.boarding_station}<span>Boarding <span class="data-num font-medium">{data.boarding_station}</span></span>{/if}
              <span><span class="data-num font-semibold">{coaches.length}</span> coaches</span>
            </Card.Description>
            {#if notice}
              <Alert.Root variant={isLocal ? 'default' : undefined} class="mt-1">
                <InfoIcon class="size-4" />
                <Alert.Title>{isLocal ? 'Static empty (IRCTC unavailable)' : 'Notice'}</Alert.Title>
                <Alert.Description class="[overflow-wrap:anywhere]">{notice}</Alert.Description>
              </Alert.Root>
            {/if}
          </Card.Header>
          <Card.Content class="grid gap-3">
            {#if isLocal && coaches.length === 0}
              <EmptyState icon={LayoutGridIcon} title="Chart not available" hint={notice || 'IRCTC is geofenced to India or chart not yet published. Try again ~4h before departure.'} />
            {:else if coaches.length === 0}
              <EmptyState icon={LayoutGridIcon} title="No coaches returned" hint="IRCTC returned no coach list for this train/date/station. The chart may not be prepared yet." />
            {:else}
              <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <span class="inline-flex items-center gap-1"><span class="size-3 rounded-sm bg-signal-go border"></span> Vacant</span>
                <span class="inline-flex items-center gap-1"><span class="size-3 rounded-sm bg-muted border"></span> Occupied</span>
                <span class="inline-flex items-center gap-1"><span class="size-3 rounded-sm bg-amber-100 border border-amber-200"></span> Other / not reserved</span>
              </div>
              <div class="grid gap-2.5">
                {#each coaches as coach (coach.code)}
                  {@const stats = coachStats(coach)}
                  {@const isOpen = !!expanded[coach.code]}
                  <div class="rounded-lg border bg-card overflow-hidden">
                    <button
                      type="button"
                      class="flex w-full items-center justify-between gap-2 px-3 py-2.5 text-left hover:bg-accent/50 transition-colors"
                      onclick={() => toggleCoach(coach.code)}
                      aria-expanded={isOpen}
                    >
                      <div class="flex flex-wrap items-center gap-2 min-w-0">
                        <span class="data-num font-semibold">{coach.code || '—'}</span>
                        <span class="rounded border bg-muted px-1.5 py-0.5 text-xs">{coach.class_code || '—'}</span>
                        <span class="text-xs text-muted-foreground"><span class="data-num font-medium text-signal-go">{stats.vacant}</span> vacant · <span class="data-num">{stats.occupied}</span> occupied · <span class="data-num">{stats.total}</span> berths</span>
                      </div>
                      <span class="text-xs text-muted-foreground shrink-0">{isOpen ? 'Hide' : 'Show'} berths</span>
                    </button>
                    {#if isOpen}
                      <div class="border-t px-3 py-2.5">
                        {#if stats.total === 0}
                          <p class="text-xs text-muted-foreground">No berths in this coach.</p>
                        {:else}
                          <div class="grid grid-cols-[repeat(auto-fill,minmax(3.25rem,1fr))] gap-1.5">
                            {#each coach.berths as b (b.number)}
                              <div class={`flex flex-col items-center justify-center rounded border px-1 py-1.5 text-center ${berthClass(b.status)}`}>
                                <span class="data-num text-xs font-semibold leading-none">{b.number}</span>
                                <span class="mt-0.5 text-xs uppercase leading-none tracking-wide opacity-80">{asText(b.status) || 'unknown'}</span>
                              </div>
                            {/each}
                          </div>
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </Card.Content>
        </Card.Root>
      {/if}
    {/snippet}
  </AsyncState>

  <BottomSpacer />
</section>
