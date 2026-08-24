<script>
  import { navigate } from '$lib/router.svelte.js'
  import { cn } from '$lib/utils.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import AvailabilityChip from '$lib/components/AvailabilityChip.svelte'
  import RouteIcon from 'lucide-svelte/icons/route'

  let {
    number = '',
    name = '',
    departureTime = '',
    arrivalTime = '',
    duration = '',
    runsOn = [],
    source = '',
    destination = '',
    availability = [],
    journeyDate = '',
    updatedAt = '',
    compact = false,
    class: className = '',
  } = $props()

  const DAY_LETTERS = ['S', 'M', 'T', 'W', 'T', 'F', 'S']

  function fmtDate(dateStr, timeStr) {
    const d = new Date(`${dateStr}T00:00:00`)
    if (isNaN(d.getTime())) return timeStr || '—'
    const day = d.toLocaleDateString('en-IN', { day: 'numeric' })
    const month = d.toLocaleDateString('en-IN', { month: 'short' })
    const weekday = d.toLocaleDateString('en-IN', { weekday: 'short' })
    return `${day} ${month}, ${weekday}, ${timeStr || ''}`
  }
</script>

<article class={cn('overflow-hidden rounded-xl border bg-card shadow-sm', className)}>
  <div class="p-4">
    <div class="flex items-start justify-between gap-2">
      <div class="min-w-0">
        <h3 class="truncate text-base font-semibold">
          {name || 'Unknown train'}
          {#if number}<span class="text-muted-foreground">({number})</span>{/if}
        </h3>
      </div>
      <Button
        type="button"
        variant="link"
        size="xs"
        class="shrink-0 text-xs"
        onclick={() => navigate(`/train/${number}`)}
      >
        <RouteIcon class="size-3" />
        Route
      </Button>
    </div>

    <div class="mt-2 flex items-center gap-2 text-sm">
      <span class="min-w-0 flex-1 text-left font-mono text-xs tabular-nums">
        {fmtDate(journeyDate, departureTime)}
      </span>
      <span class="shrink-0 text-center text-xs text-muted-foreground">{duration || '—'}</span>
      <span class="min-w-0 flex-1 text-right font-mono text-xs tabular-nums">
        {fmtDate(journeyDate, arrivalTime)}
      </span>
    </div>

    {#if runsOn.length > 0}
      <div class="mt-1 flex justify-center gap-1 text-[10px]">
        {#each DAY_LETTERS as d, i}
          <span class={cn('w-3 text-center', runsOn[i] ? 'font-semibold text-foreground' : 'text-muted-foreground/50')}>
            {d}
          </span>
        {/each}
      </div>
    {/if}

    <div class="mt-1 flex justify-between text-xs text-muted-foreground">
      <span>{source || '—'}</span>
      <span>{destination || '—'}</span>
    </div>

    {#if availability.length > 0}
      <div class="mt-3 flex flex-wrap gap-2">
        {#each availability as avl, i (i)}
          <AvailabilityChip
            classCode={avl.class ?? avl.code ?? ''}
            fare={avl.fare}
            status={avl.status ?? ''}
          />
        {/each}
      </div>
    {/if}

    {#if updatedAt}
      <p class="mt-2 text-[10px] text-muted-foreground">Updated {updatedAt}</p>
    {/if}
  </div>
</article>
