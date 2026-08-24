<script>
  import StatusBadge from './status-badge.svelte';
  import { prefetch } from '$lib/api.js';

  let {
    number,
    name = '',
    link = true,
    size = 'sm',
    class: className = '',
    ...rest
  } = $props();

  const n = $derived(String(number ?? '').trim());
  const href = $derived(link && n ? `/train/${encodeURIComponent(n)}` : '');
  const label = $derived(n || '—');

  let warmedHref = '';
  function warm() {
    if (!href || warmedHref === href) return;
    warmedHref = href;
    prefetch(`/rail-api/schedule?train=${encodeURIComponent(n)}`);
  }
</script>

<StatusBadge
  tone="neutral"
  {href}
  {size}
  title={name ? `${label} · ${name}` : label}
  class="data-num tracking-tight {className}"
  aria-label="Train {label}"
  onpointerenter={warm}
  ontouchstart={warm}
  {...rest}
>
  {label}
</StatusBadge>
