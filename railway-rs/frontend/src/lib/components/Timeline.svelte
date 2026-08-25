<script>
  import SignalDot from '$lib/components/SignalDot.svelte';
  import { cn } from '$lib/utils.js';

  /* stop = { name, code, arr?, dep?, halt?, platform?, status? }
     status: done | live | upcoming | cancelled */
  let {
    stops = [],
    /* Optional snippet (stop) rendered under the stop title for extra facts. */
    meta = null,
    class: className = ''
  } = $props();

  const lamps = {
    done: { tone: 'go' },
    live: { tone: 'go', pulse: true },
    upcoming: { tone: 'idle' },
    cancelled: { tone: 'stop' }
  };
</script>

<ol class={cn('grid', className)}>
  {#each stops as stop, i (`${i}-${stop?.code ?? ''}-${stop?.name ?? ''}`)}
    {@const lamp = lamps[stop?.status] ?? lamps.upcoming}
    {@const last = i === stops.length - 1}
    <li class="flex gap-3" aria-current={stop?.status === 'live' ? 'location' : undefined}>
      {/* Rail column stretches with the row so the hairline reaches the next lamp. */}
      <div class="flex w-2 flex-col items-center">
        <span class="mt-1.5"><SignalDot tone={lamp.tone} pulse={lamp.pulse} /></span>
        {#if !last}
          <span class="w-px flex-1 border-l border-border" aria-hidden="true"></span>
        {/if}
      </div>
      <div class={cn('min-w-0 flex-1 pb-4', last && 'pb-0')}>
        <div class="flex items-baseline justify-between gap-3">
          <span
            class={cn(
              'min-w-0 truncate text-sm',
              stop?.status === 'live' && 'font-semibold',
              (stop?.status === 'done' || stop?.status === 'cancelled') &&
                'text-muted-foreground',
              stop?.status === 'cancelled' &&
                'line-through decoration-muted-foreground/50'
            )}
          >
            {stop?.name}
            {#if stop?.code}
              <span class="data-num ml-1 text-[11px] tracking-wide text-muted-foreground">{stop.code}</span>
            {/if}
          </span>
          {#if stop?.arr || stop?.dep}
            <span class="shrink-0 whitespace-nowrap text-xs">
              {#if stop?.arr}<span class="data-num text-muted-foreground">{stop.arr}</span>{/if}
              {#if stop?.arr && stop?.dep}<span class="mx-1 text-muted-foreground">–</span>{/if}
              {#if stop?.dep}<span class="data-num font-medium">{stop.dep}</span>{/if}
            </span>
          {/if}
        </div>
        {#if stop?.halt || stop?.platform}
          <div class="data-num mt-0.5 text-[11px] text-muted-foreground">
            {#if stop?.halt}halt {stop.halt}{/if}
            {#if stop?.halt && stop?.platform} · {/if}
            {#if stop?.platform}pf {stop.platform}{/if}
          </div>
        {/if}
        {@render meta?.(stop)}
      </div>
    </li>
  {/each}
</ol>
