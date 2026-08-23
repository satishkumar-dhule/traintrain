<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import ArrowRight from 'lucide-svelte/icons/arrow-right'
  import Ticket from 'lucide-svelte/icons/ticket'
import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'
import RecentSearches from '$lib/components/RecentSearches.svelte'
import {
  TrainNumberBadge,
  StationCodeBadge,
  PnrStatusBadge,
  DataSourceBadge,
  StatusBadge,
  TrainDelayBadge
} from '$lib/components/badges/index.js'
import { stationHref, trainHref, availabilityHref, todayISO, DATE_RE } from '$lib/utils.js'
import TrainFrontIcon from 'lucide-svelte/icons/train-front'
import CalendarDaysIcon from 'lucide-svelte/icons/calendar-days'
import Building2Icon from 'lucide-svelte/icons/building-2'

  const RECENT_KEY = 'rc-pnr-recent'

  let { pnr = '' } = $props()

  let query = $state('')
  let committed = $state('')
  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)
  let captcha = $state(null)
  let captchaPnr = null
  let captchaText = $state('')
  let auto = $state(false)

  function loadRecent() {
    try {
      const raw = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]')
      if (!Array.isArray(raw)) return []
      return raw.filter((v) => typeof v === 'string' && /^\d{10}$/.test(v)).slice(0, 5)
    } catch {
      return []
    }
  }

  let recent = $state(loadRecent())

  function remember(p) {
    const next = [p, ...recent.filter((r) => r !== p)].slice(0, 5)
    recent = next
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(next))
    } catch {}
  }

  function clearRecent() {
    recent = []
    try {
      localStorage.removeItem(RECENT_KEY)
    } catch {}
  }

  function useRecent(p) {
    if (p === committed) {
      query = p
      lookup()
    } else {
      navigate(`/pnr/${p}`)
    }
  }

  function asText(v) {
    return String(v ?? '').trim()
  }

  const valid = $derived(/^\d{10}$/.test(asText(query)))
  const busy = $derived(phase === 'loading' || phase === 'refreshing')

  async function lookup(target) {
    const t = asText(target ?? query)
    if (!/^\d{10}$/.test(t)) return
    committed = t
    const want = `/pnr/${t}`
    if (route.path !== want) navigate(want)
    phase = data ? 'refreshing' : 'loading'
    errorMsg = null
    let path = `/rail-api/pnr?pnr=${t}`
    if (captcha?.session_id && captchaPnr === t) {
      path += `&captcha_session=${encodeURIComponent(captcha.session_id)}&captcha_text=${encodeURIComponent(captchaText.trim())}`
    }
    const res = await api(path)
    if (`${committed}` !== `${t}`) return
    if (res.ok) {
      data = res.data
      phase = 'ok'
      captcha = null
      captchaPnr = null
      captchaText = ''
      remember(t)
    } else if (res.status === 428 && res.body?.error === 'captcha_required') {
      captcha = res.body
      captchaPnr = t
      captchaText = ''
      phase = 'captcha'
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  $effect(() => {
    const n = asText(pnr)
    if (/^\d{10}$/.test(n) && n !== committed) {
      query = n
      lookup(n)
    }
  })

  $effect(() => {
    if (!auto || phase === 'captcha') return
    const t = committed
    const timer = setInterval(() => {
      if (t) lookup(t)
    }, 30000)
    return () => clearInterval(timer)
  })

  function fmt(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }

  function fmtUpdated(v) {
    const d = new Date(v)
    if (Number.isNaN(d.getTime())) return v
    const when = d.toLocaleString('en-IN', {
      timeZone: 'Asia/Kolkata',
      day: '2-digit',
      month: 'short',
      hour: '2-digit',
      minute: '2-digit',
      hour12: false
    })
    return `${when} IST`
  }

  const passengers = $derived(Array.isArray(data?.passengers) ? data.passengers : [])
  const notice = $derived(asText(data?.notice))
  const lastUpdated = $derived(asText(data?.last_updated))

  /* Context for the jump actions: boarding/destination codes and the
     journey date (ISO from the backend; fall back to today). */
  const pnrFrom = $derived(asText(data?.from?.code).toUpperCase())
  const pnrTo = $derived(asText(data?.to?.code).toUpperCase())
  const pnrDate = $derived(
    DATE_RE.test(asText(data?.journey_date)) ? asText(data.journey_date) : todayISO()
  )

  const cols = [
    {
      key: 'n',
      label: '#',
      class: 'w-12',
      sortable: false,
      value: (p) => String(passengers.indexOf(p) + 1),
    },
    { key: 'booking_status', label: 'Booking status', value: (p) => fmt(p.booking_status) },
    { key: 'coach', label: 'Coach', cellClass: 'font-mono', value: (p) => fmt(p.coach) },
    { key: 'berth', label: 'Berth', cellClass: 'font-mono', value: (p) => fmt(p.berth) },
    { key: 'current_status', label: 'Current', cellClass: 'font-mono', value: (p) => fmt(p.current_status) },
  ]
</script>

{#snippet numCell(p)}
  <span class="text-muted-foreground">{passengers.indexOf(p) + 1}</span>
{/snippet}

{#snippet bookingCell(p)}
  <PnrStatusBadge status={p.booking_status} />
{/snippet}

<section class="grid gap-6" class:idle-center={phase === 'idle'}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">PNR status</h1>
    <p class="max-lg:hidden text-sm text-muted-foreground">10-digit passenger name record. Upstream may require a captcha.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div class="grid min-w-48 flex-1 gap-2">
        <Label for="pnr-no">PNR number</Label>
        <Input
          id="pnr-no"
          bind:value={query}
          placeholder="e.g. 1234567890"
          inputmode="numeric"
          maxlength={10}
          onkeydown={(e) => e.key === 'Enter' && !e.defaultPrevented && lookup()}
        />
        {#if query && !valid}
          <p class="text-xs text-muted-foreground">PNR must be exactly 10 digits.</p>
        {/if}
      </div>
      <Button type="button" onclick={() => lookup()} disabled={!valid || busy}>
        {phase === 'refreshing' ? 'Refreshing…' : 'Check status'}
      </Button>
      <label class="mb-0.5 flex min-h-11 cursor-pointer items-center gap-2 py-2 text-sm text-muted-foreground">
        <input type="checkbox" bind:checked={auto} class="size-5 accent-[var(--primary)]" />
        Auto 30s
      </label>
    </Card.Content>
  </Card.Root>

  {#if phase === 'idle' && recent.length > 0}
    <RecentSearches
      title="Recent lookups"
      items={recent.map((r) => ({ id: r, label: r }))}
      onpick={(item) => useRecent(item.id)}
      onclear={clearRecent}
    />
  {/if}

  {#if phase === 'captcha' && captcha}
    <Card.Root>
      <Card.Header>
        <Card.Title>Captcha required</Card.Title>
        <Card.Description>{captcha.message ?? 'Enter the characters shown to continue.'}</Card.Description>
      </Card.Header>
      <Card.Content class="grid gap-4">
        {#if captcha.image}
          <img
            src={captcha.image.startsWith('data:') ? captcha.image : `data:image/jpeg;base64,${captcha.image}`}
            alt="PNR captcha challenge"
            class="w-full max-w-[240px] rounded-md border"
          />
        {:else}
          <Alert.Root variant="destructive">
            <Alert.Title>No captcha image supplied</Alert.Title>
            <Alert.Description>Upstream did not return an image. Try again.</Alert.Description>
          </Alert.Root>
        {/if}
        <div class="flex flex-wrap items-end gap-3">
          <div class="grid flex-1 gap-2 sm:max-w-56">
            <Label for="cap-text">Captcha text</Label>
            <Input
              id="cap-text"
              bind:value={captchaText}
              placeholder="5 characters"
              onkeydown={(e) => e.key === 'Enter' && !e.defaultPrevented && lookup()}
            />
          </div>
          <Button type="button" onclick={() => lookup()} disabled={!captchaText.trim() || busy}>
            Submit
          </Button>
        </div>
      </Card.Content>
    </Card.Root>
  {:else if phase === 'loading'}
    <div class="grid gap-2" aria-busy="true">
      {#each [0, 1, 2] as i (i)}
        <Skeleton class="h-12 w-full" />
      {/each}
    </div>
  {:else if phase === 'error'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not load PNR</Alert.Title>
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {:else if data}
    <Card.Root>
      <Card.Header class="gap-3 space-y-0">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <Card.Title class="flex flex-wrap items-center gap-2">
            <TrainNumberBadge number={data.train_number} name={data.train_name} />
            <span>{data.train_name ?? ''}</span>
            <TrainDelayBadge number={data.train_number} name={data.train_name} />
          </Card.Title>
          <DataSourceBadge source={data.data_source} freshness={data.freshness} />
        </div>
        <div class="flex flex-wrap items-center gap-x-6 gap-y-3 rounded-lg border bg-muted/40 px-5 py-3.5">
          <div class="grid min-w-28 gap-0.5">
            <StationCodeBadge code={data.from?.code} name={data.from?.name} class="text-sm" />
            <span class="truncate text-xs text-muted-foreground">{fmt(data.from?.name)}</span>
            <span class="text-xs text-muted-foreground">
              {fmt(data.from?.time)}{#if data.from?.day}&nbsp;· day {data.from.day}{/if}
            </span>
          </div>
          <ArrowRight class="size-4 shrink-0 text-muted-foreground" />
          <div class="grid min-w-28 gap-0.5">
            <StationCodeBadge code={data.to?.code} name={data.to?.name} class="text-sm" />
            <span class="truncate text-xs text-muted-foreground">{fmt(data.to?.name)}</span>
            <span class="text-xs text-muted-foreground">
              {fmt(data.to?.time)}{#if data.to?.day}&nbsp;· day {data.to.day}{/if}
            </span>
          </div>
          <StatusBadge tone="info" class="ml-auto shrink-0">journey {fmt(data.journey_date)}</StatusBadge>
        </div>
      </Card.Header>
      <Card.Content>
        <DataTable
          columns={cols}
          rows={passengers}
          primary="booking_status"
          titleText={(p) => `Passenger ${p.n ?? ''}`.trim()}
          rowKey={(p, i) => i}
          cells={{ n: numCell, booking_status: bookingCell }}
          empty="No passengers returned."
        />
        {#if data.train_number || (pnrFrom && pnrTo)}
          <div class="mt-3 flex flex-wrap items-center gap-2 border-t pt-3">
            <span class="text-xs text-muted-foreground">Jump to</span>
            {#if data.train_number}
              <Button
                type="button"
                variant="outline"
                size="xs"
                onclick={() => navigate(trainHref(data.train_number, 'status'))}
              >
                <TrainFrontIcon class="size-3" />
                Track live
              </Button>
            {/if}
            {#if pnrFrom && pnrTo}
              <Button
                type="button"
                variant="outline"
                size="xs"
                onclick={() => navigate(availabilityHref(pnrFrom, pnrTo, pnrDate))}
              >
                <CalendarDaysIcon class="size-3" />
                Availability · {pnrFrom}→{pnrTo}
              </Button>
            {/if}
            {#if pnrFrom}
              <Button
                type="button"
                variant="outline"
                size="xs"
                onclick={() => navigate(stationHref(pnrFrom))}
              >
                <Building2Icon class="size-3" />
                {pnrFrom} board
              </Button>
            {/if}
          </div>
        {/if}
      </Card.Content>
    </Card.Root>
    {#if notice || lastUpdated}
      <div class="flex flex-wrap items-center gap-x-3 gap-y-1 px-1 text-xs text-muted-foreground">
        {#if notice}
          <span class="max-w-full truncate" title={notice}>{notice}</span>
        {/if}
        {#if lastUpdated}
          <span>updated {fmtUpdated(lastUpdated)}</span>
        {/if}
      </div>
    {/if}
  {:else}
    <EmptyState
      icon={Ticket}
      title="No PNR checked yet"
      hint="Enter a 10-digit PNR above or pick a recent lookup to see status."
    />
  {/if}
</section>
