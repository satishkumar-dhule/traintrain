<script>
  import { cn } from '$lib/utils.js';

  /* status maps to the signal lamps; the fill rides currentColor of that tone. */
  let {
    from = '',
    to = '',
    progress = 0,
    status = 'run',
    class: className = ''
  } = $props();

  const tones = {
    run: 'text-signal-go',
    hold: 'text-signal-hold',
    stop: 'text-signal-stop'
  };

  const pct = $derived(Math.min(100, Math.max(0, (Number(progress) || 0) * 100)));
</script>

<div class={cn('flex items-center gap-3', className)}>
  <span class="data-num shrink-0 text-[11px] font-medium tracking-wide text-muted-foreground">{from}</span>
  <div
    class={cn('h-1 flex-1 overflow-hidden rounded-full bg-muted', tones[status] ?? tones.run)}
    role="progressbar"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={Math.round(pct)}
    aria-label={`${from} to ${to}`}
  >
    <div class="h-full rounded-full bg-current transition-all duration-500" style={`width:${pct}%`}></div>
  </div>
  <span class="data-num shrink-0 text-[11px] font-medium tracking-wide text-muted-foreground">{to}</span>
</div>
