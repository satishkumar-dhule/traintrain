<script>
  import { DataSourceBadge } from '$lib/components/badges/index.js';
  import { cn } from '$lib/utils.js';

  let {
    children = undefined,
    source = '',
    actions = undefined,
    class: className = '',
  } = $props();
</script>

<div class={cn('flex flex-wrap items-center gap-x-3 gap-y-1.5 rounded-lg border bg-card px-3 py-2 text-xs', className)}>
  {@render children?.()}
  <div class="ml-auto flex items-center gap-2">
    {@render actions?.()}
    {#if source}<DataSourceBadge source={source} />{/if}
  </div>
</div>
