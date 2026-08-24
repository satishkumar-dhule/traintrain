<script>
  import { navigate } from '$lib/router.svelte.js'
  import { DATE_RE, todayISO } from '$lib/utils.js'
  import { viewport } from '$lib/media.svelte.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import PageHeader from '$lib/components/PageHeader.svelte'
  import StationPairInput from '$lib/components/StationPairInput.svelte'
  import DateStrip from '$lib/components/DateStrip.svelte'
  import Breadcrumbs from '$lib/components/Breadcrumbs.svelte'
  import RouteContextBar from '$lib/components/RouteContextBar.svelte'
  import MobileFilterBar from '$lib/components/MobileFilterBar.svelte'
  import JourneysTable from '$lib/pages/JourneysTable.svelte'
  import Availability from '$lib/pages/Availability.svelte'

  let { src = '', dst = '', date = '', tab = 'trains' } = $props()

  function norm(v) {
    return String(v ?? '').trim().toUpperCase()
  }

  let from = $state(norm(src))
  let to = $state(norm(dst))
  let journeyDate = $state(DATE_RE.test(String(date)) ? String(date) : todayISO())

  let cSrc = $state(norm(src))
  let cDst = $state(norm(dst))
  let cDate = $state(DATE_RE.test(String(date)) ? String(date) : todayISO())

  let activeTab = $state('trains')
  let mobileSearchOpen = $state(false)

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
    mobileSearchOpen = false
    const want = '/plan/' + s + '/' + d + '/' + dt
    if (location.pathname !== want) navigate(want)
  }

  function openMobileSearch() {
    mobileSearchOpen = true
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

<!-- Mobile: sticky route context bar when route is active -->
{#if viewport.narrow && hasRoute && !mobileSearchOpen}
  <RouteContextBar from={cSrc} to={cDst} onEdit={openMobileSearch} />
{/if}

<!-- Mobile: search form overlay when editing -->
{#if viewport.narrow && mobileSearchOpen}
  <div class="flex flex-col gap-4 p-4">
    <PageHeader title="Plan a trip" size="sm" />
    <Card.Root>
      <Card.Content class="grid gap-3 p-4">
        <StationPairInput bind:from bind:to onSwap={() => { if (canSearch) search() }} />
        <DateStrip bind:value={journeyDate} />
        <div class="flex gap-2">
          <Button variant="outline" onclick={() => { mobileSearchOpen = false }} class="flex-1">Cancel</Button>
          <Button onclick={search} disabled={!canSearch} class="flex-1">Search</Button>
        </div>
      </Card.Content>
    </Card.Root>
  </div>
{:else}
  <!-- Desktop: PageHeader + Breadcrumbs -->
  <div class="hidden lg:block">
    <PageHeader title="Plan a trip" description="Trains and seat availability between any two stations." />
    <Breadcrumbs items={[{ label: 'Home', href: '/' }, { label: 'Plan a trip' }]} class="mb-3" />
  </div>

  <!-- Desktop: search form card -->
  <div class="hidden lg:block">
    <Card.Root>
      <Card.Content class="grid gap-3 p-4">
        <StationPairInput bind:from bind:to onSwap={() => { if (canSearch) search() }} />
        <div class="flex flex-wrap items-end gap-2">
          <div class="min-w-64 flex-1">
            <DateStrip bind:value={journeyDate} />
          </div>
          <Button onclick={search} disabled={!canSearch}>Search</Button>
        </div>
      </Card.Content>
    </Card.Root>
  </div>

  <!-- Mobile: sticky date strip when route is active -->
  {#if viewport.narrow && hasRoute}
    <div class="sticky top-12 z-30 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:hidden">
      <DateStrip bind:value={journeyDate} onchange={() => { if (canSearch) search() }} />
    </div>
  {/if}

  <!-- Tabs -->
  <Tabs.Root bind:value={activeTab} class="mt-4">
    <Tabs.List>
      <Tabs.Trigger value="trains">Trains</Tabs.Trigger>
      <Tabs.Trigger value="availability">Availability</Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="trains">
      <JourneysTable src={cSrc} dst={cDst} embedded />
    </Tabs.Content>
    <Tabs.Content value="availability">
      <Availability src={cSrc} dst={cDst} date={cDate} embedded />
    </Tabs.Content>
  </Tabs.Root>

  <!-- Mobile: sticky bottom filter bar in availability tab -->
  {#if viewport.narrow && activeTab === 'availability' && hasRoute}
    <MobileFilterBar />
    <div class="h-14 lg:hidden"></div>
  {/if}
{/if}
