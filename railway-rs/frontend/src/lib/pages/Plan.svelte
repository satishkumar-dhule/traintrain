<script>
  import { navigate } from '$lib/router.svelte.js'
  import { DATE_RE, todayISO, norm } from '$lib/format.js'
  import { viewport } from '$lib/media.svelte.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import PageHeader from '$lib/components/PageHeader.svelte'
  import StationPairInput from '$lib/components/StationPairInput.svelte'
  import DateStrip from '$lib/components/DateStrip.svelte'
  import Breadcrumbs from '$lib/components/Breadcrumbs.svelte'
  import RouteContextBar from '$lib/components/RouteContextBar.svelte'
  import JourneysTable from '$lib/components/JourneysTable.svelte'
  import Availability from '$lib/pages/Availability.svelte'
  import TabBar from '$lib/components/TabBar.svelte'
  import TrackRule from '$lib/components/TrackRule.svelte'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'

  let { src = '', dst = '', date = '', tab = 'trains' } = $props()

  let from = $state(norm(src))
  let to = $state(norm(dst))
  let journeyDate = $state(DATE_RE.test(String(date)) ? String(date) : todayISO())

  let cSrc = $state(norm(src))
  let cDst = $state(norm(dst))
  let cDate = $state(DATE_RE.test(String(date)) ? String(date) : todayISO())

  let activeTab = $state('trains')
  let filterQuery = $state('')

  const hasRoute = $derived(Boolean(cSrc) && Boolean(cDst))
  const canSearch = $derived(
    Boolean(from) && Boolean(to) && DATE_RE.test(journeyDate) && from !== to,
  )

  function search() {
    const s = norm(from)
    const d = norm(to)
    const dt = journeyDate
    if (!s || !d || !DATE_RE.test(dt) || s === d) return
    cSrc = s
    cDst = d
    cDate = dt
    const want = '/plan/' + s + '/' + d + '/' + dt
    if (location.pathname !== want) navigate(want)
  }

  $effect(() => {
    const ns = norm(src)
    const nd = norm(dst)
    const ndOk = DATE_RE.test(String(date))
    const nt = tab === 'availability' ? 'availability' : 'trains'
    if (ns) {
      from = ns
      cSrc = ns
    }
    if (nd) {
      to = nd
      cDst = nd
    }
    if (ndOk) {
      journeyDate = String(date)
      cDate = String(date)
    }
    if (nt === 'availability') activeTab = 'availability'
  })
</script>

<!-- Mobile: route context bar replaces header when route is active -->
{#if viewport.narrow && hasRoute}
  <RouteContextBar from={cSrc} to={cDst} onEdit={() => {}} />
{/if}

<!-- PageHeader + Breadcrumbs (desktop always, mobile when no route) -->
{#if !viewport.narrow || !hasRoute}
  <div class="hidden lg:block">
    <PageHeader title="Plan a trip" description="Trains and seat availability between two stations." />
    <Breadcrumbs items={[{ label: 'Home', href: '/' }, { label: 'Plan a trip' }]} />
  </div>
  <div class="lg:hidden">
    <PageHeader title="Plan a trip" size="sm" />
  </div>
{/if}

{#if !viewport.narrow}
  <TrackRule />
{/if}

<!-- Search form card -->
<Card.Root>
  <Card.Content class="grid gap-2 p-2">
    <StationPairInput bind:from bind:to onSwap={() => { if (canSearch) search() }} />
    <DateStrip bind:value={journeyDate} />
    <Button onclick={search} disabled={!canSearch} class="w-full">Search</Button>
  </Card.Content>
</Card.Root>

<!-- Mobile: sticky date strip when route is active (above tabs) -->
{#if viewport.narrow && hasRoute}
  <div class="sticky top-12 z-30 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:hidden">
    <DateStrip bind:value={journeyDate} onchange={() => { if (canSearch) search() }} />
  </div>
{/if}

<!-- Tabs -->
<Tabs.Root bind:value={activeTab} class="mt-3">
  <TabBar cols={2}>
    <Tabs.Trigger value="trains" class="max-lg:justify-center max-lg:text-xs max-lg:py-2.5">Trains</Tabs.Trigger>
    <Tabs.Trigger value="availability" class="max-lg:justify-center max-lg:text-xs max-lg:py-2.5">Availability</Tabs.Trigger>
  </TabBar>

  <Tabs.Content value="trains">
    <JourneysTable src={cSrc} dst={cDst} embedded />
  </Tabs.Content>
  <Tabs.Content value="availability">
    <Availability src={cSrc} dst={cDst} date={cDate} embedded filterQuery={filterQuery} />
  </Tabs.Content>
</Tabs.Root>

<BottomSpacer />


