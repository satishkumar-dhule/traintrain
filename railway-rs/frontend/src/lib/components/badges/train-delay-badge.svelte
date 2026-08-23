<script>
  import StatusBadge from './status-badge.svelte';
  import { trainDelayEntry } from '$lib/trainDelay.svelte.js';
  import {
    classifyTrainKind,
    kindLabel,
    avgDelayPattern,
    kindDelayKind
  } from '$lib/train-kind.js';

  /* Avg-delay badge for any displayed train, calibrated to the kind of train
     (premium / superfast / fast express / slow passenger). Renders nothing
     until data lands; failures stay silent so lists don't fill with n/a. */
  let {
    number,
    name = '',
    type = '',
    data = null,
    compact = false,
    class: className = '',
    ...rest
  } = $props();

  /* Pages that already hold an AverageDelayResponse can pass it via `data`
     to skip the shared fetch. */
  const entry = $derived(data ? { status: 'ok', data } : trainDelayEntry(number));

  const pattern = $derived(
    entry?.status === 'ok' ? avgDelayPattern(entry.data?.stations) : null
  );
  const mins = $derived(pattern?.mean ?? null);
  const kind = $derived(classifyTrainKind({ number, name, type: type || entry?.data?.train_type }));
  const tier = $derived(kindDelayKind(kind, mins));
</script>

{#if tier !== 'none'}
  {@const worstTxt = pattern?.worst != null ? `, worst ${pattern.worst} min` : ''}
  <StatusBadge
    tone={tier === 'good' ? 'success' : tier === 'minor' ? 'warning' : 'danger'}
    dot
    size={compact ? 'xs' : 'sm'}
    title={`${kindLabel(kind)} · avg delay ~${mins} min over ${pattern.samples} readings${worstTxt}`}
    class={className}
    {...rest}
  >
    {#if mins === 0}
      on time
    {:else if compact}
      ~{mins}m
    {:else}
      ~{mins}m avg
    {/if}
  </StatusBadge>
{/if}
