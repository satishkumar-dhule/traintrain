<script>
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import ChevronLeftIcon from 'lucide-svelte/icons/chevron-left'
  import ChevronRightIcon from 'lucide-svelte/icons/chevron-right'

  let {
    value = $bindable(''),
    id = 'date-strip-input',
    label = 'Journey date',
    min = '',
    max = '',
    class: klass = '',
    onchange
  } = $props()

  const DATE_RE = /^\d{4}-\d{2}-\d{2}$/
  const MAX_DAY = 364
  const STRIP_WINDOW = 15

  function today() {
    const d = new Date()
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  }

  function isoShift(iso, days) {
    const d = new Date(`${iso}T00:00:00`)
    d.setDate(d.getDate() + days)
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  }

  function diffDays(a, b) {
    return Math.round((new Date(`${b}T00:00:00`) - new Date(`${a}T00:00:00`)) / 86400000)
  }

  function weekdayShort(iso) {
    const d = new Date(`${iso}T00:00:00`)
    return isNaN(d.getTime()) ? '' : d.toLocaleDateString('en-IN', { weekday: 'short' })
  }

  function monthShort(iso) {
    const d = new Date(`${iso}T00:00:00`)
    return isNaN(d.getTime()) ? '' : d.toLocaleDateString('en-IN', { month: 'short' })
  }

  const TODAY = today()
  const MIN = $derived(DATE_RE.test(String(min ?? '')) ? min : TODAY)
  const MAX = $derived(DATE_RE.test(String(max ?? '')) ? max : isoShift(TODAY, MAX_DAY))

  // Empty/invalid parent values fall back to Today for strip positioning only;
  // the bound value is only written when the user picks a date.
  const sel = $derived(DATE_RE.test(String(value ?? '')) ? value : TODAY)

  // Windowed date strip: ~STRIP_WINDOW day-buttons centered on the selection.
  // Bounded DOM width keeps the grid track from being blown out by min-content.
  const stripStart = $derived.by(() => {
    const idx = Math.max(0, diffDays(TODAY, sel))
    return Math.max(0, Math.min(idx - Math.floor(STRIP_WINDOW / 2), MAX_DAY + 1 - STRIP_WINDOW))
  })
  const stripDates = $derived(
    Array.from(
      { length: Math.min(STRIP_WINDOW, MAX_DAY + 1) },
      (_, i) => isoShift(TODAY, stripStart + i),
    ),
  )

  function clampDate(iso) {
    if (!DATE_RE.test(iso)) return TODAY
    return iso < MIN ? MIN : iso > MAX ? MAX : iso
  }

  function pickDate(iso) {
    if (!DATE_RE.test(iso) || iso === value) return
    value = iso
    onchange?.()
  }

  function stepDay(days) {
    pickDate(clampDate(isoShift(sel, days)))
  }
</script>

<div
  class={`flex w-full min-w-0 items-center gap-1 rounded-lg border bg-card px-1.5 py-1 shadow-sm ${klass}`}
  role="group"
  aria-label={label}
>
  <Button
    type="button"
    variant="ghost"
    size="icon"
    class="size-7 shrink-0"
    onclick={() => stepDay(-1)}
    aria-label="Previous day"
    title="Previous day"
  >
    <ChevronLeftIcon />
  </Button>
  <div class="min-w-0 flex-1">
    <div
      class="flex items-center gap-0.5 overflow-x-auto px-1.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    >
      {#each stripDates as iso (iso)}
        {@const active = iso === sel}
        <button
          type="button"
          data-active={active}
          aria-current={active ? 'date' : undefined}
          aria-label={`${label}: ${iso}`}
          onclick={() => pickDate(iso)}
          class={`flex w-[3.6rem] shrink-0 cursor-pointer flex-col items-center rounded-md border px-1 py-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
            active
              ? 'border-transparent bg-primary text-primary-foreground shadow-sm'
              : 'border-transparent hover:bg-muted'
          }`}
        >
          <span
            class={`text-[10px] font-medium uppercase tracking-wide ${active ? 'opacity-80' : 'text-muted-foreground'}`}
          >
            {iso === TODAY ? 'Today' : weekdayShort(iso)}
          </span>
          <span class="font-mono text-[11px] font-semibold tabular-nums">
            {iso.slice(8)}&thinsp;{monthShort(iso)}
          </span>
        </button>
      {/each}
    </div>
  </div>
  <Button
    type="button"
    variant="ghost"
    size="icon"
    class="size-7 shrink-0"
    onclick={() => stepDay(1)}
    aria-label="Next day"
    title="Next day"
  >
    <ChevronRightIcon />
  </Button>
  <Input
    {id}
    type="date"
    bind:value
    min={MIN}
    max={MAX}
    onchange={() => {
      if (DATE_RE.test(String(value ?? ''))) onchange?.()
    }}
    aria-label={`${label} (calendar)`}
    title="Calendar"
    class="h-8 w-32 shrink-0 sm:w-36"
  />
</div>
