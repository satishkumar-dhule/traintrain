<script module>
  /* Map a PNR booking/current status string to a semantic kind.
     Heads like "CNF", "RAC 12", "WL/GNWL 34" are recognized. */
  export function pnrStatusKind(status) {
    const t = String(status ?? '')
      .trim()
      .toUpperCase();
    if (!t || t === '-' || t === '--') return null;
    const head = t.split(/[\s/-]/)[0];
    if (head === 'CNF' || head === 'CONFIRMED') return 'confirmed';
    if (head === 'RAC') return 'rac';
    if (head === 'CAN' || head === 'CANCELLED') return 'cancelled';
    if (head === 'WL' || head === 'WAITLIST' || head === 'WAITLISTED') return 'waitlist';
    return 'other';
  }
</script>

<script>
  import StatusBadge from './status-badge.svelte';

  let {
    status,
    class: className = '',
    ...rest
  } = $props();

  const raw = $derived(String(status ?? '').trim());
  const kind = $derived(pnrStatusKind(raw));
  const text = $derived(raw && raw !== '-' && raw !== '--' ? raw : '—');
</script>

{#if kind === 'confirmed'}
  <StatusBadge tone="success" dot class={className} {...rest}>{text}</StatusBadge>
{:else if kind === 'rac'}
  <StatusBadge tone="warning" dot class={className} {...rest}>{text}</StatusBadge>
{:else if kind === 'waitlist' || kind === 'cancelled'}
  <StatusBadge tone="danger" dot class={className} {...rest}>{text}</StatusBadge>
{:else if kind === 'other'}
  <StatusBadge tone="neutral" class={className} {...rest}>{text}</StatusBadge>
{:else}
  <span class="text-muted-foreground">—</span>
{/if}
