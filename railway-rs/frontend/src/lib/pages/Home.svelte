<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import RouteIcon from 'lucide-svelte/icons/route'
  import Ticket from 'lucide-svelte/icons/ticket'

  let state = $state({ phase: 'loading', data: null })
  let obs = $state(null)

  $effect(() => {
    api('/rail-api/source-status').then((res) => {
      state = res.ok ? { phase: 'ok', data: res.data } : { phase: 'error', data: null }
    })
    api('/rail-api/observability').then((res) => {
      if (res.ok && res.data && typeof res.data === 'object') {
        obs = res.data
      }
    })
  })

  function fmtUptime(secs) {
    const n = Number(secs)
    if (!Number.isFinite(n) || n <= 0) return null
    const h = Math.floor(n / 3600)
    const m = Math.floor((n % 3600) / 60)
    return h > 0 ? `${h}h ${m}m` : `${m}m`
  }

  const uptimeLabel = $derived(fmtUptime(obs?.uptime_secs))
  const showObs = $derived(
    Boolean(obs && (obs?.requests_total != null || uptimeLabel || obs?.active_connections != null))
  )

  const popularTrains = [12951, 12309, 12002]

  const features = [
    { href: '/train', icon: TrainFront, title: 'Live train status', desc: 'Where is my train right now, delay per station.' },
    { href: '/station', icon: Building2, title: 'Station boards', desc: 'Arrivals and departures for the next hours.' },
    { href: '/journeys', icon: RouteIcon, title: 'Find journeys', desc: 'Trains between any two stations, with run days.' },
    { href: '/pnr', icon: Ticket, title: 'PNR status', desc: 'Passenger reservation status with captcha retry.' }
  ]
</script>

<section class="grid gap-8">
  <div class="grid gap-3">
    <h1 class="text-4xl font-semibold tracking-tight">RailCompanion</h1>
    <p class="max-w-xl text-muted-foreground">
      Live Indian Railways data served by a Rust backend — honest sources, no accounts,
      no fabricated numbers.
    </p>
    <div class="flex flex-wrap items-center gap-2 pt-1">
      {#if state.phase === 'loading'}
        <Skeleton class="h-6 w-40" />
        <Skeleton class="h-6 w-24" />
      {:else if state.phase === 'error'}
        <Badge variant="destructive">source status unavailable</Badge>
      {:else}
        <Badge variant={state.data.live_enabled ? 'default' : 'destructive'}>
          {state.data.live_enabled ? `live · ${state.data.mode}` : 'live disabled'}
        </Badge>
        <Badge variant="secondary">cache {state.data.cache_ttl_seconds}s</Badge>
        <Badge variant="outline">
          {state.data.sources.filter((s) => s.reachable).length}/{state.data.sources.length} upstreams up
        </Badge>
      {/if}
    </div>

    {#if showObs}
      <div class="flex flex-wrap gap-2">
        <div class="rounded-md border bg-muted/40 px-3 py-1.5">
          <p class="text-[10px] uppercase tracking-wide text-muted-foreground">requests_total</p>
          <p class="text-sm font-medium tabular-nums">{obs?.requests_total ?? '—'}</p>
        </div>
        <div class="rounded-md border bg-muted/40 px-3 py-1.5">
          <p class="text-[10px] uppercase tracking-wide text-muted-foreground">uptime</p>
          <p class="text-sm font-medium tabular-nums">{uptimeLabel ?? '—'}</p>
        </div>
        <div class="rounded-md border bg-muted/40 px-3 py-1.5">
          <p class="text-[10px] uppercase tracking-wide text-muted-foreground">active connections</p>
          <p class="text-sm font-medium tabular-nums">{obs?.active_connections ?? '—'}</p>
        </div>
      </div>
    {/if}

    <div class="flex flex-wrap items-center gap-2 pt-1">
      <span class="text-xs text-muted-foreground">Popular trains</span>
      {#each popularTrains as n (n)}
        <button
          type="button"
          class="inline-flex h-6 items-center rounded-full border px-2.5 font-mono text-xs transition-colors hover:bg-muted hover:text-foreground"
          onclick={() => navigate(`/train/${n}`)}
        >
          {n}
        </button>
      {/each}
    </div>
  </div>

  <div class="grid gap-4 sm:grid-cols-2">
    {#each features as f (f.href)}
      <Card.Root class="transition-colors hover:border-primary/50">
        <a
          href={f.href}
          class="flex h-full flex-col gap-3 p-6"
          onclick={(e) => {
            e.preventDefault()
            navigate(f.href)
          }}
        >
          <span class="flex size-10 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <f.icon class="size-5" />
          </span>
          <Card.Title class="text-base">{f.title}</Card.Title>
          <Card.Description>{f.desc}</Card.Description>
        </a>
      </Card.Root>
    {/each}
  </div>

  {#if state.phase === 'error'}
    <Alert.Root variant="destructive">
      <Alert.Title>Backend unreachable</Alert.Title>
      <Alert.Description>Could not load source status from /rail-api/source-status.</Alert.Description>
    </Alert.Root>
  {/if}
</section>
