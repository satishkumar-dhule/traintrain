<script>
  import { theme, setTheme, contrast, setContrast } from '$lib/theme.svelte.js'
  import Sun from 'lucide-svelte/icons/sun'
  import Moon from 'lucide-svelte/icons/moon'
  import Monitor from 'lucide-svelte/icons/monitor'

  let { placement = 'up', compact = false } = $props()

  let open = $state(false)
  let wrap
  let trigger

  const themeIcons = { system: Monitor, light: Sun, dark: Moon }
  const TriggerIcon = $derived(themeIcons[theme.mode] ?? Monitor)
  const themes = [
    ['system', 'System'],
    ['light', 'Light'],
    ['dark', 'Dark']
  ]
  const contrasts = [
    ['off', 'Normal'],
    ['high', 'High'],
    ['invert', 'Invert']
  ]

  function toggle() {
    open = !open
  }

  function seg(active) {
    return `rounded-[calc(var(--radius)-4px)] px-2 py-1 text-xs transition-colors ${
      active ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground'
    }`
  }

  $effect(() => {
    if (!open) return
    const onPointer = (e) => {
      if (wrap && !wrap.contains(e.target)) open = false
    }
    const onKey = (e) => {
      if (e.key === 'Escape') {
        open = false
        trigger?.focus()
      }
    }
    document.addEventListener('pointerdown', onPointer)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('pointerdown', onPointer)
      document.removeEventListener('keydown', onKey)
    }
  })
</script>

<div class="relative" bind:this={wrap}>
  <button
    type="button"
    bind:this={trigger}
    class={
      compact
        ? 'rounded-md p-2 text-muted-foreground hover:bg-accent'
        : 'flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground'
    }
    onclick={toggle}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label="Display settings"
  >
    <TriggerIcon class="size-4" />
    {#if !compact}
      <span>Display</span>
      <span class="ml-auto text-xs capitalize">{theme.mode}</span>
    {/if}
  </button>

  {#if open}
    <div
      role="dialog"
      aria-label="Display settings"
      class={`absolute z-50 w-44 rounded-lg border bg-popover p-3 text-popover-foreground shadow-md ${
        placement === 'up' ? 'bottom-full left-0 mb-2' : 'right-0 top-full mt-2'
      }`}
    >
      <p class="mb-1 text-xs font-medium text-muted-foreground">Theme</p>
      <div class="grid grid-cols-3 gap-1 rounded-md bg-muted p-1">
        {#each themes as [value, label] (value)}
          <button
            type="button"
            class={seg(theme.mode === value)}
            aria-pressed={theme.mode === value}
            onclick={() => setTheme(value)}
          >
            {label}
          </button>
        {/each}
      </div>
      <p class="mb-1 mt-3 text-xs font-medium text-muted-foreground">Contrast</p>
      <div class="grid grid-cols-3 gap-1 rounded-md bg-muted p-1">
        {#each contrasts as [value, label] (value)}
          <button
            type="button"
            class={seg(contrast.mode === value)}
            aria-pressed={contrast.mode === value}
            onclick={() => setContrast(value)}
          >
            {label}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>
