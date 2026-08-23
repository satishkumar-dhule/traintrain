<script module>
  /* Semantic tone palette. Soft tinted chips that hold up in light and dark
     themes; text uses -700/-400 pairs so contrast stays AA on both. */
  export const STATUS_TONES = {
    success:
      'border-emerald-600/25 bg-emerald-500/10 text-emerald-700 dark:border-emerald-500/35 dark:bg-emerald-400/10 dark:text-emerald-400',
    warning:
      'border-amber-600/30 bg-amber-500/10 text-amber-700 dark:border-amber-500/35 dark:bg-amber-400/10 dark:text-amber-400',
    danger:
      'border-red-600/30 bg-red-500/10 text-red-700 dark:border-red-500/35 dark:bg-red-400/10 dark:text-red-400',
    info: 'border-sky-600/30 bg-sky-500/10 text-sky-700 dark:border-sky-500/35 dark:bg-sky-400/10 dark:text-sky-400',
    neutral: 'border-border bg-muted/50 text-muted-foreground',
    outline: 'border-border text-foreground'
  };
</script>

<script>
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { navigate } from '$lib/router.svelte.js';
  import { cn } from '$lib/utils.js';

  let {
    tone = 'neutral',
    dot = false,
    size = 'sm',
    href = '',
    title = '',
    class: className = '',
    children,
    ...rest
  } = $props();

  const sizeClass = $derived(
    size === 'xs'
      ? 'px-1.5 text-[10px] max-lg:min-w-10 max-lg:px-2 max-lg:text-xs'
      : 'max-lg:min-w-10'
  );

  function onclick(e) {
    if (!href) return;
    if (
      e.defaultPrevented ||
      e.button !== 0 ||
      e.metaKey ||
      e.ctrlKey ||
      e.shiftKey ||
      e.altKey
    ) {
      return;
    }
    e.preventDefault();
    navigate(href);
  }
</script>

<svelte:element
  this={href ? 'a' : 'span'}
  data-slot="status-badge"
  data-tone={tone}
  {href}
  {...title ? { title } : {}}
  class={cn(
    'inline-flex items-center justify-center',
    tone === 'outline' ? '' : STATUS_TONES[tone] ?? STATUS_TONES.neutral,
    sizeClass,
    href && 'hit-y cursor-pointer',
    className
  )}
  variant="outline"
  {onclick}
  {...rest}
>
  {#if dot}<span class="size-1.5 shrink-0 rounded-full bg-current opacity-80"></span>{/if}
  {@render children?.()}
</svelte:element>
