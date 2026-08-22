<script module>
  /* Map a train exception kind to a semantic tone. */
  export function exceptionKindKind(kind) {
    const t = String(kind ?? '')
      .trim()
      .toLowerCase();
    if (t === 'cancelled') return 'danger';
    if (t === 'rescheduled') return 'info';
    if (t === 'diverted' || t === 'shortterminated') return 'warning';
    return 'neutral';
  }
</script>

<script>
  import StatusBadge from './status-badge.svelte';

  let {
    kind,
    class: className = '',
    ...rest
  } = $props();

  const raw = $derived(String(kind ?? '').trim());
</script>

<StatusBadge tone={exceptionKindKind(raw)} dot={raw.toLowerCase() !== 'unknown'} class={className} {...rest}>
  {raw || 'unknown'}
</StatusBadge>
