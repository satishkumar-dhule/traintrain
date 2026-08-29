<script>
  import { route } from '$lib/router.svelte.js'
  import Layout from '$lib/Layout.svelte'
  import Home from '$lib/pages/Home.svelte'
  import Train from '$lib/pages/Train.svelte'
  import Station from '$lib/pages/Station.svelte'
  import Plan from '$lib/pages/Plan.svelte'
  import Pnr from '$lib/pages/Pnr.svelte'
  import Exceptions from '$lib/pages/Exceptions.svelte'
  import Extras from '$lib/pages/Extras.svelte'
  import Assistant from '$lib/pages/Assistant.svelte'
  import System from '$lib/pages/System.svelte'
import Kaizen from '$lib/pages/Kaizen.svelte'
import About from '$lib/pages/About.svelte'
import Chart from '$lib/pages/Chart.svelte'

  const segs = $derived(route.path.split('/').filter(Boolean))
</script>

<Layout>
  {#if segs.length === 0}
    <Home />
  {:else if segs[0] === 'train'}
    <Train number={segs[1] ?? ''} view={segs[2] ?? ''} />
  {:else if segs[0] === 'station'}
    <Station code={segs[1] ?? ''} view={segs[2] ?? ''} />
  {:else if segs[0] === 'plan'}
    <Plan src={segs[1] ?? ''} dst={segs[2] ?? ''} date={segs[3] ?? ''} tab={segs[1] === 'availability' ? 'availability' : 'trains'} />
  {:else if segs[0] === 'journeys'}
    <Plan src={segs[1] ?? ''} dst={segs[2] ?? ''} tab="trains" />
  {:else if segs[0] === 'availability'}
    <Plan src={segs[1] ?? ''} dst={segs[2] ?? ''} date={segs[3] ?? ''} tab="availability" />
  {:else if segs[0] === 'pnr'}
    <Pnr pnr={segs[1] ?? ''} />
  {:else if segs[0] === 'chart'}
    <Chart train={segs[1] ?? ''} date={segs[2] ?? ''} station={segs[3] ?? ''} />
  {:else if segs[0] === 'exceptions'}
    <Exceptions />
  {:else if segs[0] === 'extras'}
    <Extras view={segs[1] ?? ''} selection={segs[2] ?? ''} />
  {:else if segs[0] === 'assistant'}
    <Assistant seed={segs[1] ? decodeURIComponent(segs[1]) : ''} />
  {:else if segs[0] === 'system'}
    <System />
  {:else if segs[0] === 'kaizen'}
    <Kaizen />
  {:else if segs[0] === 'about'}
    <About />
  {:else}
    <section class="grid gap-2">
      <h1 class="text-3xl font-semibold tracking-tight">Page not found</h1>
      <p class="text-muted-foreground">No route matches <code class="font-mono">{route.path}</code>.</p>
    </section>
  {/if}
</Layout>
