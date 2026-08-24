<script>
  import { cn } from '$lib/utils.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import SearchIcon from 'lucide-svelte/icons/search'
  import Volume2Icon from 'lucide-svelte/icons/volume-2'
  import MicIcon from 'lucide-svelte/icons/mic'

  let { count = 0, query = $bindable(''), class: className = '' } = $props()
</script>

<div
  class={cn(
    'flex items-center gap-2 rounded-xl border bg-card px-3 py-2 shadow-sm',
    className,
  )}
>
  <Badge variant="secondary" class="shrink-0 rounded-full px-2.5 py-1 text-xs tabular-nums">
    {count} trains found
  </Badge>

  <button type="button" class="shrink-0 text-muted-foreground transition-colors hover:text-foreground" aria-label="Announce results">
    <Volume2Icon class="size-4" />
  </button>

  <div class="relative min-w-0 flex-1">
    <SearchIcon class="pointer-events-none absolute top-1/2 left-2 size-4 -translate-y-1/2 text-muted-foreground" />
    <input
      type="text"
      bind:value={query}
      placeholder="Train Name/Number"
      class="h-8 w-full rounded-md bg-transparent pl-8 pr-2 text-sm placeholder:text-muted-foreground focus-visible:outline-none"
    />
  </div>

  <button type="button" class="shrink-0 text-muted-foreground transition-colors hover:text-foreground" aria-label="Voice search">
    <MicIcon class="size-4" />
  </button>
</div>
