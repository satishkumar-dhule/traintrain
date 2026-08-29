<script>
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import ChevronLeftIcon from 'lucide-svelte/icons/chevron-left'
  import ChevronRightIcon from 'lucide-svelte/icons/chevron-right'
  import CalendarIcon from 'lucide-svelte/icons/calendar'

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
  const MOBILE_WINDOW = 5

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

  // Memoized formatters for super optimisation
  const weekdayFmt = new Intl.DateTimeFormat('en-IN', { weekday: 'short' })
  const monthFmt = new Intl.DateTimeFormat('en-IN', { month: 'short' })
  const weekdayCache = new Map()
  const monthCache = new Map()
  function weekdayShort(iso) {
    if (weekdayCache.has(iso)) return weekdayCache.get(iso)
    const d = new Date(`${iso}T00:00:00`)
    const v = isNaN(d.getTime()) ? '' : weekdayFmt.format(d)
    weekdayCache.set(iso, v)
    return v
  }
  function monthShort(iso) {
    if (monthCache.has(iso)) return monthCache.get(iso)
    const d = new Date(`${iso}T00:00:00`)
    const v = isNaN(d.getTime()) ? '' : monthFmt.format(d)
    monthCache.set(iso, v)
    return v
  }

  const TODAY = today()
  const MIN = $derived(DATE_RE.test(String(min ?? '')) ? min : TODAY)
  const MAX = $derived(DATE_RE.test(String(max ?? '')) ? max : isoShift(TODAY, MAX_DAY))

  const sel = $derived(DATE_RE.test(String(value ?? '')) ? value : TODAY)

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

  // Mobile: 5-day window fan-out, centered, clamped, no scroll
  const mobileStart = $derived.by(() => {
    const idx = Math.max(0, diffDays(TODAY, sel))
    const maxStart = Math.max(0, MAX_DAY + 1 - MOBILE_WINDOW)
    return Math.max(0, Math.min(idx - Math.floor(MOBILE_WINDOW / 2), maxStart))
  })
  const mobileDates = $derived(
    Array.from({ length: MOBILE_WINDOW }, (_, i) => isoShift(TODAY, mobileStart + i)),
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

  // n2 super delegate: single handler fan-out via data-iso (no per-button closure)
  function handleStripClick(e) {
    const iso = e.currentTarget?.dataset?.iso
    if (iso) pickDate(iso)
  }

  // Calendar super delegate: hidden native input, triggered by icon
  let nativeInput = $state(null)
  function openPicker() {
    if (nativeInput?.showPicker) {
      try { nativeInput.showPicker() } catch {}
    } else {
      nativeInput?.focus?.()
      nativeInput?.click?.()
    }
  }

  /* Auto-center the selected day in the strip (desktop only).
     Respects prefers-reduced-motion. */
  let stripEl = $state(null)
  $effect(() => {
    if (!stripEl) return
    // skip on mobile grid (no scroll)
    if (window.matchMedia('(max-width: 1023.98px)').matches) return
    sel // dependency: re-center whenever the selection changes
    const el = stripEl.querySelector('[data-active="true"]')
    if (!el) return
    const r = el.getBoundingClientRect()
    const pr = stripEl.getBoundingClientRect()
    const delta = r.left + r.width / 2 - (pr.left + pr.width / 2)
    if (Math.abs(delta) < 4) return
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches
    stripEl.scrollBy({ left: delta, behavior: reduce ? 'auto' : 'smooth' })
  })
</script>

<div
  class={`flex w-full min-w-0 items-center gap-0.5 rounded-lg border bg-card px-1 py-1 shadow-sm max-lg:gap-0.5 max-lg:px-1 ${klass}`}
  role="group"
  aria-label={label}
>
  <Button
    type="button"
    variant="ghost"
    size="icon"
    class="size-8 shrink-0 hit-y max-lg:size-8"
    onclick={() => stepDay(-1)}
    aria-label="Previous day"
    title="Previous day"
  >
    <ChevronLeftIcon class="size-4" />
  </Button>

  <!-- Desktop strip: 15 days scrollable -->
  <div class="hidden lg:flex min-w-0 flex-1">
    <div
      bind:this={stripEl}
      class="flex items-center gap-1 overflow-x-auto px-1.5 snap-x snap-mandatory [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    >
      {#each stripDates as iso (iso)}
        {@const active = iso === sel}
        <button
          type="button"
          data-iso={iso}
          data-active={active}
          aria-current={active ? 'date' : undefined}
          aria-label={`${label}: ${iso}`}
          onclick={handleStripClick}
          class={`flex min-h-11 w-[4.25rem] shrink-0 cursor-pointer snap-center flex-col items-center justify-center rounded-md border px-1 py-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
            active
              ? 'border-transparent bg-primary text-primary-foreground shadow-sm'
              : 'border-transparent hover:bg-muted'
          }`}
        >
          <span
            class={`text-xs leading-tight font-medium uppercase tracking-wide ${active ? 'opacity-80' : 'text-muted-foreground'}`}
          >
            {iso === TODAY ? 'Today' : weekdayShort(iso)}
          </span>
          <span class="font-mono text-xs leading-tight font-semibold tabular-nums">
            {iso.slice(8)}&thinsp;{monthShort(iso)}
          </span>
        </button>
      {/each}
    </div>
  </div>

  <!-- Mobile super-optimized: 5-day grid, no scroll, fan-out -->
  <div class="flex lg:hidden min-w-0 flex-1">
    <div class="grid flex-1 grid-cols-5 gap-0.5">
      {#each mobileDates as iso (iso)}
        {@const active = iso === sel}
        <button
          onclick={handleStripClick}
          type="button"
          data-iso={iso}
          data-active={active}
          aria-current={active ? 'date' : undefined}
          aria-label={`${label}: ${iso}`}
          class={`flex min-h-10 flex-col items-center justify-center rounded-md border px-0.5 py-1 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
            active
              ? 'border-transparent bg-primary text-primary-foreground shadow-sm'
              : 'border-transparent bg-muted/40 hover:bg-muted'
          }`}
        >
          <span
            class={`text-xs leading-none font-medium uppercase tracking-wide ${active ? 'opacity-90' : 'text-muted-foreground'}`}
          >
            {iso === TODAY ? 'Today' : weekdayShort(iso)}
          </span>
          <span class="font-mono text-[11px] leading-none font-semibold tabular-nums">
            {iso.slice(8)} {monthShort(iso).slice(0,3)}
          </span>
        </button>
      {/each}
    </div>
  </div>

  <Button
    type="button"
    variant="ghost"
    size="icon"
    class="size-8 shrink-0 hit-y max-lg:size-8"
    onclick={() => stepDay(1)}
    aria-label="Next day"
    title="Next day"
  >
    <ChevronRightIcon class="size-4" />
  </Button>

  <!-- Desktop native picker -->
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
    class="hidden lg:flex h-10 w-36 shrink-0"
  />

  <!-- Mobile calendar super delegate: icon only -->
  <div class="flex lg:hidden shrink-0 items-center">
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="size-8 shrink-0"
      onclick={openPicker}
      aria-label="Open calendar"
      title="Open calendar"
    >
      <CalendarIcon class="size-4" />
    </Button>
    <!-- Hidden native input delegated -->
    <input
      bind:this={nativeInput}
      id={`${id}-mobile`}
      type="date"
      value={value}
      min={MIN}
      max={MAX}
      onchange={(e) => {
        const v = e.currentTarget.value
        if (DATE_RE.test(v)) {
          value = v
          onchange?.()
        }
      }}
      aria-hidden="true"
      tabindex="-1"
      class="sr-only h-0 w-0 overflow-hidden border-0 p-0"
    />
  </div>
</div>
