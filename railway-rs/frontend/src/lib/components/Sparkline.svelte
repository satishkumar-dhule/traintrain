<script>
  let { values = [], variant = 'bar', color = 'bg-primary' } = $props()
  let nums = $derived((Array.isArray(values) ? values : []).map(Number).filter((v) => Number.isFinite(v)).slice(-60))
  let max = $derived(Math.max(0, ...nums))
  let barPts = $derived(nums.map((v, i) => ({ pct: max > 0 ? Math.min(100, (Math.max(v, 0) / max) * 100) : 0, op: nums.length > 1 ? 0.2 + 0.8 * (i / (nums.length - 1)) : 1 })))
  function lineData(vals, w = 100, h = 24, pad = 2) {
    if (vals.length < 2) return null
    const min = Math.min(...vals)
    const span = Math.max(...vals) - min || 1
    const dx = (w - pad * 2) / (vals.length - 1)
    const pts = vals.map((v, i) => `${(pad + i * dx).toFixed(1)},${(h - pad - ((v - min) / span) * (h - pad * 2)).toFixed(1)}`)
    return { line: pts.join(' '), area: `M${pad},${h} L${pts.join(' L')} L${w - pad},${h} Z`, last: pts[pts.length - 1].split(',').map(Number) }
  }
  let spark = $derived(lineData(nums, 100, 24, 2))
</script>

{#if variant === 'bar'}
  {#if barPts.length}
    <div class="flex h-6 items-end gap-[2px]" role="img" aria-label="sparkline bar">
      {#each barPts as p, i (i)}
        <div class={`min-w-[2px] flex-1 rounded-sm ${color}`} style={`height:${p.pct}%;opacity:${p.op};${p.pct > 0 ? 'min-height:2px;' : ''}`}></div>
      {/each}
    </div>
  {/if}
{:else if spark}
  <svg viewBox="0 0 100 24" width="100" height="24" class="w-full" preserveAspectRatio="none" aria-hidden="true">
    <path d={spark.area} fill="currentColor" class={`${color} opacity-15`} />
    <polyline points={spark.line} fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class={color} />
    <circle cx={spark.last[0]} cy={spark.last[1]} r="2" fill="currentColor" class={color} />
  </svg>
{/if}
