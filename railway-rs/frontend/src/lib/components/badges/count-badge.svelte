<script>
  import StatusBadge from './status-badge.svelte';

  let {
    value,
    label = '',
    tone = 'neutral',
    dot = false,
    class: className = '',
    ...rest
  } = $props();

  const n = $derived(Number(value));
  const count = $derived(Number.isFinite(n) ? n : null);
  const shown = $derived(count === null ? String(value ?? '—') : count.toLocaleString());
  /* Pluralize: "12 trains" / "1 train". Only strips a trailing "s" (never
     touches words like "class" or "stops" → "stop"). */
  const unit = $derived(
    label && count === 1 && label.endsWith('s') && !label.endsWith('ss')
      ? label.slice(0, -1)
      : label
  );
</script>

<StatusBadge {tone} {dot} class={className} {...rest}>
  {unit ? `${shown} ${unit}` : shown}
</StatusBadge>
