<script>
  import StatusBadge from './status-badge.svelte';

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
</script>

<StatusBadge
  tone="neutral"
  {href}
  {size}
  title={name ? `${label} · ${name}` : label}
  class="font-mono tracking-widest uppercase {className}"
  aria-label="Station {label}"
  {...rest}
>
  {label}
</StatusBadge>
