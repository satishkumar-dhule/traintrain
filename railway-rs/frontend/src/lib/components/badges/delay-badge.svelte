<script module>
  import { parseDelay, delayKind } from '$lib/delay.js';
  export const parseDelayMinutes = parseDelay;
  export { delayKind };
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
