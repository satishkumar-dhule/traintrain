<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import { StatusBadge } from '$lib/components/badges/index.js'
  import SignalDot from '$lib/components/SignalDot.svelte'
  import TrackRule from '$lib/components/TrackRule.svelte'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'
  import { fmtUptime, fmtCompact as fmtInt } from '$lib/format.js'

  import TrainFront from 'lucide-svelte/icons/train-front'
  import Clock from 'lucide-svelte/icons/clock'
  import Activity from 'lucide-svelte/icons/activity'
  import Zap from 'lucide-svelte/icons/zap'
  import Gauge from 'lucide-svelte/icons/gauge'
  import Database from 'lucide-svelte/icons/database'
  import Radio from 'lucide-svelte/icons/radio'
  import MegaphoneOff from 'lucide-svelte/icons/megaphone-off'
  import EyeOff from 'lucide-svelte/icons/eye-off'
  import Lock from 'lucide-svelte/icons/lock'
  import ShieldCheck from 'lucide-svelte/icons/shield-check'
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert'
  import Heart from 'lucide-svelte/icons/heart'
  import House from 'lucide-svelte/icons/house'
  import Building2 from 'lucide-svelte/icons/building-2'
  import RouteIcon from 'lucide-svelte/icons/route'
  import CalendarDays from 'lucide-svelte/icons/calendar-days'
  import Ticket from 'lucide-svelte/icons/ticket'
  import Package from 'lucide-svelte/icons/package'
  import Sparkles from 'lucide-svelte/icons/sparkles'

  let obs = $state({ phase: 'loading', data: null })

  async function load() {
    obs = { phase: obs.data ? 'refreshing' : 'loading', data: obs.data }
    const res = await api('/rail-api/observability')
    obs = res.ok ? { phase: 'ok', data: res.data } : { phase: 'error', data: null }
  }

  $effect(() => {
    untrack(() => load())
  })

  function num(v) {
    const n = Number(v)
    return Number.isFinite(n) ? n : null
  }

  function hitRate(d) {
    if (!d?.cache) return null
    const hits = num(d.cache.hits)
    const misses = num(d.cache.misses)
    if (hits === null || misses === null) return null
    const lookups = hits + misses
    return lookups > 0 ? (hits / lookups) * 100 : null
  }

  const SRC_SHORT = {
    railyatri: 'RY',
    etrain: 'ET',
    ntes: 'NTES',
    irctc: 'IRCTC',
    paytm: 'PTM',
    askdisha_api: 'AD·API',
    askdisha_cdn: 'AD·CDN',
  }

  function srcShort(name) {
    return SRC_SHORT[String(name ?? '').toLowerCase()] ?? String(name ?? '?').slice(0, 4)
  }

  function sparkline(values, w = 128, h = 40, pad = 3) {
    const nums = (Array.isArray(values) ? values : [])
      .map(Number)
      .filter((v) => Number.isFinite(v))
      .slice(-60)
    if (nums.length < 2) return null
    const min = Math.min(...nums)
    const max = Math.max(...nums)
    const span = max - min || 1
    const dx = (w - pad * 2) / (nums.length - 1)
    const pts = nums.map(
      (v, i) =>
        `${(pad + i * dx).toFixed(1)},${(h - pad - ((v - min) / span) * (h - pad * 2)).toFixed(1)}`
    )
    return {
      line: pts.join(' '),
      area: `M${pad},${h} L${pts.join(' L')} L${w - pad},${h} Z`,
      last: pts[pts.length - 1].split(',').map(Number),
    }
  }

  const d = $derived(obs.phase === 'ok' ? obs.data : null)
  const rpsSpark = $derived(sparkline(d?.series?.rps))
  const latencySpark = $derived(sparkline(d?.series?.latency_ms))
  const rate = $derived(hitRate(d))
  const rateArc = $derived(rate === null ? null : rate / 100)

  const R = 26
  const CIRC = 2 * Math.PI * R

  const FEATURES = [
    { href: '/', label: 'Live Status', icon: TrainFront },
    { href: '/station', label: 'Station Board', icon: Building2 },
    { href: '/journeys', label: 'Journeys', icon: RouteIcon },
    { href: '/availability', label: 'Availability', icon: CalendarDays },
    { href: '/pnr', label: 'PNR Status', icon: Ticket },
    { href: '/extras', label: 'Heritage & Parcel', icon: Package },
    { href: '/assistant', label: 'Ask Train Bro', icon: Sparkles },
    { href: '/system', label: 'System', icon: Activity },
  ]

  const PRINCIPLES = [
    { label: 'No ads', icon: MegaphoneOff },
    { label: 'No tracking', icon: EyeOff },
    { label: 'No accounts', icon: Lock },
    { label: 'Raw honest data', icon: ShieldCheck },
  ]

  function go(e, href) {
    e.preventDefault()
    navigate(href)
  }

  // reusable compact card pattern - single source for gap/py, title/value styling
  const ABOUT_CARD_SIMPLE = 'gap-1 py-4'
  const ABOUT_CARD_SPARK = 'gap-0 py-4'
  const ABOUT_CARD_GAUGE = 'gap-2 py-4'
  const ABOUT_TITLE_CLS = 'flex items-center gap-1.5 px-4 text-[11px] font-medium uppercase tracking-wide text-muted-foreground'
  const ABOUT_VALUE_CLS = 'px-4 data-num text-2xl font-semibold'
</script>

<section class="grid gap-6">
  <header
    class="relative overflow-hidden rounded-xl border bg-gradient-to-br from-primary/12 via-primary/5 to-transparent p-5 sm:p-7"
  >
    <svg
      class="pointer-events-none absolute inset-x-0 bottom-0 h-14 w-full opacity-[0.13]"
      viewBox="0 0 600 24"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <line x1="0" y1="6" x2="600" y2="6" stroke="currentColor" stroke-width="2" />
      <line x1="0" y1="18" x2="600" y2="18" stroke="currentColor" stroke-width="2" />
      {#each Array.from({ length: 31 }, (_, i) => i) as i}
        <rect x={i * 20} y="0" width="8" height="24" fill="currentColor" />
      {/each}
    </svg>

    <div class="relative flex items-center gap-4 sm:gap-5 min-w-0">
      <div
        class="grid size-16 shrink-0 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-lg sm:size-20"
      >
        <TrainFront class="size-9 sm:size-11" />
      </div>
      <div class="grid gap-2 min-w-0">
        <h1 class="signage text-2xl sm:text-3xl break-words">Train Bro</h1>
        <div class="flex flex-wrap items-center gap-1.5">
          <StatusBadge tone="neutral">Free forever</StatusBadge>
          <StatusBadge tone="neutral">No login</StatusBadge>
          <StatusBadge tone="neutral">Indian Railways data</StatusBadge>
        </div>
      </div>
    </div>
  </header>

  <TrackRule />

  <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-6">
    {#if obs.data === null}
      {#each Array.from({ length: 6 }, (_, i) => i) as i (i)}
        <Skeleton class="h-[104px] rounded-xl" />
      {/each}
    {:else}
      <Card.Root class={ABOUT_CARD_SIMPLE}>
        <Card.Title class={ABOUT_TITLE_CLS}>
          <Clock class="size-3.5" /> Uptime
        </Card.Title>
        <Card.Description class={ABOUT_VALUE_CLS}>
          {fmtUptime(d?.uptime_secs)}
        </Card.Description>
      </Card.Root>

      <Card.Root class={ABOUT_CARD_SIMPLE}>
        <Card.Title class={ABOUT_TITLE_CLS}>
          <Activity class="size-3.5" /> Requests
        </Card.Title>
        <Card.Description class={ABOUT_VALUE_CLS}>
          {fmtInt(d?.requests_total)}
        </Card.Description>
      </Card.Root>

      <Card.Root class={ABOUT_CARD_SPARK}>
        <Card.Title class={ABOUT_TITLE_CLS}>
          <Zap class="size-3.5" /> Req / sec
        </Card.Title>
        <Card.Description class={ABOUT_VALUE_CLS}>
          {fmtInt(d?.req_per_sec)}
        </Card.Description>
        {#if rpsSpark}
          <svg viewBox="0 0 128 40" class="-mt-1 h-10 w-full" preserveAspectRatio="none" aria-hidden="true">
            <path d={rpsSpark.area} fill="currentColor" class="text-primary/15" />
            <polyline
              points={rpsSpark.line}
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="text-primary"
            />
            <circle cx={rpsSpark.last[0]} cy={rpsSpark.last[1]} r="2.5" fill="currentColor" class="text-primary" />
          </svg>
        {/if}
      </Card.Root>

      <Card.Root class={ABOUT_CARD_SPARK}>
        <Card.Title class={ABOUT_TITLE_CLS}>
          <Gauge class="size-3.5" /> Avg latency
        </Card.Title>
        <Card.Description class={ABOUT_VALUE_CLS}>
          {fmtInt(d?.latency_ms)}<span class="text-sm font-normal text-muted-foreground">ms</span>
        </Card.Description>
        {#if latencySpark}
          <svg viewBox="0 0 128 40" class="-mt-1 h-10 w-full" preserveAspectRatio="none" aria-hidden="true">
            <path d={latencySpark.area} fill="currentColor" class="text-chart-2/15" />
            <polyline
              points={latencySpark.line}
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="text-chart-2"
            />
            <circle cx={latencySpark.last[0]} cy={latencySpark.last[1]} r="2.5" fill="currentColor" class="text-chart-2" />
          </svg>
        {/if}
      </Card.Root>

      <Card.Root class={ABOUT_CARD_GAUGE}>
        <Card.Title class={ABOUT_TITLE_CLS}>
          <Database class="size-3.5" /> Cache hit
        </Card.Title>
        <div class="flex items-center justify-between gap-2 px-4">
          <span class="data-num text-2xl font-semibold">
            {rate === null ? '—' : `${Math.round(rate)}%`}
          </span>
          <svg viewBox="0 0 64 64" class="size-12 shrink-0 -rotate-90" aria-hidden="true">
            <circle cx="32" cy="32" r={R} fill="none" stroke="currentColor" stroke-width="7" class="text-muted" />
            {#if rateArc !== null}
              <circle
                cx="32"
                cy="32"
                r={R}
                fill="none"
                stroke="currentColor"
                stroke-width="7"
                stroke-linecap="round"
                stroke-dasharray={`${(CIRC * rateArc).toFixed(1)} ${CIRC.toFixed(1)}`}
                class="text-primary transition-all duration-700"
              />
            {/if}
          </svg>
        </div>
      </Card.Root>

      <Card.Root class={ABOUT_CARD_GAUGE}>
        <Card.Title class={ABOUT_TITLE_CLS}>
          <Radio class="size-3.5" /> Sources live <SignalDot tone="go" pulse />
        </Card.Title>
        <Card.Description class={ABOUT_VALUE_CLS}>
          {fmtInt(d?.origins?.length)}
        </Card.Description>
        <div class="flex flex-wrap gap-1 px-4">
          {#each d?.origins ?? [] as o (o.name)}
            <span
              class="data-num rounded border bg-muted/50 px-1.5 py-0.5 text-xs text-muted-foreground"
            >
              {srcShort(o.name)}
            </span>
          {/each}
        </div>
      </Card.Root>
    {/if}
  </div>

  <div class="grid gap-3">
    <h2 class="text-[11px] font-medium uppercase tracking-[0.16em] text-muted-foreground">
      Everything, one tap away
    </h2>
    <div class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-5">
      {#each FEATURES as f (f.href)}
        {@const Icon = f.icon}
        <button
          type="button"
          onclick={(e) => go(e, f.href)}
          class="group grid place-items-center gap-3 rounded-xl border bg-card p-5 text-card-foreground shadow-sm transition-all hover:-translate-y-0.5 hover:border-primary/50 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <span
            class="grid size-11 place-items-center rounded-lg bg-muted text-muted-foreground transition-colors group-hover:bg-primary group-hover:text-primary-foreground"
          >
            <Icon class="size-5" />
          </span>
          <span class="text-sm font-medium leading-none">{f.label}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
    {#each PRINCIPLES as p (p.label)}
      {@const Icon = p.icon}
      <div class="flex items-center gap-3 rounded-xl border bg-card p-4 shadow-sm">
        <span class="grid size-10 shrink-0 place-items-center rounded-full bg-signal-go/10 text-signal-go-ink dark:bg-signal-go/15">
          <Icon class="size-5" />
        </span>
        <span class="text-sm font-semibold">{p.label}</span>
      </div>
    {/each}
  </div>

  <footer class="flex flex-wrap items-center justify-center gap-2 pb-2 text-center">
    <StatusBadge tone="neutral" class="gap-1.5 px-3 py-1.5 font-normal normal-case">
      <Database class="size-3.5 shrink-0" />
      Data © Indian Railways · NTES · IRCTC &amp; partners
    </StatusBadge>
    <StatusBadge tone="neutral" class="gap-1.5 px-3 py-1.5 font-normal normal-case">
      <TriangleAlert class="size-3.5 shrink-0" />
      Verify critical info with official sources
    </StatusBadge>
    <StatusBadge tone="neutral" class="gap-1.5 px-3 py-1.5 font-normal normal-case">
      <Heart class="size-3.5 shrink-0 text-signal-stop" />
      Made for the Indian railway community
    </StatusBadge>
  </footer>
  <BottomSpacer />
</section>
