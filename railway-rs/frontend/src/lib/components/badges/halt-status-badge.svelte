<script module>
  /* Map a halt/run position status to a semantic kind. */
  export function haltStatusKind(status) {
    const t = String(status ?? '')
      .trim()
      .toLowerCase();
    if (!t) return null;
    if (t === 'departed' || t === 'arrived') return 'past';
    if (t === 'expected' || t === 'running' || t === 'ontime') return 'live';
    if (t === 'cancelled') return 'cancelled';
    return 'scheduled';
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
  const kind = $derived(haltStatusKind(raw));
</script>

{#if !raw}
  <span class="text-muted-foreground">—</span>
{:else if kind === 'past'}
  <StatusBadge tone="neutral" dot class={className} {...rest}>{raw}</StatusBadge>
{:else if kind === 'live'}
  <StatusBadge tone="info" dot class={className} {...rest}>{raw}</StatusBadge>
{:else if kind === 'cancelled'}
  <StatusBadge tone="danger" dot class={className} {...rest}>{raw}</StatusBadge>
{:else}
  <StatusBadge tone="outline" class={className} {...rest}>{raw}</StatusBadge>
{/if}
