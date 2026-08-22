<script module>
  /* Map a class-wise availability status to a semantic kind. */
  export function availabilityStatusKind(status) {
    const t = String(status ?? '')
      .trim()
      .toUpperCase();
    if (!t) return null;
    if (t.startsWith('AVAILABLE') || t.startsWith('AVBL')) return 'available';
    if (t.startsWith('RAC')) return 'rac';
    if (t.startsWith('WL')) return 'waitlist';
    if (t.startsWith('REGRET') || t.startsWith('CAN') || t.startsWith('NOT AVBL')) {
      return 'closed';
    }
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
  const kind = $derived(availabilityStatusKind(raw));
</script>

{#if !raw}
  <span class="text-muted-foreground">—</span>
{:else if kind === 'available'}
  <StatusBadge tone="success" dot class={className} {...rest}>{raw}</StatusBadge>
{:else if kind === 'rac'}
  <StatusBadge tone="warning" dot class={className} {...rest}>{raw}</StatusBadge>
{:else if kind === 'waitlist' || kind === 'closed'}
  <StatusBadge tone="danger" dot class={className} {...rest}>{raw}</StatusBadge>
{:else if kind === 'other'}
  <StatusBadge tone="neutral" class={className} {...rest}>{raw}</StatusBadge>
{:else}
  <span class="text-muted-foreground">{raw}</span>
{/if}
