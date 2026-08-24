<script>
  import { theme, toggleTheme } from '$lib/theme.svelte.js'
  import Sun from 'lucide-svelte/icons/sun'
  import Moon from 'lucide-svelte/icons/moon'

  let { compact = false } = $props()

  const Icon = $derived(theme.mode === 'dark' ? Sun : Moon)
  const next = $derived(theme.mode === 'dark' ? 'Bright' : 'Dark')
</script>

<button
  type="button"
  class={
    compact
      ? 'flex size-11 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground'
      : 'flex min-h-11 w-full items-center gap-3 rounded-lg px-3 text-[0.95rem] text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground'
  }
  onclick={toggleTheme}
  aria-label={`Switch to ${next.toLowerCase()} theme`}
  title={`Switch to ${next.toLowerCase()} theme`}
>
  <Icon class="size-5 shrink-0" />
  {#if !compact}
    <span>{next} theme</span>
  {/if}
</button>
