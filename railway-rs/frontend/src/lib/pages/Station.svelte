<script>
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'

  let { code = '' } = $props()

  let query = $state(code)
  let hours = $state('2')
  let dateInput = $state('')
  let tab = $state('live')

  let livePhase = $state('idle')
  let liveError = $state(null)
  let live = $state(null)

  let ttPhase = $state('idle')
  let ttError = $state(null)
  let timetable = $state(null)

  const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC']
  const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']

  function targetCode() {
    return String(query || '').trim().toUpperCase()
  }

  function ntesDate(iso) {
    if (!iso) return null
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso).trim())
    if (!m) return null
    const mi = Number(m[2]) - 1
    if (mi < 0 || mi > 11) return null
    return `${m[3]}-${MONTHS[mi]}-${m[1]}`
  }

  async function loadLive() {
    const target = targetCode()
    if (!target) return
    livePhase = live && `${live.station}` === `${target}` ? 'refreshing' : 'loading'
    liveError = null
    const res = await api(`/rail-api/ntes/live-station?station=${encodeURIComponent(target)}&hours=${hours}`)
    if (res.ok) {
      live = res.data
      livePhase = 'ok'
      if (`${target}` !== `${code}`) navigate(`/station/${target}`)
    } else {
      livePhase = 'error'
      liveError = res.error || `HTTP ${res.status}`
    }
  }

  async function loadTimetable() {
    const target = targetCode()
    if (!target) return
    ttPhase = timetable && `${timetable.station}` === `${target}` ? 'refreshing' : 'loading'
    ttError = null
    const d = ntesDate(dateInput)
    const qs = `station=${encodeURIComponent(target)}${d ? `&date=${encodeURIComponent(d)}` : ''}`
    const res = await api(`/rail-api/ntes/station-timetable?${qs}`)
    if (res.ok) {
      timetable = res.data
      ttPhase = 'ok'
      if (`${target}` !== `${code}`) navigate(`/station/${target}`)
    } else {
      ttPhase = 'error'
      ttError = res.error || `HTTP ${res.status}`
    }
  }

  function showBoard() {
    if (tab === 'timetable') loadTimetable()
    else loadLive()
  }

  $effect(() => {
    if (code) {
      query = code
      loadLive()
    }
  })

  function fmt(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }

  function upper() {
    query = String(query || '').toUpperCase()
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Station board</h1>
    <p class="text-sm text-muted-foreground">Live board and full-day timetable for any station.</p>
  </div>

  <Card.Root>
    <Card.Content class="flex flex-wrap items-end gap-3">
      <div class="grid min-w-40 flex-1 gap-2">
        <Label for="stn-code">Station code</Label>
        <Input
          id="stn-code"
          bind:value={query}
          oninput={upper}
          placeholder="e.g. NDLS"
          onkeydown={(e) => e.key === 'Enter' && showBoard()}
        />
      </div>
      <div class="grid gap-2">
        <Label>Window</Label>
        <Select.Root type="single" bind:value={hours}>
          <Select.Trigger class="w-32" aria-label="Time window">
            {hours} hour{hours === '1' ? '' : 's'}
          </Select.Trigger>
          <Select.Content>
            {#each ['1', '2', '3', '4'] as h (h)}
              <Select.Item value={h} label="{h} hour{h === '1' ? '' : 's'}" />
            {/each}
          </Select.Content>
        </Select.Root>
      </div>
      <div class="grid gap-2">
        <Label for="stn-date">Date (timetable)</Label>
        <Input id="stn-date" type="date" bind:value={dateInput} class="w-40" />
      </div>
      <Button onclick={showBoard} disabled={livePhase === 'loading' || livePhase === 'refreshing' || ttPhase === 'loading' || ttPhase === 'refreshing'}>
        {(livePhase === 'refreshing' && tab === 'live') || (ttPhase === 'refreshing' && tab === 'timetable') ? 'Refreshing…' : 'Show board'}
      </Button>
    </Card.Content>
  </Card.Root>

  <Tabs.Root bind:value={tab}>
    <Tabs.List>
      <Tabs.Trigger value="live">Live</Tabs.Trigger>
      <Tabs.Trigger value="timetable">Timetable</Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="live">
      {#if livePhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if livePhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load board</Alert.Title>
          <Alert.Description>{liveError}</Alert.Description>
        </Alert.Root>
      {:else if live}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title>{live.station ?? '—'} departures &amp; arrivals</Card.Title>
              <Card.Description>{live.trains?.length ?? 0} trains within {live.hours}h</Card.Description>
            </div>
            <Badge variant="secondary">{live.hours}h window</Badge>
          </Card.Header>
          <Card.Content>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head>Train</Table.Head>
                  <Table.Head>Sched</Table.Head>
                  <Table.Head>Expected</Table.Head>
                  <Table.Head class="w-24">Delay</Table.Head>
                  <Table.Head class="w-24">Platform</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each live.trains ?? [] as t (t.number + t.name)}
                  {@const late = Number(t.delay_arr) > 0}
                  <Table.Row>
                    <Table.Cell>
                      <span class="font-mono text-xs text-muted-foreground">{t.number}</span>
                      <span class="ml-2 font-medium">{t.name}</span>
                    </Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmt(t.sta)}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmt(t.eta)}</Table.Cell>
                    <Table.Cell>
                      {#if late}<Badge variant="destructive">{t.delay_arr}m</Badge>
                      {:else}<Badge variant="secondary">on time</Badge>{/if}
                    </Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmt(t.platform)}</Table.Cell>
                  </Table.Row>
                {:else}
                  <Table.Row>
                    <Table.Cell colspan={5} class="text-muted-foreground">No trains in this window.</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </Card.Content>
        </Card.Root>
      {:else}
        <div class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
          Enter a station code and show the board to see live arrivals &amp; departures.
        </div>
      {/if}
    </Tabs.Content>

    <Tabs.Content value="timetable">
      {#if ttPhase === 'loading'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if ttPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load timetable</Alert.Title>
          <Alert.Description>{ttError}</Alert.Description>
        </Alert.Root>
      {:else if timetable}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title>{timetable.station_name ?? timetable.station ?? '—'} timetable</Card.Title>
              <Card.Description>{timetable.total ?? timetable.trains?.length ?? 0} trains{timetable.date ? ` · ${timetable.date}` : ''}</Card.Description>
            </div>
            {#if timetable.date}<Badge variant="secondary">{timetable.date}</Badge>{/if}
          </Card.Header>
          <Card.Content>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head>Train</Table.Head>
                  <Table.Head>Type</Table.Head>
                  <Table.Head class="w-20">Arr</Table.Head>
                  <Table.Head class="w-20">Dep</Table.Head>
                  <Table.Head>Days</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each timetable.trains ?? [] as t (t.number + t.name)}
                  {@const days = Array.isArray(t.days) ? t.days : []}
                  <Table.Row>
                    <Table.Cell>
                      <span class="font-mono text-xs text-muted-foreground">{t.number}</span>
                      <span class="ml-2 font-medium">{t.name}</span>
                      {#if t.route}<span class="block text-xs text-muted-foreground">{t.route}</span>{/if}
                    </Table.Cell>
                    <Table.Cell class="text-xs">
                      {#if t.train_type}<Badge variant="outline">{t.train_type}</Badge>{:else}—{/if}
                      {#if t.classes}<span class="mt-1 block text-xs text-muted-foreground">{t.classes}</span>{/if}
                    </Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmt(t.arrival)}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmt(t.departure)}</Table.Cell>
                    <Table.Cell>
                      <div class="flex flex-wrap gap-1">
                        {#each DAYS as d, i (d)}
                          {#if days[i]}
                            <Badge variant="secondary" class="px-1.5 text-[10px]">{d}</Badge>
                          {:else}
                            <Badge variant="outline" class="px-1.5 text-[10px] opacity-40">{d}</Badge>
                          {/if}
                        {/each}
                      </div>
                    </Table.Cell>
                  </Table.Row>
                {:else}
                  <Table.Row>
                    <Table.Cell colspan={5} class="text-muted-foreground">No scheduled trains found.</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </Card.Content>
        </Card.Root>
      {:else}
        <div class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
          Enter a station code (and optionally a date) to load the full-day timetable.
        </div>
      {/if}
    </Tabs.Content>
  </Tabs.Root>
</section>
