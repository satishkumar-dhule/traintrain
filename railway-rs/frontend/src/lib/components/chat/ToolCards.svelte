<script>
  import { navigate } from '$lib/router.svelte.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import MapPin from 'lucide-svelte/icons/map-pin'
  import ClockIcon from 'lucide-svelte/icons/clock'
  import CalendarDays from 'lucide-svelte/icons/calendar-days'
  import ListIcon from 'lucide-svelte/icons/list'
  import SearchIcon from 'lucide-svelte/icons/search'
  import RouteIcon from 'lucide-svelte/icons/route'
  import ClipboardCheck from 'lucide-svelte/icons/clipboard-check'
  import ArrowRight from 'lucide-svelte/icons/arrow-right'
  import { availabilityHref, journeysHref, trainHref } from '$lib/utils.js'

  let { kind = '', data = {} } = $props()

  let d = $derived(data ?? {})

  const DASH = '—'

  const txt = (v) => (v === undefined || v === null || v === '' ? DASH : String(v))
  const num = (v) => {
    const n = Number(v)
    return Number.isFinite(n) ? n : 0
  }
  const list = (v) => (Array.isArray(v) ? v : [])

  /* `@const` is illegal directly inside <div>; hoist to derived state. */
  let trainsBetweenRows = $derived(list(d.trains).slice(0, 8))
  let trainsBetweenMore = $derived(
    Math.max(0, num(d.total_found ?? d.trains?.length) - trainsBetweenRows.length)
  )

  const CHIP_TONES = {
    ok: 'border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
    warn: 'border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400',
    bad: 'border-red-500/30 bg-red-500/10 text-red-600 dark:text-red-400',
  }

  const DOT_CLS = {
    red: 'bg-red-500',
    amber: 'bg-amber-500',
    green: 'bg-emerald-500',
  }
  const BAR_CLS = {
    red: 'bg-red-500',
    amber: 'bg-amber-400',
    green: 'bg-emerald-400',
  }

  const delayTone = (m) => {
    const n = num(m)
    return n >= 20 ? 'red' : n >= 8 ? 'amber' : 'green'
  }

  const LINK_BTN =
    'inline-flex w-fit items-center gap-1 text-xs font-medium text-primary hover:underline max-lg:min-h-11 max-lg:px-1 max-lg:-mx-1'
</script>

{#snippet header(Icon, title, source)}
  <div class="mb-2 flex min-w-0 items-center gap-1.5">
    <Icon class="size-3.5 shrink-0 text-muted-foreground" />
    <span class="truncate font-medium">{title}</span>
    {#if source}
      <span class="ml-auto shrink-0 rounded-full bg-muted px-1.5 py-0.5 text-[10px] leading-none text-muted-foreground">
        {source}
      </span>
    {/if}
  </div>
{/snippet}

{#snippet footer(label, href)}
  <button type="button" class={LINK_BTN} onclick={() => navigate(href)}>
    {label}
    <ArrowRight class="size-3" />
  </button>
{/snippet}

{#snippet ToneChip(c)}
  {@const tone = CHIP_TONES[c?.tone] ?? CHIP_TONES.warn}
  <span class={`inline-flex items-baseline gap-1 rounded-lg border px-2 py-0.5 text-[11px] leading-tight max-lg:text-xs ${tone}`}>
    <span class="font-semibold">{txt(c?.class)}</span>
    <span>{txt(c?.status)}</span>
    {#if c?.fare !== undefined && c?.fare !== null && c?.fare !== ''}
      <span class="opacity-75">₹{c.fare}</span>
    {/if}
    {#if c?.prediction !== undefined && c?.prediction !== null && c?.prediction !== ''}
      <span class="opacity-75">· {c.prediction}%</span>
    {/if}
  </span>
{/snippet}

{#if kind === 'trains_between'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(TrainFront, `Trains · ${txt(d.from)} → ${txt(d.to)}`, d.data_source)}
    <div class="space-y-1">
      {#each trainsBetweenRows as t, ti (t?.number ?? ti)}
        <div class="flex items-center gap-2">
          <Badge variant="outline" class="shrink-0 font-mono">{txt(t?.number)}</Badge>
          <span class="min-w-0 flex-1 truncate">{txt(t?.name)}</span>
          <span class="shrink-0 tabular-nums text-xs max-lg:text-sm">{txt(t?.dep)} → {txt(t?.arr)}</span>
          <span class="hidden w-24 shrink-0 truncate text-right text-[11px] text-muted-foreground sm:block">
            {t?.runs ?? ''}
          </span>
        </div>
      {:else}
        <p class="text-xs text-muted-foreground">No trains found</p>
      {/each}
      {#if trainsBetweenMore > 0}
        <p class="text-[11px] text-muted-foreground">+{trainsBetweenMore} more</p>
      {/if}
    </div>
    {#if d.src_code && d.dst_code}
      <div class="mt-2">
        {@render footer('Open Journeys', journeysHref(d.src_code, d.dst_code))}
      </div>
    {/if}
  </div>
{:else if kind === 'live_status'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(MapPin, `Live · ${txt(d.train_number)}`, d.data_source)}
    <p class="rounded-md bg-muted px-2 py-1 text-xs">{d.position || 'Position unavailable'}</p>
    <div class="mt-2 flex flex-wrap items-center gap-1.5">
      {#if num(d.last_seen_delay_minutes) > 0}
        <span class="inline-flex items-center rounded-full border border-red-500/30 bg-red-500/10 px-2 py-0.5 text-[11px] max-lg:text-xs font-medium text-red-600 dark:text-red-400">
          {num(d.last_seen_delay_minutes)}m late
        </span>
      {:else}
        <span class="inline-flex items-center rounded-full border border-emerald-500/30 bg-emerald-500/10 px-2 py-0.5 text-[11px] max-lg:text-xs font-medium text-emerald-600 dark:text-emerald-400">
          on time
        </span>
      {/if}
      {#if d.platform !== undefined && d.platform !== null && d.platform !== ''}
        <Badge variant="outline">Platform {d.platform}</Badge>
      {/if}
    </div>
    <div class="mt-2 space-y-1">
      {#each list(d.next_stops) as s (s?.code + s?.sch)}
        <div class="flex items-center justify-between gap-2 text-xs max-lg:text-sm">
          <span class="min-w-0 flex-1 truncate">
            {txt(s?.name)}
            <span class="ml-1 font-mono text-[11px] max-lg:text-xs text-muted-foreground">{s?.code ?? DASH}</span>
          </span>
          <span class="shrink-0 tabular-nums text-muted-foreground">{txt(s?.sch)} → {txt(s?.act)}</span>
          <span
            class={`w-10 shrink-0 text-right tabular-nums ${s?.delay_min > 0 ? 'text-red-600 dark:text-red-400' : ''}`}
          >
            {s?.delay_min != null ? `${num(s.delay_min)}m` : DASH}
          </span>
        </div>
      {:else}
        <p class="text-xs text-muted-foreground">No upcoming stops</p>
      {/each}
    </div>
    <div class="mt-2">
      {@render footer('Open live view', `/train/${d.train_number}/status`)}
    </div>
  </div>
{:else if kind === 'average_delay'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(ClockIcon, `Avg delay · ${txt(d.train_no)}`, d.data_source)}
    <div class="space-y-1">
      {#each list(d.stations_worst_first) as s, i (s?.code + i)}
        {@const mins = Math.max(0, num(s?.arr_delay_min))}
        {@const tone = delayTone(mins)}
        <div class="flex items-center gap-2">
          <span class={`size-1.5 shrink-0 rounded-full ${DOT_CLS[tone]}`}></span>
          <span class="w-32 min-w-0 shrink-0 truncate text-xs max-lg:text-sm">
            {txt(s?.name)}
            <span class="ml-1 font-mono text-[11px] max-lg:text-xs text-muted-foreground">{s?.code ?? DASH}</span>
          </span>
          <span class="h-1.5 min-w-1 flex-1 overflow-hidden rounded-full bg-muted">
            <span
              class={`block h-full rounded-full ${BAR_CLS[tone]}`}
              style={`width:${Math.min(100, mins * 2)}%`}
            ></span>
          </span>
          <span class="w-16 shrink-0 text-right tabular-nums text-xs max-lg:text-sm">
            {mins}m
            {#if s?.dep_delay_min != null && s.dep_delay_min !== s.arr_delay_min}
              <span class="text-muted-foreground">/ {Math.max(0, num(s.dep_delay_min))}m</span>
            {/if}
          </span>
        </div>
      {:else}
        <p class="text-xs text-muted-foreground">No delay data</p>
      {/each}
    </div>
    {#if d.train_no}
      <div class="mt-2">
        {@render footer('Open avg delay view', trainHref(d.train_no, 'delay'))}
      </div>
    {/if}
  </div>
{:else if kind === 'seat_availability'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(
      CalendarDays,
      `Availability · ${txt(d.from)} → ${txt(d.to)}`,
      [d.date, d.data_source].filter(Boolean).join(' · ') || undefined
    )}
    {#if d.notice}
      <p class="mb-2 text-xs italic text-muted-foreground">{d.notice}</p>
    {/if}
    <div class="space-y-2">
      {#each list(d.trains) as t, i (t?.number + i)}
        <div class="rounded-lg border p-2">
          <div class="flex items-baseline justify-between gap-2">
            <span class="min-w-0 truncate">
              <span class="mr-1.5 inline-block rounded bg-muted px-1 font-mono text-xs max-lg:text-sm">{txt(t?.number)}</span>
              {txt(t?.name)}
            </span>
            <span class="shrink-0 tabular-nums text-[11px] max-lg:text-xs text-muted-foreground">
              {txt(t?.dep)} → {txt(t?.arr)} · {t?.duration ?? DASH}
            </span>
          </div>
          <div class="mt-1.5 grid grid-cols-1 gap-1 sm:grid-cols-2 min-w-0">
            {#each list(t?.classes) as c, j (c?.class + j)}
              {@render ToneChip(c)}
            {/each}
          </div>
        </div>
      {:else}
        <p class="text-xs text-muted-foreground">No availability data</p>
      {/each}
    </div>
    {#if d.src_code && d.dst_code}
      <div class="mt-2">
        {@render footer('Open availability', availabilityHref(d.src_code, d.dst_code, d.date))}
      </div>
    {/if}
  </div>
{:else if kind === 'station_board'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(ListIcon, `Station board · ${txt(d.station_code)} (${num(d.hours)}h)`, d.data_source)}
    <div class="space-y-1">
      {#each list(d.trains) as r, i (r?.number + i)}
        <div class="flex items-center gap-2 text-xs max-lg:text-sm">
          <span
            class={`size-1.5 shrink-0 rounded-full ${r?.late ? 'bg-red-500' : 'bg-transparent'}`}
            title={r?.late ? 'late' : undefined}
          ></span>
          <Badge variant="outline" class="shrink-0 font-mono">{txt(r?.number)}</Badge>
          <span class="min-w-0 flex-1 truncate">{txt(r?.name)}</span>
          <span class="shrink-0 tabular-nums text-muted-foreground">{txt(r?.sta)} → {txt(r?.eta)}</span>
          <span class="w-7 shrink-0 text-right tabular-nums">{r?.platform ?? DASH}</span>
        </div>
      {:else}
        <p class="text-xs text-muted-foreground">No scheduled arrivals/departures</p>
      {/each}
    </div>
    {#if d.station_code}
      <div class="mt-2">
        {@render footer('Open station view', `/station/${d.station_code}/live`)}
      </div>
    {/if}
  </div>
{:else if kind === 'search_rail'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(SearchIcon, `Corpus search · “${d.query ?? ''}”`, d.data_source)}
    <div class="flex flex-wrap gap-1.5">
      {#each list(d.results) as r, i (r?.code + i)}
        {@const isStation = String(r?.type ?? '').toLowerCase().includes('station')}
        <button
          type="button"
          class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs transition-colors hover:bg-muted max-lg:min-h-11 max-lg:px-3.5"
          onclick={() => navigate(isStation ? `/station/${r.code}` : `/train/${r.code}`)}
        >
          {#if isStation}
            <Building2 class="size-3 text-muted-foreground" />
          {:else}
            <TrainFront class="size-3 text-muted-foreground" />
          {/if}
          {txt(r?.name ?? r?.code)}
          <span class="text-muted-foreground">{r?.code ?? ''}</span>
        </button>
      {:else}
        <p class="text-xs text-muted-foreground">No results</p>
      {/each}
    </div>
  </div>
{:else if kind === 'train_schedule'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(RouteIcon, `Route · ${txt(d.train_number)} ${d.train_name ?? ''}`, d.data_source)}
    {#if list(d.running_days).length}
      <div class="mb-2 flex flex-wrap gap-1">
        {#each d.running_days as day, i (day + i)}
          <Badge variant="secondary" class="text-[10px] max-lg:text-xs">{day}</Badge>
        {/each}
      </div>
    {/if}
    <div class="space-y-1">
      {#each list(d.stops) as s, i (s?.code + i)}
        {#if i < 12}
          <div class="flex items-baseline justify-between gap-2 border-l border-border pl-3 text-xs max-lg:text-sm">
            <span class="min-w-0 flex-1 truncate">
              {i + 1}.
              <span class="ml-0.5 font-mono">{txt(s?.code)}</span>
              <span class="ml-1.5">{txt(s?.name)}</span>
            </span>
            <span class="shrink-0 tabular-nums text-muted-foreground">{txt(s?.arr)} – {txt(s?.dep)}</span>
            <span class="w-9 shrink-0 text-right text-[11px] max-lg:text-xs text-muted-foreground">{s?.halt ?? ''}</span>
          </div>
        {/if}
      {:else}
        <p class="text-xs text-muted-foreground">No stops listed</p>
      {/each}
      {#if list(d.stops).length > 12}
        <p class="pl-3 text-[11px] text-muted-foreground">+{list(d.stops).length - 12} more stops</p>
      {/if}
    </div>
    {#if d.train_number}
      <div class="mt-2">
        {@render footer('Open full schedule', trainHref(d.train_number, 'schedule'))}
      </div>
    {/if}
  </div>
{:else if kind === 'chart_status'}
  <div class="rounded-xl border bg-card p-3 text-sm">
    {@render header(ClipboardCheck, `Chart · ${txt(d.train_number)}`, d.data_source)}
    <dl class="grid grid-cols-[auto_1fr] items-baseline gap-x-4 gap-y-1 text-xs max-lg:text-sm">
      <dt class="text-muted-foreground">Journey date</dt>
      <dd>{txt(d.journey_date)}</dd>
      <dt class="text-muted-foreground">Boarding at</dt>
      <dd>{txt(d.boarding_station)}</dd>
      <dt class="text-muted-foreground">Coaches</dt>
      <dd>{d.coach_count != null ? `${d.coach_count} coaches` : DASH}</dd>
    </dl>
    {#if d.notice}
      <p class="mt-2 text-xs italic text-muted-foreground">{d.notice}</p>
    {/if}
  </div>
{:else}
  <div class="flex items-center gap-2 rounded-xl border border-dashed bg-card px-3 py-2 text-xs text-muted-foreground">
    Unknown result
    {#if kind}<span class="rounded bg-muted px-1 font-mono">{kind}</span>{/if}
  </div>
{/if}
