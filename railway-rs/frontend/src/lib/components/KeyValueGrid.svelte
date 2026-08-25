<script>
  import { cn } from '$lib/utils.js';

  /* row = { label, value, mono?, tone? }; tone: default|success|warning|danger */
  let {
    rows = [],
    class: className = ''
  } = $props();

  const tones = {
    default: 'text-foreground',
    success: 'text-signal-go-ink',
    warning: 'text-signal-hold-ink',
    danger: 'text-signal-stop-ink'
  };
</script>

<dl class={cn('grid gap-x-8 gap-y-2 sm:grid-cols-2', className)}>
  {#each rows as row, i (`${i}-${row?.label ?? ''}`)}
    <div class="flex min-w-0 items-baseline justify-between gap-3 border-b border-dashed border-border pb-1.5">
      <dt class="shrink-0 text-xs text-muted-foreground">{row?.label}</dt>
      <dd
        class={cn(
          'min-w-0 truncate text-right text-sm font-medium',
          tones[row?.tone] ?? tones.default,
          row?.mono && 'data-num'
        )}
      >
        {row?.value == null || row?.value === '' ? '—' : row.value}
      </dd>
    </div>
  {/each}
</dl>
