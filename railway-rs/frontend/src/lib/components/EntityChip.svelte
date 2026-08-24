<script>
  import { navigate } from '$lib/router.svelte.js';
  import { StationCodeBadge, TrainNumberBadge } from '$lib/components/badges/index.js';
  import { stationHref, trainHref } from '$lib/utils.js';
  import { cn } from '$lib/utils.js';

  let {
    type,
    code,
    name = '',
    view = '',
    size = 'xs',
    class: className = '',
  } = $props();

  const href = $derived(type === 'station' ? stationHref(code, view || 'live') : trainHref(code, view));
  const sizeCls = $derived(size === 'sm' ? 'text-sm' : 'text-xs');

  function go(e) {
    e.preventDefault();
    navigate(href);
  }
</script>

<button type="button" onclick={go} class={cn('inline-flex items-center gap-1.5 rounded-full border border-border bg-background px-2 py-0.5 font-medium transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring', sizeCls, className)} title={name || code} aria-label={name || code}>
  {#if type === 'station'}<StationCodeBadge code={code} name={name} link={false} {size} />{:else}<TrainNumberBadge number={code} name={name} link={false} {size} />{/if}
  {#if name && size !== 'xs'}<span class="truncate">{name}</span>{/if}
</button>
