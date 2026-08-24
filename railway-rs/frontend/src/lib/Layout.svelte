<script>
  import { route, navigate } from '$lib/router.svelte.js'
  import { initTheme } from '$lib/theme.svelte.js'
  import { palette, togglePalette } from '$lib/palette.svelte.js'
  import PowerSearch from '$lib/components/PowerSearch.svelte'
  import NearbyStationDialog from '$lib/components/NearbyStationDialog.svelte'
  import DisplaySettings from '$lib/components/DisplaySettings.svelte'
  import VisitTrail from '$lib/components/VisitTrail.svelte'
  import House from 'lucide-svelte/icons/house'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import RouteIcon from 'lucide-svelte/icons/route'
  import Ticket from 'lucide-svelte/icons/ticket'
  import Activity from 'lucide-svelte/icons/activity'
  import CalendarDays from 'lucide-svelte/icons/calendar-days'
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert'
  import Package from 'lucide-svelte/icons/package'
  import Sparkles from 'lucide-svelte/icons/sparkles'
  import Lightbulb from 'lucide-svelte/icons/lightbulb'
  import Info from 'lucide-svelte/icons/info'
  import Search from 'lucide-svelte/icons/search'
  import XIcon from 'lucide-svelte/icons/x'
  import EllipsisIcon from 'lucide-svelte/icons/ellipsis'

  let { children } = $props()

  let mobileOpen = $state(false)

  /* Primary destinations get thumb-reachable slots; everything else lives in
     the "More" sheet. Bottom-nav pattern: ≤5 visible items. */
  const primaryItems = [
    { href: '/', label: 'Home', icon: House, exact: true },
    { href: '/train', label: 'Live', icon: TrainFront },
    { href: '/station', label: 'Board', icon: Building2 },
    { href: '/journeys', label: 'Journeys', icon: RouteIcon }
  ]

  const items = [
    ...primaryItems,
    { href: '/availability', label: 'Availability', icon: CalendarDays },
    { href: '/pnr', label: 'PNR Status', icon: Ticket },
    { href: '/exceptions', label: 'Exceptions', icon: TriangleAlert },
    { href: '/extras', label: 'Heritage & Parcel', icon: Package },
    { href: '/assistant', label: 'Ask Train Bro', icon: Sparkles },
    { href: '/insights', label: 'Insights', icon: Lightbulb },
    { href: '/system', label: 'System', icon: Activity },
    { href: '/about', label: 'About', icon: Info }
  ]

  function isActive(item) {
    if (item.exact) return route.path === '/'
    return route.path === item.href || route.path.startsWith(item.href + '/')
  }

  function go(e, href) {
    e.preventDefault()
    mobileOpen = false
    navigate(href)
  }

  function onKeydown(e) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault()
      togglePalette()
    } else if (e.key === 'Escape') {
      mobileOpen = false
    }
  }

  $effect(() => {
    initTheme()
    window.addEventListener('keydown', onKeydown)
    return () => window.removeEventListener('keydown', onKeydown)
  })

  /* Lock body scroll while the mobile drawer is open. */
  $effect(() => {
    if (!mobileOpen) return
    const prev = document.body.style.overflow
    document.body.style.overflow = 'hidden'
    return () => {
      document.body.style.overflow = prev
    }
  })

  /* Scroll-aware chrome: slide the mobile bars out of the way while scrolling
     down so results own the screen; reveal on scroll up, near top, or focus. */
  let chromeHidden = $state(false)
  let lastY = 0
  let scrollRaf = false

  function onScroll() {
    if (scrollRaf) return
    scrollRaf = true
    requestAnimationFrame(() => {
      scrollRaf = false
      const y = window.scrollY
      const dy = y - lastY
      lastY = y
      if (y <= 48) chromeHidden = false
      else if (dy > 8) chromeHidden = true
      else if (dy < -8) chromeHidden = false
    })
  }

  function revealChrome() {
    chromeHidden = false
  }

  $effect(() => {
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  })

  /* Fresh navigation always starts with visible chrome. */
  $effect(() => {
    void route.path
    chromeHidden = false
  })
</script>

<div class="min-h-screen bg-background">
  <aside
    class="fixed inset-y-0 left-0 z-40 hidden w-60 flex-col border-r bg-sidebar text-sidebar-foreground lg:flex"
  >
    <a class="flex h-14 items-center gap-2 border-b px-5" href="/" onclick={(e) => go(e, '/')}>
      <span class="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
        <TrainFront class="size-4" />
      </span>
      <span class="font-semibold tracking-tight">Train Bro</span>
    </a>
    <nav class="flex-1 space-y-1 overflow-y-auto p-3">
      {#each items as item (item.href)}
        <a
          href={item.href}
          onclick={(e) => go(e, item.href)}
          class={`flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors ${
            isActive(item)
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
          }`}
        >
          <item.icon class="size-4" />
          {item.label}
        </a>
      {/each}
    </nav>
    <div class="space-y-2 border-t p-3">
      <button
        type="button"
        class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        onclick={() => togglePalette()}
      >
        <Search class="size-4" />
        <span>Search everything</span>
        <kbd class="ml-auto rounded border bg-muted px-1.5 font-mono text-[10px]">⌘K</kbd>
      </button>
      <DisplaySettings />
    </div>
  </aside>

  <header
    onfocusin={revealChrome}
    class={`sticky top-0 z-30 flex h-[calc(3.5rem+env(safe-area-inset-top))] items-center gap-0.5 border-b bg-background/95 px-2 pt-[env(safe-area-inset-top)] backdrop-blur transition-transform duration-200 supports-[backdrop-filter]:bg-background/80 motion-reduce:transition-none lg:hidden ${
      chromeHidden ? 'max-lg:-translate-y-full' : 'translate-y-0'
    }`}
  >
    <a
      class="mr-1 flex min-h-11 min-w-11 items-center gap-2 rounded-lg px-2 transition-colors hover:bg-accent"
      href="/"
      onclick={(e) => go(e, '/')}
    >
      <span class="flex size-7 items-center justify-center rounded-md bg-primary text-primary-foreground">
        <TrainFront class="size-4" />
      </span>
      <span class="whitespace-nowrap font-semibold tracking-tight">Train Bro</span>
    </a>
    <div class="ml-auto flex items-center">
      <button
        type="button"
        class="flex size-11 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        onclick={() => togglePalette()}
        aria-label="Open search"
      >
        <Search class="size-5" />
      </button>
      <DisplaySettings placement="down" compact />
    </div>
  </header>

  {#if mobileOpen}
    <div class="fixed inset-0 z-50 lg:hidden" role="dialog" aria-modal="true" aria-label="Menu">
      <div class="scrim-enter absolute inset-0 bg-black/50" onclick={() => (mobileOpen = false)}></div>
      <nav
        class="drawer-enter absolute inset-y-0 left-0 w-[min(19rem,85vw)] overflow-y-auto border-r bg-card pb-[calc(0.75rem+env(safe-area-inset-bottom))] pt-[max(0.75rem,env(safe-area-inset-top))] shadow-xl"
      >
        <div class="flex items-center justify-between px-3 pb-1">
          <span class="text-sm font-semibold tracking-tight">Menu</span>
          <button
            type="button"
            class="flex size-11 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            onclick={() => (mobileOpen = false)}
            aria-label="Close menu"
          >
            <XIcon class="size-5" />
          </button>
        </div>
        {#each items as item (item.href)}
          <a
            href={item.href}
            onclick={(e) => go(e, item.href)}
            aria-current={isActive(item) ? 'page' : undefined}
            class={`mt-1 flex min-h-12 items-center gap-3.5 rounded-lg px-3.5 text-base transition-colors ${
              isActive(item)
                ? 'bg-primary text-primary-foreground'
                : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
            }`}
          >
            <item.icon class="size-[22px] shrink-0" />
            {item.label}
          </a>
        {/each}
        <div class="mt-3 space-y-1 border-t pt-3">
          <button
            type="button"
            class="flex min-h-12 w-full items-center gap-3.5 rounded-lg px-3.5 text-base text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
            onclick={() => togglePalette()}
          >
            <Search class="size-5 shrink-0" />
            <span>Search everything</span>
          </button>
          <DisplaySettings placement="down" />
        </div>
      </nav>
    </div>
  {/if}

  <div class="lg:pl-60">
    <VisitTrail />
    <main
      class="mx-auto w-full max-w-5xl px-4 pt-4 md:px-8 md:pt-10 max-lg:pb-[calc(5.5rem+env(safe-area-inset-bottom))] lg:pb-[calc(2.5rem+env(safe-area-inset-bottom))]"
    >
      {@render children()}
    </main>
  </div>

  <!-- Bottom tab bar: primary destinations thumb-reachable; the rest in "More". -->
  <nav
    onfocusin={revealChrome}
    class={`fixed inset-x-0 bottom-0 z-40 border-t bg-background/95 backdrop-blur transition-transform duration-200 supports-[backdrop-filter]:bg-background/85 motion-reduce:transition-none lg:hidden ${
      chromeHidden ? 'max-lg:translate-y-full' : 'translate-y-0'
    }`}
    aria-label="Primary"
  >
    <div class="mx-auto grid max-w-xl grid-cols-5">
      {#each primaryItems as item (item.href)}
        <a
          href={item.href}
          onclick={(e) => go(e, item.href)}
          aria-current={isActive(item) ? 'page' : undefined}
          class={`flex min-h-14 flex-col items-center justify-center gap-1 px-1 pt-0.5 text-xs font-medium transition-colors ${
            isActive(item) ? 'text-primary' : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          <item.icon class="size-6" />
          {item.label}
        </a>
      {/each}
      <button
        type="button"
        onclick={() => (mobileOpen = true)}
        aria-expanded={mobileOpen}
        aria-haspopup="dialog"
        class="flex min-h-14 flex-col items-center justify-center gap-1 px-1 pt-0.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
      >
        <EllipsisIcon class="size-6" />
        More
      </button>
    </div>
    <div class="h-[env(safe-area-inset-bottom)]"></div>
  </nav>
</div>

<PowerSearch />
<NearbyStationDialog />
