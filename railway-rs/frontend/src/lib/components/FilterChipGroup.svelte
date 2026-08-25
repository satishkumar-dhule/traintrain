<script>
  // Super-optimized chip group with n2 super delegation (single handler fan-out)
  let {
    options = [], // [{value,label}]
    active = null, // single active value or Set/array for multi
    multi = false,
    size = 'sm',
    class: klass = '',
    onToggle = null // (value) => void - super delegate
  } = $props()

  const isActive = (v) => {
    if (multi) return Array.isArray(active) ? active.includes(v) : active instanceof Set ? active.has(v) : false
    return active === v
  }

  function handleClick(e) {
    const chip = e.target.closest('[data-chip]')
    if (!chip) return
    const v = chip.getAttribute('data-chip')
    if (v != null) onToggle?.(v)
  }
</script>

<div
  class={`flex flex-wrap items-center gap-1 ${klass}`}
  role="group"
  onclick={handleClick}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      const chip = e.target.closest('[data-chip]')
      if (chip) {
        e.preventDefault()
        handleClick(e)
      }
    }
  }}
>
  {#each options as opt (opt.value)}
    {@const val = String(opt.value)}
    {@const label = String(opt.label ?? opt.value)}
    {@const on = isActive(opt.value) || isActive(val)}
    <button
      type="button"
      data-chip={val}
      aria-pressed={on}
      class={`inline-flex items-center rounded-full border px-2.5 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
        on ? 'border-primary bg-primary text-primary-foreground' : 'border-border bg-card text-muted-foreground hover:bg-muted hover:text-foreground'
      } ${size === 'sm' ? 'h-6 text-xs' : 'h-7 text-xs'}`}
    >
      {label}
    </button>
  {/each}
</div>
