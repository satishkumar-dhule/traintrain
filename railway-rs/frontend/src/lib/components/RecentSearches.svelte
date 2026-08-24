<script>
  import HistoryIcon from 'lucide-svelte/icons/history'
  import { Button } from '$lib/components/ui/button/index.js'

  let { title = 'Recent searches', items = [], onpick, onclear } = $props()
</script>

<div
  class="flex flex-wrap items-center gap-2 max-lg:-mx-4 max-lg:snap-x max-lg:snap-mandatory max-lg:flex-nowrap max-lg:items-center max-lg:gap-1.5 max-lg:overflow-x-auto max-lg:px-4 max-lg:[scrollbar-width:none] max-lg:[&::-webkit-scrollbar]:hidden"
>
  <span class="flex items-center gap-1.5 text-xs text-muted-foreground max-lg:shrink-0 max-lg:snap-start">
    <HistoryIcon class="size-3.5 shrink-0" aria-hidden="true" />
    {title}
  </span>
  {#each items as item (item.id)}
    <button
      type="button"
      class="inline-flex h-6 max-w-full items-center gap-1.5 rounded-full border px-2.5 text-xs transition-colors hover:bg-muted hover:text-foreground max-lg:h-11 max-lg:shrink-0 max-lg:snap-start max-lg:px-4"
      onclick={() => onpick?.(item)}
    >
      <span class="font-mono">{item.label}</span>
      {#if item.sub}<span class="max-w-40 truncate text-muted-foreground">{item.sub}</span>{/if}
    </button>
  {/each}
  <Button
    variant="ghost"
    size="xs"
    class="h-6 text-xs text-muted-foreground max-lg:h-11 max-lg:shrink-0 max-lg:snap-start max-lg:px-4"
    onclick={() => onclear?.()}
  >
    Clear all
  </Button>
</div>
