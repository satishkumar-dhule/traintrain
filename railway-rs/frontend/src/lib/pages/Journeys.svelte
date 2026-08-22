<script>
  import JourneysTable from './JourneysTable.svelte'
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Find journeys</h1>
    <p class="text-sm text-muted-foreground">All trains between two stations, with weekly run days.</p>
  </div>
  <JourneysTable />
</section>
