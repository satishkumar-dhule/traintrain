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
    success: 'text-signal-go-ink',
    warning: 'text-signal-hold-ink',
    danger: 'text-signal-stop-ink',
  };
</script>

<span class={cn('inline-flex items-baseline gap-1.5 rounded-full border border-border bg-muted/40 px-2.5 py-1 text-xs', className)}>
  {#if label}<span class="text-muted-foreground">{label}</span>{/if}
  <span class={cn('font-medium tabular-nums', toneValue[tone] ?? toneValue.default)}>{value}</span>
  {@render icon?.()}
</span>
