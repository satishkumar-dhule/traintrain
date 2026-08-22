<script>
  import { route, navigate } from '$lib/router.svelte.js'
  import { theme, setTheme, initTheme } from '$lib/theme.svelte.js'
  import { palette, togglePalette } from '$lib/palette.svelte.js'
  import PowerSearch from '$lib/components/PowerSearch.svelte'
  import House from 'lucide-svelte/icons/house'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import RouteIcon from 'lucide-svelte/icons/route'
  import Ticket from 'lucide-svelte/icons/ticket'
  import Activity from 'lucide-svelte/icons/activity'
  import CalendarDays from 'lucide-svelte/icons/calendar-days'
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert'
  import Package from 'lucide-svelte/icons/package'
  import Search from 'lucide-svelte/icons/search'
  import Sun from 'lucide-svelte/icons/sun'
  import Moon from 'lucide-svelte/icons/moon'
  import Monitor from 'lucide-svelte/icons/monitor'

  let { children } = $props()

  let mobileOpen = $state(false)

  const items = [
    { href: '/', label: 'Home', icon: House, exact: true },
    { href: '/train', label: 'Live Status', icon: TrainFront },
    { href: '/station', label: 'Station Board', icon: Building2 },
    { href: '/journeys', label: 'Journeys', icon: RouteIcon },
    { href: '/availability', label: 'Availability', icon: CalendarDays },
    { href: '/pnr', label: 'PNR Status', icon: Ticket },
    { href: '/exceptions', label: 'Exceptions', icon: TriangleAlert },
    { href: '/extras', label: 'Heritage & Parcel', icon: Package },
    { href: '/system', label: 'System', icon: Activity }
  ]

  const themeIcons = { system: Monitor, light: Sun, dark: Moon }

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
    }
  }

  $effect(() => {
    initTheme()
    window.addEventListener('keydown', onKeydown)
    return () => window.removeEventListener('keydown', onKeydown)
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
      <span class="font-semibold tracking-tight">RailCompanion</span>
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
      <button
        type="button"
        class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        onclick={() => {
          const order = ['system', 'light', 'dark']
          const next = order[(order.indexOf(theme.mode) + 1) % order.length]
          setTheme(next)
        }}
        aria-label={`Theme: ${theme.mode}`}
      >
        <svelte:component this={themeIcons[theme.mode] ?? Monitor} class="size-4" />
        <span class="capitalize">{theme.mode} theme</span>
      </button>
    </div>
  </aside>

  <header
    class="sticky top-0 z-30 flex h-14 items-center gap-1 overflow-x-auto border-b bg-background/95 px-4 backdrop-blur lg:hidden"
  >
    <a class="mr-2 flex items-center gap-2" href="/" onclick={(e) => go(e, '/')}>
      <span class="flex size-7 items-center justify-center rounded-md bg-primary text-primary-foreground">
        <TrainFront class="size-4" />
      </span>
      <span class="whitespace-nowrap font-semibold tracking-tight">RailCompanion</span>
    </a>
    <button
      type="button"
      class="ml-auto flex items-center gap-1 rounded-md p-2 text-muted-foreground hover:bg-accent"
      onclick={() => togglePalette()}
      aria-label="Search"
    >
      <Search class="size-4" />
    </button>
    <button
      type="button"
      class="rounded-md p-2 text-muted-foreground hover:bg-accent"
      onclick={() => {
        const order = ['system', 'light', 'dark']
        const next = order[(order.indexOf(theme.mode) + 1) % order.length]
        setTheme(next)
      }}
      aria-label={`Theme: ${theme.mode}`}
    >
      <svelte:component this={themeIcons[theme.mode] ?? Monitor} class="size-4" />
    </button>
    <button
      type="button"
      class="rounded-md px-2 py-1 text-sm text-muted-foreground hover:bg-accent"
      onclick={() => (mobileOpen = !mobileOpen)}
      aria-expanded={mobileOpen}
    >
      Menu
    </button>
  </header>

  {#if mobileOpen}
    <div class="fixed inset-0 z-50 lg:hidden" role="dialog" aria-modal="true">
      <div class="absolute inset-0 bg-black/50" onclick={() => (mobileOpen = false)}></div>
      <nav class="absolute inset-y-0 left-0 w-64 overflow-y-auto border-r bg-card p-3">
        {#each items as item (item.href)}
          <a
            href={item.href}
            onclick={(e) => go(e, item.href)}
            class={`mt-1 flex items-center gap-3 rounded-md px-3 py-2 text-sm ${
              isActive(item) ? 'bg-primary text-primary-foreground' : 'hover:bg-accent'
            }`}
          >
            <item.icon class="size-4" />
            {item.label}
          </a>
        {/each}
      </nav>
    </div>
  {/if}

  <div class="lg:pl-60">
    <main class="mx-auto w-full max-w-5xl px-4 py-6 md:px-8 md:py-10">
      {@render children()}
    </main>
  </div>
</div>

<PowerSearch />
