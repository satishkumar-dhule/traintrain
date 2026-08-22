<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import ActivityIcon from 'lucide-svelte/icons/activity'
  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'
  import ChartColumnIcon from 'lucide-svelte/icons/chart-no-axes-column'

  let { number = '', view = '' } = $props()

  let query = $state('')
  let auto = $state(false)
  let activeTab = $state('status')
  let committed = $state('')

  let phase = $state('idle')
  let errorMsg = $state(null)
  let data = $state(null)

  let schPhase = $state('idle')
  let schErr = $state(null)
  let schData = $state(null)
  let schFor = null

  let avgPhase = $state('idle')
  let avgErr = $state(null)
  let avgData = $state(null)
  let avgFor = null

  const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']
  const VIEW_TO_TAB = { status: 'status', schedule: 'schedule', delay: 'avg' }

  function asText(v) {
    return String(v ?? '').trim()
  }

  async function loadStatus(t) {
    const fresh = `${data?.train_number}` !== `${t}`
    phase = fresh ? 'loading' : 'refreshing'
    errorMsg = null
    const res = await api(`/rail-api/live-status?train=${encodeURIComponent(t)}`)
    if (`${committed}` !== `${t}`) return
    if (res.ok) {
      data = res.data
      phase = 'ok'
      const want = `/train/${t}/${VIEW_TO_TAB[activeTab] ?? 'status'}`
      if (window.location.pathname !== want) navigate(want)
    } else {
      phase = 'error'
      errorMsg = res.error || `HTTP ${res.status}`
    }
  }

  async function loadSchedule(t) {
    schFor = t
    schPhase = 'loading'
    schErr = null
    const res = await api(`/rail-api/schedule?train=${encodeURIComponent(t)}`)
    if (`${schFor}` !== `${t}`) return
    if (res.ok) {
      schData = res.data
      schPhase = 'ok'
    } else {
      schErr = res.error || `HTTP ${res.status}`
      schPhase = 'error'
    }
  }

  async function loadAvg(t) {
    avgFor = t
    avgPhase = 'loading'
    avgErr = null
    const res = await api(`/rail-api/ntes/average-delay?train=${encodeURIComponent(t)}`)
    if (`${avgFor}` !== `${t}`) return
    if (res.ok) {
      avgData = res.data
      avgPhase = 'ok'
    } else {
      avgErr = res.error || `HTTP ${res.status}`
      avgPhase = 'error'
    }
  }

  function track(n) {
    const t = String(n ?? query ?? '').trim()
    if (!t) return
    committed = t
    loadStatus(t)
  }

  function onTabChange(v) {
    if (!committed) return
    navigate(`/train/${committed}/${VIEW_TO_TAB[v] ?? 'status'}`)
  }

  $effect(() => {
    const n = asText(number)
    const tab = VIEW_TO_TAB[asText(view)] ?? null
    if (tab && tab !== activeTab) activeTab = tab
    if (n && `${n}` !== `${committed}`) {
      query = n
      committed = n
      loadStatus(n)
    }
  })

  $effect(() => {
    const t = committed
    const tab = activeTab
    if (!t) return
    if (tab === 'schedule') {
      if (`${schFor}` !== `${t}`) loadSchedule(t)
    } else if (tab === 'avg') {
      if (`${avgFor}` !== `${t}`) loadAvg(t)
    }
  })

  $effect(() => {
    if (!auto) return
    const timer = setInterval(() => {
      if (committed) loadStatus(committed)
    }, 30000)
    return () => clearInterval(timer)
  })

  function delayBadge(d) {
    if (d == null) return { variant: 'outline', text: '—' }
    if (d > 0) return { variant: 'destructive', text: `${d}m late` }
    return { variant: 'secondary', text: 'on time' }
  }

  function fmtTime(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }

  function maxDelay(stations) {
    let m = 0
    for (const s of stations ?? []) {
      for (const v of [s.arrival_delay, s.departure_delay]) {
        const n = Number(v)
        if (Number.isFinite(n) && n > m) m = n
      }
    }
    return m
  }

  function barPct(v, m) {
    const n = Number(v)
    if (!Number.isFinite(n)) return null
    if (m <= 0) return 0
    return Math.min(100, (n / m) * 100)
  }

  function delayLabel(v) {
    const n = Number(v)
    if (!Number.isFinite(n)) return '—'
    return `${Math.round(n)}m`
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Live train status</h1>
    <p class="text-sm text-muted-foreground">Spot any train by number or name. Data refreshes honestly from the live API.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div
        class="grid min-w-48 flex-1 gap-2"
        onkeydown={(e) => {
          if (e.key === 'Enter' && !e.defaultPrevented) track()
        }}
      >
        <Label for="train-no">Train</Label>
        <AutoCompleteInput
          id="train-no"
          bind:value={query}
          kind="train"
          placeholder="Train number or name…"
          onpick={(item) => {
            query = String(item.number)
            track(item.number)
          }}
        />
      </div>
      <Button type="button" onclick={() => track()} disabled={phase === 'loading' || phase === 'refreshing'}>
        {phase === 'refreshing' ? 'Refreshing…' : 'Track'}
      </Button>
      <label class="mb-0.5 flex cursor-pointer items-center gap-2 text-sm text-muted-foreground">
        <input type="checkbox" bind:checked={auto} class="size-4 accent-[var(--primary)]" />
        Auto 30s
      </label>
    </Card.Content>
  </Card.Root>

  <Tabs.Root bind:value={activeTab} onValueChange={onTabChange}>
    <Tabs.List class="w-full justify-start">
      <Tabs.Trigger value="status"><ActivityIcon class="mr-2 size-4" />Status</Tabs.Trigger>
      <Tabs.Trigger value="schedule"><CalendarClockIcon class="mr-2 size-4" />Schedule</Tabs.Trigger>
      <Tabs.Trigger value="avg"><ChartColumnIcon class="mr-2 size-4" />Avg delay</Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="status" class="mt-4 grid gap-4">
      {#if phase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if phase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load live status</Alert.Title>
          <Alert.Description>{errorMsg}</Alert.Description>
        </Alert.Root>
      {:else if data}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title>
                <span class="font-mono">{data.train_number ?? '—'}</span> · {data.train_name ?? ''}
              </Card.Title>
              <Card.Description>{data.stations?.length ?? 0} stations on the run</Card.Description>
            </div>
            <Badge variant={auto ? 'default' : 'outline'}>{auto ? 'auto 30s' : 'manual'}</Badge>
          </Card.Header>
          <Card.Content>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head>Station</Table.Head>
                  <Table.Head>Sched</Table.Head>
                  <Table.Head>Actual</Table.Head>
                  <Table.Head class="w-28">Delay</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each data.stations ?? [] as s (s.code + s.name)}
                  {@const d = delayBadge(s.delay_minutes)}
                  <Table.Row>
                    <Table.Cell>
                      <span class="font-medium">{s.name}</span>
                      <span class="ml-2 font-mono text-xs text-muted-foreground">{s.code}</span>
                    </Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmtTime(s.scheduled_arrival)}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmtTime(s.actual_arrival)}</Table.Cell>
                    <Table.Cell><Badge variant={d.variant}>{d.text}</Badge></Table.Cell>
                  </Table.Row>
                {:else}
                  <Table.Row>
                    <Table.Cell colspan={4} class="text-muted-foreground">No station data returned.</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </Card.Content>
        </Card.Root>
      {:else}
        <p class="text-sm text-muted-foreground">Enter a train number or name above and press Track to see live status.</p>
      {/if}
    </Tabs.Content>

    <Tabs.Content value="schedule" class="mt-4 grid gap-4">
      {#if !committed}
        <p class="text-sm text-muted-foreground">Enter a train number above to load its schedule.</p>
      {:else if schPhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3, 4] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if schPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load schedule</Alert.Title>
          <Alert.Description>{schErr}</Alert.Description>
        </Alert.Root>
      {:else if schData}
        <Card.Root>
          <Card.Header>
            <Card.Title>
              <span class="font-mono">{schData.train_number ?? '—'}</span> · {schData.train_name ?? ''}
            </Card.Title>
            <Card.Description>{schData.stops?.length ?? 0} scheduled stops</Card.Description>
          </Card.Header>
          <Card.Content class="grid gap-4">
            <div class="flex flex-wrap items-center gap-1.5">
              {#each DAY_LETTERS as letter, i (i)}
                {#if schData.running_days?.[i]}
                  <Badge variant="default">{letter}</Badge>
                {:else}
                  <Badge variant="outline" class="text-muted-foreground opacity-60">{letter}</Badge>
                {/if}
              {/each}
            </div>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head>Code</Table.Head>
                  <Table.Head>Station</Table.Head>
                  <Table.Head>Arr</Table.Head>
                  <Table.Head>Dep</Table.Head>
                  <Table.Head class="w-16">Day</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each schData.stops ?? [] as s (s.code + s.name)}
                  <Table.Row>
                    <Table.Cell class="font-mono text-xs">{s.code}</Table.Cell>
                    <Table.Cell class="font-medium">{s.name}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmtTime(s.arrival)}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmtTime(s.departure)}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{s.day ?? '—'}</Table.Cell>
                  </Table.Row>
                {:else}
                  <Table.Row>
                    <Table.Cell colspan={5} class="text-muted-foreground">No stops returned.</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </Card.Content>
        </Card.Root>
      {:else}
        <p class="text-sm text-muted-foreground">No schedule loaded for this train yet.</p>
      {/if}
    </Tabs.Content>

    <Tabs.Content value="avg" class="mt-4 grid gap-4">
      {#if !committed}
        <p class="text-sm text-muted-foreground">Enter a train number above to load average delays.</p>
      {:else if avgPhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3, 4] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if avgPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load average delay</Alert.Title>
          <Alert.Description>{avgErr}</Alert.Description>
        </Alert.Root>
      {:else if avgData}
        {@const maxD = maxDelay(avgData.stations)}
        <Card.Root>
          <Card.Header>
            <Card.Title>
              <span class="font-mono">{avgData.train_no ?? '—'}</span> · {avgData.train_name ?? ''}
            </Card.Title>
            <Card.Description>
              Average arrival / departure delays{avgData.days_of_run ? ` · runs: ${avgData.days_of_run}` : ''}
            </Card.Description>
          </Card.Header>
          <Card.Content>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head>Station</Table.Head>
                  <Table.Head>arrival_delay</Table.Head>
                  <Table.Head>departure_delay</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each avgData.stations ?? [] as s (s.sr + s.code + s.name)}
                  {@const aPct = barPct(s.arrival_delay, maxD)}
                  {@const dPct = barPct(s.departure_delay, maxD)}
                  <Table.Row>
                    <Table.Cell>
                      <span class="font-medium">{s.name}</span>
                      <span class="ml-2 font-mono text-xs text-muted-foreground">{s.code}</span>
                    </Table.Cell>
                    <Table.Cell>
                      <div class="flex items-center gap-2">
                        <div class="h-2 flex-1 overflow-hidden rounded bg-muted">
                          <div class="h-full rounded bg-primary" style={`width:${aPct ?? 0}%`}></div>
                        </div>
                        <span class="w-12 shrink-0 text-right font-mono text-xs">{delayLabel(s.arrival_delay)}</span>
                      </div>
                    </Table.Cell>
                    <Table.Cell>
                      <div class="flex items-center gap-2">
                        <div class="h-2 flex-1 overflow-hidden rounded bg-muted">
                          <div class="h-full rounded bg-primary" style={`width:${dPct ?? 0}%`}></div>
                        </div>
                        <span class="w-12 shrink-0 text-right font-mono text-xs">{delayLabel(s.departure_delay)}</span>
                      </div>
                    </Table.Cell>
                  </Table.Row>
                {:else}
                  <Table.Row>
                    <Table.Cell colspan={3} class="text-muted-foreground">No station data returned.</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </Card.Content>
        </Card.Root>
      {:else}
        <p class="text-sm text-muted-foreground">No average-delay data loaded yet.</p>
      {/if}
    </Tabs.Content>
  </Tabs.Root>
</section>
