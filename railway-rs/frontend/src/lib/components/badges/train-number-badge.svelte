<script>
  import StatusBadge from './status-badge.svelte';

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
</script>

<StatusBadge
  tone="neutral"
  {href}
  {size}
  title={name ? `${label} · ${name}` : label}
  class="font-mono tabular-nums tracking-tight {className}"
  aria-label="Train {label}"
  {...rest}
>
  {label}
</StatusBadge>
