<script>
  import StatusBadge from './status-badge.svelte';

  let {
    source = '',
    cached = false,
    freshness = '',
    class: className = '',
    ...rest
  } = $props();

  const src = $derived(String(source ?? '').trim());
  const fresh = $derived(String(freshness ?? '').trim());
  const empty = $derived(!src && !fresh && !cached);
</script>

 {#if !empty}
  <span class={`inline-flex flex-wrap items-center gap-1.5 max-lg:hidden ${className}`} {...rest}>
    {#if fresh}
      <StatusBadge tone="info" size="xs" title="How current this data is">{fresh}</StatusBadge>
    {/if}
    {#if cached}
      <StatusBadge tone="neutral" dot size="xs" title="Served from cache">cached</StatusBadge>
    {/if}
    {#if src}
      <StatusBadge tone="outline" size="xs" title="Upstream data source">{src}</StatusBadge>
    {/if}
  </span>
{/if}
