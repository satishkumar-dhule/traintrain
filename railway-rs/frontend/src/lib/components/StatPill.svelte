<script>
  import { cn } from '$lib/utils.js';

  let {
    label = '',
    value,
    icon = undefined,
    tone = 'default',
    class: className = '',
  } = $props();

  const toneValue = {
    default: 'text-foreground',
    success: 'text-emerald-600 dark:text-emerald-400',
    warning: 'text-amber-600 dark:text-amber-400',
    danger: 'text-red-600 dark:text-red-400',
  };
</script>

<span class={cn('inline-flex items-baseline gap-1.5 rounded-full border border-border bg-muted/40 px-2.5 py-1 text-xs', className)}>
  {#if label}<span class="text-muted-foreground">{label}</span>{/if}
  <span class={cn('font-medium tabular-nums', toneValue[tone] ?? toneValue.default)}>{value}</span>
  {@render icon?.()}
</span>
