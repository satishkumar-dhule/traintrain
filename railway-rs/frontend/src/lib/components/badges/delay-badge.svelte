<script module>
  /* Parse a delay value into whole minutes. Accepts numbers or strings like
     "5", "5m", "05 min". Returns null when the value is absent/unparseable. */
  export function parseDelayMinutes(v) {
    if (v == null) return null;
    if (typeof v === 'number') return Number.isFinite(v) ? Math.round(v) : null;
    const s = String(v).trim();
    if (!s || s === '-' || s === '--') return null;
    const m = /^[-+]?\d+(\.\d+)?\s*(m|min|mins|minutes?)?$/i.exec(s);
    if (!m) return null;
    const n = Math.round(parseFloat(s));
    return Number.isFinite(n) ? n : null;
  }

  /* Severity tier for a delay in minutes: on-time / minor / major. */
  export function delayKind(minutes) {
    if (minutes == null) return 'none';
    if (minutes <= 0) return 'ontime';
    return minutes <= 15 ? 'minor' : 'major';
  }
</script>

<script>
  import StatusBadge from './status-badge.svelte';

  let {
    minutes,
    compact = false,
    class: className = '',
    ...rest
  } = $props();

  const mins = $derived(parseDelayMinutes(minutes));
  const kind = $derived(delayKind(mins));
</script>

{#if kind === 'none'}
  <StatusBadge tone="outline" class={className} {...rest}>—</StatusBadge>
{:else if kind === 'ontime'}
  <StatusBadge tone="success" dot class={className} {...rest}>on time</StatusBadge>
{:else}
  <StatusBadge
    tone={kind === 'minor' ? 'warning' : 'danger'}
    dot
    title={`${mins} minute${Math.abs(mins) === 1 ? '' : 's'} late`}
    class={className}
    {...rest}
  >
    {compact ? `${mins}m late` : `${mins} min late`}
  </StatusBadge>
{/if}
