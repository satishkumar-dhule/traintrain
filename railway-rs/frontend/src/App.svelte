<script>
  import { route } from '$lib/router.svelte.js'
  import Layout from '$lib/Layout.svelte'
  import Home from '$lib/pages/Home.svelte'
  import Train from '$lib/pages/Train.svelte'
  import Station from '$lib/pages/Station.svelte'
  import Journeys from '$lib/pages/Journeys.svelte'
  import Pnr from '$lib/pages/Pnr.svelte'
  import System from '$lib/pages/System.svelte'

  const segs = $derived(route.path.split('/').filter(Boolean))
</script>

<Layout>
  {#if segs.length === 0}
    <Home />
  {:else if segs[0] === 'train'}
    <Train number={segs[1] ?? ''} />
  {:else if segs[0] === 'station'}
    <Station code={segs[1] ?? ''} />
  {:else if segs[0] === 'journeys'}
    <Journeys />
  {:else if segs[0] === 'pnr'}
    <Pnr />
  {:else if segs[0] === 'system'}
    <System />
  {:else}
    <section class="grid gap-2">
      <h1 class="text-3xl font-semibold tracking-tight">Page not found</h1>
      <p class="text-muted-foreground">No route matches <code class="font-mono">{route.path}</code>.</p>
    </section>
  {/if}
</Layout>
