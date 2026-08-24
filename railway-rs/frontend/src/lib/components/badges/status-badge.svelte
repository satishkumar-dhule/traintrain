<script module>
  /* Semantic tone palette on the Signal & Steel tokens: signal lamps for
     go/hold/stop, coach indigo tint for info. Text uses the -ink variants
     so contrast stays AA in both themes. */
  export const STATUS_TONES = {
    success:
      'border-signal-go/30 bg-signal-go/10 text-signal-go-ink dark:border-signal-go/40 dark:bg-signal-go/15',
    warning:
      'border-signal-hold/40 bg-signal-hold/15 text-signal-hold-ink dark:border-signal-hold/45 dark:bg-signal-hold/20',
    danger:
      'border-signal-stop/30 bg-signal-stop/10 text-signal-stop-ink dark:border-signal-stop/40 dark:bg-signal-stop/15',
    info: 'border-primary/25 bg-primary/10 text-primary dark:border-primary/35 dark:bg-primary/15',
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
