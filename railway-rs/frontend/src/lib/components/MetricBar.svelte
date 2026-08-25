<script>
  // MetricBar — thin progress bar: h-1.5 bg-muted track → tone-colored fill at pct%.
  // Replaces 4 identical bars in System latency, Train avgDelay, Availability chance.
  let { pct = 0, label = '', tone = 'primary' } = $props()
  const toneMap = {
    primary: 'bg-primary',
    'signal-go': 'bg-signal-go',
    'signal-hold': 'bg-signal-hold',
    'signal-stop': 'bg-signal-stop'
  }
  let width = $derived(Math.min(100, Math.max(0, Number(pct) ?? 0)))
  let bg = $derived(toneMap[tone] ?? toneMap.primary)
</script>

<div class="flex items-center gap-2" role="progressbar" aria-valuenow={width} aria-valuemin="0" aria-valuemax="100">
  {#if label}<span class="min-w-12 text-right text-xs text-muted-foreground">{label}</span>{/if}
  <div class="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
    <div class={`h-full rounded-full ${bg} transition-all duration-500`} style={`width:${width}%`}></div>
  </div>
</div>
