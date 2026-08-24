<script>
  import { cn } from '$lib/utils.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import SlidersHorizontalIcon from 'lucide-svelte/icons/sliders-horizontal'

  let {
    quota = 'GN',
    quotaOptions = [['GN', 'GN'], ['TQ', 'TQ'], ['PT', 'PT'], ['LD', 'LD']],
    onQuotaChange = () => {},
    sortBy = 'departure',
    sortOptions = [['departure', 'Departure'], ['duration', 'Duration'], ['fare', 'Fare'], ['chance', 'Confirm %']],
    onSortChange = () => {},
    sourceFilter = 'all',
    sourceOptions = [['all', 'All'], ['available', 'Available'], ['waitlist', 'WL']],
    onSourceChange = () => {},
    class: className = '',
  } = $props()
</script>

<div
  class={cn(
    'fixed inset-x-0 bottom-[4.5rem] z-30 border-t bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:hidden',
    className,
  )}
>
  <div class="mx-auto flex max-w-xl items-center gap-2 px-3 py-2">
    <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
      <SlidersHorizontalIcon class="size-4" />
      <span class="max-sm:hidden">Filter</span>
    </div>

    <Select.Root type="single" bind:value={quota} onchange={() => onQuotaChange(quota)}>
      <Select.Trigger class="h-8 min-w-[4rem] rounded-full border-border px-3 text-xs" aria-label="Quota">
        {quota}
      </Select.Trigger>
      <Select.Content>
        {#each quotaOptions as [val, label] (val)}
          <Select.Item value={val} {label} />
        {/each}
      </Select.Content>
    </Select.Root>

    <Select.Root type="single" bind:value={sortBy} onchange={() => onSortChange(sortBy)}>
      <Select.Trigger class="h-8 min-w-[4.5rem] rounded-full border-border px-3 text-xs" aria-label="Sort by">
        {sortOptions.find(([k]) => k === sortBy)?.[1] ?? 'Sort'}
      </Select.Trigger>
      <Select.Content>
        {#each sortOptions as [val, label] (val)}
          <Select.Item value={val} {label} />
        {/each}
      </Select.Content>
    </Select.Root>

    <Select.Root type="single" bind:value={sourceFilter} onchange={() => onSourceChange(sourceFilter)}>
      <Select.Trigger class="h-8 min-w-[4rem] rounded-full border-border px-3 text-xs" aria-label="Filter by status">
        {sourceOptions.find(([k]) => k === sourceFilter)?.[1] ?? 'All'}
      </Select.Trigger>
      <Select.Content>
        {#each sourceOptions as [val, label] (val)}
          <Select.Item value={val} {label} />
        {/each}
      </Select.Content>
    </Select.Root>
  </div>
</div>
