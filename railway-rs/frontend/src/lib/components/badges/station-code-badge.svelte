<script>
  import StatusBadge from './status-badge.svelte';
  import { prefetch } from '$lib/api.js';

  let {
    code,
    name = '',
    link = true,
    size = 'sm',
    class: className = '',
    ...rest
  } = $props();

  const c = $derived(String(code ?? '').trim().toUpperCase());
  const href = $derived(link && c ? `/station/${encodeURIComponent(c)}` : '');
  const label = $derived(c || '—');

  let warmedHref = '';
  function warm() {
    if (!href || warmedHref === href) return;
    warmedHref = href;
    prefetch(`/rail-api/stations/${encodeURIComponent(c)}`);
  }
</script>

<StatusBadge
  tone="neutral"
  {href}
  {size}
  title={name ? `${label} · ${name}` : label}
  class="font-mono tracking-widest uppercase {className}"
  aria-label="Station {label}"
  onpointerenter={warm}
  ontouchstart={warm}
  {...rest}
>
  {label}
</StatusBadge>
