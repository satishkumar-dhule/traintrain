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
  import Activity from 'lucide-svelte/icons/activity'

  let state = $state({ phase: 'loading', data: null })

  $effect(() => {
    api('/rail-api/source-status').then((res) => {
      state = res.ok ? { phase: 'ok', data: res.data } : { phase: 'error', data: null }
    })
  })

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
