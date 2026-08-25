<script>
  // Gauge — donut with R=26 CIRC dasharray (mirrors About 247-268).
  // Signal & Steel tokens; width 64 viewBox preserves R geometry.
  let { value = 0, max = 100, label = '' } = $props()
  const R = 26
  const CIRC = 2 * Math.PI * R
  let frac = $derived(max > 0 ? Math.min(1, Math.max(0, Number(value) / Number(max))) : 0)
  let dash = $derived(`${(CIRC * frac).toFixed(1)} ${CIRC.toFixed(1)}`)
  let display = $derived(Number.isFinite(Number(value)) ? value : '—')
</script>

<div class="flex items-center justify-between gap-2">
  <span class="grid gap-0.5">
    {#if label}<span class="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">{label}</span>{/if}
    <span class="data-num text-lg font-semibold">{display}</span>
  </span>
  <svg viewBox="0 0 64 64" width="64" height="64" class="size-12 shrink-0 -rotate-90" aria-hidden="true">
    <circle cx="32" cy="32" r={R} fill="none" stroke="currentColor" stroke-width="7" class="text-muted" />
    <circle cx="32" cy="32" r={R} fill="none" stroke="currentColor" stroke-width="7" stroke-linecap="round" stroke-dasharray={dash} class="text-primary transition-all duration-700" />
  </svg>
</div>
