<script>
  import { cn } from '$lib/utils.js';

  let {
    title,
    description = '',
    actions = undefined,
    children = undefined,
    size = 'default',
    class: className = '',
  } = $props();
</script>

<div class={cn('flex flex-wrap items-start justify-between gap-3 overflow-hidden', className)}>
  <div class="grid min-w-0 gap-0.5 overflow-hidden">
    <h1 class={cn('font-semibold tracking-tight', size === 'sm' ? 'text-xl' : 'text-2xl sm:text-3xl')}>{title}</h1>
    {#if description}<p class="max-lg:hidden text-sm text-muted-foreground">{description}</p>{/if}
    {@render children?.()}
  </div>
  {#if actions}<div class="flex flex-wrap items-center gap-2">{@render actions()}</div>{/if}
</div>
