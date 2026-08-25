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
import PageShell from '$lib/components/PageShell.svelte'
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

  let activeTab = $state(tab === 'availability' ? 'availability' : 'trains')
  let prevTab = $state(tab === 'availability' ? 'availability' : 'trains')
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

  function selectRoute(s, d, dt) {
    const ns = norm(s)
    const nd = norm(d)
    const ndt = DATE_RE.test(String(dt)) ? String(dt) : todayISO()
    if (!ns || !nd || !DATE_RE.test(ndt) || ns === nd) return
    from = ns
    to = nd
    journeyDate = ndt
    filterQuery = ''
    search()
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
    if (nt !== prevTab) {
      activeTab = nt
      prevTab = nt
    }
  })
</script>

<PageShell
  title="Plan a trip"
  description="Trains and seat availability between two stations."
  breadcrumbs={[{ label: 'Home', href: '/' }, { label: 'Plan a trip' }]}
  routeFrom={cSrc}
  routeTo={cDst}
  onEdit={() => {}}
  showRouteBar={hasRoute}
/>

<section class="grid gap-4 md:gap-6">
  <!-- Search form card -->
  <Card.Root>
    <Card.Content class="grid gap-2 p-2">
      <StationPairInput bind:from bind:to onSwap={() => { if (canSearch) search() }} />
      {#if !viewport.narrow || !hasRoute}
        <DateStrip bind:value={journeyDate} />
      {/if}
      <Button onclick={search} disabled={!canSearch} class="w-full">Search</Button>
    </Card.Content>
  </Card.Root>

  <!-- Mobile: sticky date strip when route is active (above tabs) -->
  {#if viewport.narrow && hasRoute}
    <div class="sticky top-12 z-30 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:hidden">
      <DateStrip bind:value={journeyDate} onchange={() => { if (canSearch) search() }} />
    </div>
  {/if}

  <TrackRule />

  <!-- Tabs -->
  <Tabs.Root bind:value={activeTab}>
    <TabBar cols={2}>
      <Tabs.Trigger value="trains" class="max-lg:justify-center max-lg:text-xs max-lg:py-2.5">Trains</Tabs.Trigger>
      <Tabs.Trigger value="availability" class="max-lg:justify-center max-lg:text-xs max-lg:py-2.5">Availability</Tabs.Trigger>
    </TabBar>

    <Tabs.Content value="trains">
      <JourneysTable
        src={cSrc}
        dst={cDst}
        date={cDate}
        embedded
        onSelectRoute={selectRoute}
      />
    </Tabs.Content>
    <Tabs.Content value="availability">
      <Availability
        src={cSrc}
        dst={cDst}
        date={cDate}
        embedded
        filterQuery={filterQuery}
        onSelectRoute={selectRoute}
      />
    </Tabs.Content>
  </Tabs.Root>

<BottomSpacer />
</section>


