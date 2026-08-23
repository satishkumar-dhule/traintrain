<script>
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import { StationCodeBadge } from '$lib/components/badges/index.js'
  import { nearby, chooseNearby, cancelNearby } from '$lib/nearby.svelte.js'
  import LocateFixedIcon from 'lucide-svelte/icons/locate-fixed'
  import LoaderCircleIcon from 'lucide-svelte/icons/loader-circle'

  // One global blocking dialog ("stations near you") shared by every
  // source/destination input. Rendered once in Layout; opened via
  // pickNearbyStation() from anywhere.
  const PHASE_TEXT = {
    locating: 'Detecting your location…',
    loading: 'Finding stations around you…'
  }

  function km(d) {
    const n = Number(d)
    return Number.isFinite(n) ? `${n.toFixed(1)} km` : ''
  }

  function onOpenChange(next) {
    if (!next) cancelNearby()
  }
</script>

<Dialog.Root open={nearby.open} onOpenChange={onOpenChange}>
  <Dialog.Content class="gap-3 sm:max-w-sm">
    <Dialog.Header class="space-y-1">
      <Dialog.Title class="flex items-center gap-2 text-base">
        <LocateFixedIcon class="size-4 text-primary" />
        Stations near you
      </Dialog.Title>
      <Dialog.Description class="text-xs">
        Pick your station from the list — sorted closest first.
      </Dialog.Description>
    </Dialog.Header>

    {#if nearby.phase === 'locating' || nearby.phase === 'loading'}
      <div class="grid gap-2 py-1" aria-busy="true">
        <p class="flex items-center gap-2 text-sm text-muted-foreground" role="status">
          <LoaderCircleIcon class="size-4 animate-spin" />
          {PHASE_TEXT[nearby.phase]}
        </p>
        {#if nearby.phase === 'loading'}
          {#each [0, 1, 2] as i (i)}
            <Skeleton class="h-9 w-full" />
          {/each}
        {/if}
      </div>
    {:else if nearby.phase === 'error'}
      <div class="grid gap-3 py-1">
        <p class="text-sm text-destructive" role="alert">{nearby.message}</p>
        <Dialog.Footer>
          <Button variant="outline" onclick={cancelNearby}>Close</Button>
        </Dialog.Footer>
      </div>
    {:else if nearby.phase === 'ready'}
      {#if nearby.stations.length === 0}
        <p class="py-1 text-sm text-muted-foreground">
          No stations found around your location.
        </p>
      {:else}
        <div class="-mx-1 max-h-72 overflow-y-auto px-1" role="listbox" aria-label="Stations near you">
          {#each nearby.stations as s (s.code)}
            <button
              type="button"
              role="option"
              aria-selected="false"
              onclick={() => chooseNearby(s)}
              class="flex min-h-11 w-full cursor-pointer items-center gap-2 rounded-md px-2 py-2 text-left transition-colors hover:bg-muted focus-visible:bg-muted focus-visible:outline-none"
            >
              <StationCodeBadge code={s.code} name={s.name} link={false} size="xs" />
              <span class="min-w-0 flex-1 truncate text-sm font-medium">{s.name}</span>
              <span class="text-xs tabular-nums text-muted-foreground">{km(s.distance_km)}</span>
            </button>
          {/each}
        </div>
      {/if}
    {/if}
  </Dialog.Content>
</Dialog.Root>
