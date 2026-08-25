<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import LandmarkIcon from 'lucide-svelte/icons/landmark'
  import PackageIcon from 'lucide-svelte/icons/package'
import FilterIcon from 'lucide-svelte/icons/filter'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import {
  TrainNumberBadge,
  StationCodeBadge,
  RunsOnBadges,
  DataSourceBadge
} from '$lib/components/badges/index.js'
import SignalDot from '$lib/components/SignalDot.svelte'

  let { view = '', selection = '' } = $props()

  const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

  let tab = $state('heritage')
  let selInput = $state('')

  let hPhase = $state('idle')
  let hError = $state(null)
  let hData = $state(null)

  let pPhase = $state('idle')
  let pError = $state(null)
  let pData = $state(null)

  let hKey = null
  let hBusy = false
  let pStarted = false

  const hTrains = $derived(listOf(hData))
  const hTotal = $derived(totalOf(hData))
  const hCaption = $derived(str(hData?.selection).trim())
  const hSource = $derived(str(hData?.data_source).trim())
  const pTrains = $derived(listOf(pData))
  const pTotal = $derived(totalOf(pData))
  const pSource = $derived(str(pData?.data_source).trim())

  function str(v) {
    return v == null ? '' : String(v)
  }

  function fmt(v) {
    const s = str(v).trim()
    return s && s !== '-' && s !== '--' ? s : '—'
  }

  function listOf(d) {
    if (Array.isArray(d?.trains)) return d.trains
    if (Array.isArray(d?.list)) return d.list
    return []
  }

  function totalOf(d) {
    if (!d) return 0
    const n = Number(d.total)
    return Number.isFinite(n) && n > 0 ? n : listOf(d).length
  }

  function stopOf(t, nestedKey, flatPrefix) {
    const nested = t?.[nestedKey]
    const o = nested && typeof nested === 'object' ? nested : {}
    const flat = (suffix) => str(t?.[`${flatPrefix}_${suffix}`]).trim()
    return {
      code: str(o.code ?? flat('code')).trim(),
      name: str(o.name ?? o.station ?? flat('station')).trim(),
      time: str(o.time ?? flat('time')).trim()
    }
  }

  function validityOf(t) {
    const from = fmt(t?.validity_from)
    const to = fmt(t?.validity_to)
    if (from === '—' && to === '—') return '—'
    if (from === '—') return to
    if (to === '—') return from
    return `${from} – ${to}`
  }

  function dayFlags(raw) {
    const s = str(raw).trim().toLowerCase()
    const flags = [false, false, false, false, false, false, false]
    if (!s || s === '-' || s === '--') return { flags, any: false }
    let any = false
    ;['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'].forEach((name, i) => {
      if (s.includes(name)) {
        flags[i] = true
        any = true
      }
    })
    if (!any) {
      const letters = s.replace(/[^a-z]/g, '')
      if (letters.length === 7) {
        for (let i = 0; i < 7; i++) {
          if (letters[i] !== '-') {
            flags[i] = true
            any = true
          }
        }
      }
    }
    return { flags, any }
  }

  async function loadHeritage(sel) {
    hKey = sel
    hBusy = true
    hPhase = hData ? 'refreshing' : 'loading'
    hError = null
    const qs = sel ? `?selection=${encodeURIComponent(sel)}` : ''
    const res = await api(`/rail-api/ntes/heritage${qs}`)
    hBusy = false
    if (hKey !== sel) return
    if (res.ok) {
      hData = res.data
      hPhase = 'ok'
    } else {
      hPhase = 'error'
      hError = res.error || `HTTP ${res.status}`
    }
  }

  async function loadParcel() {
    pStarted = true
    pPhase = pData ? 'refreshing' : 'loading'
    pError = null
    const res = await api('/rail-api/ntes/parcel')
    if (res.ok) {
      pData = res.data
      pPhase = 'ok'
    } else {
      pPhase = 'error'
      pError = res.error || `HTTP ${res.status}`
    }
  }

  function applyFilter() {
    const kw = str(selInput).trim()
    navigate(kw ? `/extras/heritage/${encodeURIComponent(kw)}` : '/extras/heritage')
  }

  function onTabChange(next) {
    // bits-ui updates the bound value before calling this callback, so compare
    // against the URL-derived prop rather than `tab`.
    if (!next || next === view) return
    navigate(`/extras/${next}`)
  }

  function routeText(t) {
    const src = stopOf(t, 'source', 'source')
    const dst = stopOf(t, 'destination', 'dest')
    return `${src.code}${src.time ? ` ${src.time}` : ''} → ${dst.code}${dst.time ? ` ${dst.time}` : ''}`
  }

  function daysText(raw) {
    const { flags, any } = dayFlags(raw)
    if (!any) return str(raw).trim()
    return DAY_LETTERS.filter((_, i) => flags[i]).join('')
  }

  const heritageCols = [
    { key: 'number', label: 'Number', class: 'w-24', value: (t) => fmt(t.number) },
    { key: 'name', label: 'Name', value: (t) => fmt(t.name), cellClass: 'font-medium' },
    { key: 'runs', label: 'Runs', cellClass: 'text-muted-foreground', value: (t) => fmt(t.runs) },
    { key: 'duration', label: 'Duration', class: 'w-28', cellClass: 'data-num text-xs', value: (t) => fmt(t.duration) },
    {
      key: 'route',
      label: 'Source → Destination',
      cellClass: 'data-num text-xs',
      value: (t) => routeText(t),
    },
  ]

  const parcelCols = [
    { key: 'number', label: 'Number', class: 'w-24', value: (t) => fmt(t.number) },
    { key: 'name', label: 'Name', cellClass: 'max-w-40 truncate font-medium', value: (t) => fmt(t.name) },
    { key: 'route', label: 'Route', cellClass: 'max-w-56 truncate', value: (t) => fmt(t.route) },
    {
      key: 'validity',
      label: 'Validity',
      class: 'w-48',
      cellClass: 'data-num text-xs whitespace-nowrap',
      value: (t) => validityOf(t),
    },
    { key: 'travel_time', label: 'Travel time', class: 'w-24', cellClass: 'data-num text-xs', value: (t) => fmt(t.travel_time) },
    {
      key: 'days',
      label: 'Days',
      value: (t) => daysText(t.days_of_run),
      sortValue: (t) => {
        const { any } = dayFlags(t.days_of_run)
        return any ? DAY_LETTERS.filter((_, i) => dayFlags(t.days_of_run).flags[i]).join('') : null
      },
    },
  ]

  $effect(() => {
    const nextTab = view === 'parcel' ? 'parcel' : 'heritage'
    const sel = str(selection).trim()
    untrack(() => {
      tab = nextTab
      if (nextTab === 'heritage') {
        if (selInput !== sel) selInput = sel
        if (!hBusy && hKey !== sel) loadHeritage(sel)
      } else if (!pStarted || pPhase === 'error') {
        loadParcel()
      }
    })
  })
</script>

{#snippet hNumberCell(t)}
  <TrainNumberBadge number={t.number} name={t.name} />
{/snippet}

{#snippet hRouteCell(t)}
  {@const src = stopOf(t, 'source', 'source')}
  {@const dst = stopOf(t, 'destination', 'dest')}
  <div class="flex flex-wrap items-center gap-x-2 text-xs">
    <span class="flex items-center gap-1.5">
      <StationCodeBadge code={src.code} name={src.name} size="xs" />
      {#if src.time}<span class="data-num text-muted-foreground">{src.time}</span>{/if}
    </span>
    <span class="text-muted-foreground">→</span>
    <span class="flex items-center gap-1.5">
      <StationCodeBadge code={dst.code} name={dst.name} size="xs" />
      {#if dst.time}<span class="data-num text-muted-foreground">{dst.time}</span>{/if}
    </span>
  </div>
{/snippet}

{#snippet pNumberCell(t)}
  <TrainNumberBadge number={t.number} name={t.name} />
{/snippet}

{#snippet pDaysCell(t)}
  {@const days = dayFlags(t.days_of_run)}
  {#if days.any}
    <RunsOnBadges flags={days.flags} />
  {:else}
    <span class="text-xs text-muted-foreground">{fmt(t.days_of_run)}</span>
  {/if}
{/snippet}

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="signage text-2xl sm:text-3xl">Extras</h1>
    <p class="max-lg:hidden text-sm text-muted-foreground">Heritage trains and running parcel specials, live from NTES.</p>
  </div>
  <div aria-hidden="true" class="track-rule"></div>

  <Tabs.Root class="min-w-0" bind:value={tab} onValueChange={onTabChange}>
    <Tabs.List class="w-full justify-start overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <Tabs.Trigger value="heritage"><LandmarkIcon class="mr-2 size-4" />Heritage</Tabs.Trigger>
      <Tabs.Trigger value="parcel"><PackageIcon class="mr-2 size-4" />Parcel</Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="heritage" class="mt-4 grid gap-4">
      <Card.Root>
        <Card.Content class="flex flex-wrap items-end gap-3">
          <div class="grid min-w-0 sm:min-w-56 flex-1 gap-2">
            <Label for="heritage-kw">Keyword</Label>
            <Input
              id="heritage-kw"
              bind:value={selInput}
              placeholder="Selection keyword, blank = all"
              onkeydown={(e) => {
                if (e.key === 'Enter') applyFilter()
              }}
            />
          </div>
          <Button onclick={applyFilter} class="max-lg:min-h-11 shrink-0 max-lg:w-full sm:w-auto"><FilterIcon class="mr-2 size-4" />Filter</Button>
        </Card.Content>
      </Card.Root>

      {#if hPhase === 'loading' || hPhase === 'refreshing'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if hPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load heritage trains</Alert.Title>
          <Alert.Description>{hError}</Alert.Description>
        </Alert.Root>
      {:else if hData}
        <Card.Root>
          <Card.Header class="flex flex-col items-start justify-between gap-3 space-y-0 sm:flex-row sm:items-center min-w-0">
            <div class="grid gap-2 min-w-0">
              <span class="grid size-9 place-items-center rounded-md bg-primary/10 text-primary">
                <LandmarkIcon class="size-4" aria-hidden="true" />
              </span>
              <div class="grid gap-1 min-w-0">
                <span class="w-fit rounded bg-saffron/15 px-1.5 text-xs font-semibold uppercase tracking-wide text-saffron-ink">Heritage</span>
                <Card.Title class="break-words">Heritage trains</Card.Title>
                <Card.Description class="flex flex-wrap items-center gap-x-1 break-words [overflow-wrap:anywhere]">
                  <span><span class="data-num">{hTotal}</span> train{hTotal === 1 ? '' : 's'}</span>
                  {#if hCaption}<span class="truncate max-w-[60vw]">· {hCaption}</span>{/if}
                </Card.Description>
              </div>
            </div>
            <div class="flex flex-wrap items-center justify-end gap-2 min-w-0">
              {#if str(selection).trim()}<Badge variant="outline" class="max-w-[60vw] truncate">keyword: {selection}</Badge>{/if}
              <DataSourceBadge source={hSource} />
            </div>
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={heritageCols}
              rows={hTrains}
              primary="name"
              rowKey={(t, i) => `${i}|${str(t.number)}|${str(t.name)}`}
              cells={{ number: hNumberCell, route: hRouteCell }}
              empty="No heritage trains match this selection."
            />
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={LandmarkIcon}
          title="Heritage list not loaded"
          hint="Heritage trains load automatically — press Filter to narrow by keyword."
        />
      {/if}
    </Tabs.Content>

    <Tabs.Content value="parcel" class="mt-4 grid gap-4">
      {#if pPhase === 'loading' || pPhase === 'refreshing'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if pPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load parcel specials</Alert.Title>
          <Alert.Description>{pError}</Alert.Description>
        </Alert.Root>
      {:else if pData}
        <Card.Root>
          <Card.Header class="flex flex-col items-start justify-between gap-3 space-y-0 sm:flex-row sm:items-center">
            <div class="grid gap-2 min-w-0">
              <span class="grid size-9 place-items-center rounded-md bg-primary/10 text-primary">
                <PackageIcon class="size-4" aria-hidden="true" />
              </span>
              <div class="grid gap-1 min-w-0">
                <span class="w-fit rounded bg-saffron/15 px-1.5 text-xs font-semibold uppercase tracking-wide text-saffron-ink">Parcel</span>
                <Card.Title>Parcel special trains</Card.Title>
                <Card.Description class="flex flex-wrap items-center gap-x-1.5 break-words">
                  <SignalDot tone="go" pulse />
                  <span><span class="data-num">{pTotal}</span> train{pTotal === 1 ? '' : 's'} currently listed</span>
                </Card.Description>
              </div>
            </div>
            <DataSourceBadge source={pSource} />
          </Card.Header>
          <Card.Content>
            <DataTable
              columns={parcelCols}
              rows={pTrains}
              primary="name"
              rowKey={(t, i) => `${i}|${str(t.number)}|${str(t.name)}`}
              cells={{ days: pDaysCell, number: pNumberCell }}
              empty="No parcel special trains are currently listed."
            />
          </Card.Content>
        </Card.Root>
      {:else}
        <EmptyState
          icon={PackageIcon}
          title="Parcel list not loaded"
          hint="Parcel specials load automatically when you open this tab — switch tabs and back to retry."
        />
      {/if}
    </Tabs.Content>
  </Tabs.Root>
</section>
