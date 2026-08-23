<script>
  import JourneysTable from './JourneysTable.svelte'

  let { src = '', dst = '' } = $props()
</script>

<section class="grid gap-6" class:idle-center={!src || !dst}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Find journeys</h1>
    <p class="max-lg:hidden text-sm text-muted-foreground">All trains between two stations, with weekly run days.</p>
  </div>
  <JourneysTable {src} {dst} />
</section>
