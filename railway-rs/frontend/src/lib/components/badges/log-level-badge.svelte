<script module>
  /* Map a log level to a semantic tone. */
  export function logLevelKind(level) {
    const t = String(level ?? '')
      .trim()
      .toUpperCase();
    if (t.includes('FATAL') || t.includes('ERROR')) return 'error';
    if (t.includes('WARN')) return 'warn';
    if (t.includes('INFO')) return 'info';
    return 'debug';
  }
</script>

<script>
  import StatusBadge from './status-badge.svelte';

  let {
    level,
    class: className = '',
    ...rest
  } = $props();

  const raw = $derived(String(level ?? '').trim());
  const kind = $derived(logLevelKind(raw));
</script>

{#if kind === 'error'}
  <StatusBadge tone="danger" dot class={className} {...rest}>{raw.toLowerCase() || 'unknown'}</StatusBadge>
{:else if kind === 'warn'}
  <StatusBadge tone="warning" dot class={className} {...rest}>{raw.toLowerCase()}</StatusBadge>
{:else if kind === 'info'}
  <StatusBadge tone="info" class={className} {...rest}>{raw.toLowerCase()}</StatusBadge>
{:else}
  <StatusBadge tone="neutral" class={className} {...rest}>
    {raw ? raw.toLowerCase() : 'unknown'}
  </StatusBadge>
{/if}
