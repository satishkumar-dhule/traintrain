<script>
  import { api } from '$lib/api.js'
  import { navigate, route } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import ArrowRight from 'lucide-svelte/icons/arrow-right'

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

  function fmt(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }

  function statusKind(s) {
    const t = String(s ?? '')
      .trim()
      .toUpperCase()
    if (!t || t === '-' || t === '--') return null
    const head = t.split(/[\s/-]/)[0]
    if (head === 'CNF') return 'cnf'
    if (head === 'RAC') return 'rac'
    if (head === 'WL' || head === 'WAITLIST' || head === 'WAITLISTED') return 'wl'
    return null
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">PNR status</h1>
    <p class="text-sm text-muted-foreground">10-digit passenger name record. Upstream may require a captcha.</p>
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
    </Card.Content>
  </Card.Root>

  {#if recent.length > 0}
    <div class="flex flex-wrap items-center gap-2">
      <span class="flex items-center gap-1.5 text-xs text-muted-foreground">Recent lookups</span>
      {#each recent as r (r)}
        <a
          href={`/pnr/${r}`}
          class="inline-flex h-6 items-center rounded-full border px-2.5 font-mono text-xs transition-colors hover:bg-muted hover:text-foreground"
          onclick={(e) => {
            e.preventDefault()
            useRecent(r)
          }}
        >
          {r}
        </a>
      {/each}
      <Button variant="ghost" size="xs" class="h-6 text-xs text-muted-foreground" onclick={clearRecent}>
        Clear all
      </Button>
    </div>
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
          <Card.Title>
            <span class="font-mono">{data.train_number ?? '—'}</span> · {data.train_name ?? ''}
          </Card.Title>
          <div class="flex shrink-0 flex-wrap items-center gap-1.5">
            {#if data.freshness}<Badge variant="secondary">{data.freshness}</Badge>{/if}
            {#if data.data_source}<Badge variant="outline">{data.data_source}</Badge>{/if}
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-x-5 gap-y-3 rounded-lg border bg-muted/40 px-4 py-3">
          <div class="grid min-w-28 gap-0.5">
            <span class="font-mono text-sm font-semibold">{fmt(data.from?.code)}</span>
            <span class="truncate text-xs text-muted-foreground">{fmt(data.from?.name)}</span>
            <span class="text-xs text-muted-foreground">
              {fmt(data.from?.time)}{#if data.from?.day}&nbsp;· day {data.from.day}{/if}
            </span>
          </div>
          <ArrowRight class="size-4 shrink-0 text-muted-foreground" />
          <div class="grid min-w-28 gap-0.5">
            <span class="font-mono text-sm font-semibold">{fmt(data.to?.code)}</span>
            <span class="truncate text-xs text-muted-foreground">{fmt(data.to?.name)}</span>
            <span class="text-xs text-muted-foreground">
              {fmt(data.to?.time)}{#if data.to?.day}&nbsp;· day {data.to.day}{/if}
            </span>
          </div>
          <Badge variant="secondary" class="ml-auto shrink-0">journey {fmt(data.journey_date)}</Badge>
        </div>
      </Card.Header>
      <Card.Content>
        <Table.Root>
          <Table.Header>
            <Table.Row>
              <Table.Head>#</Table.Head>
              <Table.Head>Booking status</Table.Head>
              <Table.Head>Coach</Table.Head>
              <Table.Head>Berth</Table.Head>
              <Table.Head>Current</Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each data.passengers ?? [] as p, i (i)}
              {@const kind = statusKind(p.booking_status)}
              <Table.Row>
                <Table.Cell class="text-muted-foreground">{i + 1}</Table.Cell>
                <Table.Cell class="font-medium">
                  {#if kind === 'cnf'}
                    <Badge
                      variant="default"
                      class="border-emerald-600/40 text-emerald-600 dark:text-emerald-400"
                    >
                      {fmt(p.booking_status)}
                    </Badge>
                  {:else if kind === 'rac'}
                    <Badge variant="secondary">{fmt(p.booking_status)}</Badge>
                  {:else if kind === 'wl'}
                    <Badge variant="outline">{fmt(p.booking_status)}</Badge>
                  {:else}
                    <span class="text-muted-foreground">{fmt(p.booking_status)}</span>
                  {/if}
                </Table.Cell>
                <Table.Cell class="font-mono">{fmt(p.coach)}</Table.Cell>
                <Table.Cell class="font-mono">{fmt(p.berth)}</Table.Cell>
                <Table.Cell class="font-mono">{fmt(p.current_status)}</Table.Cell>
              </Table.Row>
            {:else}
              <Table.Row>
                <Table.Cell colspan={5} class="text-muted-foreground">No passengers returned.</Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </Card.Content>
    </Card.Root>
  {:else}
    <p class="text-sm text-muted-foreground">Enter a PNR above or pick a recent lookup to see status.</p>
  {/if}
</section>
