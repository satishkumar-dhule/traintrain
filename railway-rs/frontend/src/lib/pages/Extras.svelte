<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import LandmarkIcon from 'lucide-svelte/icons/landmark'
  import PackageIcon from 'lucide-svelte/icons/package'
  import FilterIcon from 'lucide-svelte/icons/filter'

  let { view = '', selection = '' } = $props()

  const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

  let tab = $state('heritage')
  let selInput = $state('')

  let hPhase = $state('idle')
  let hError = $state(null)
  let hData = $state(null)

  let pPhase = $state('idle')
  let pError = $state(null)
  let pData = $state(null)

  let hKey = null
  let hBusy = false
  let pStarted = false

  const hTrains = $derived(listOf(hData))
  const hTotal = $derived(totalOf(hData))
  const hCaption = $derived(str(hData?.selection).trim())
  const hSource = $derived(str(hData?.data_source).trim())
  const pTrains = $derived(listOf(pData))
  const pTotal = $derived(totalOf(pData))
  const pSource = $derived(str(pData?.data_source).trim())

  function str(v) {
    return v == null ? '' : String(v)
  }

  function fmt(v) {
    const s = str(v).trim()
    return s && s !== '-' && s !== '--' ? s : '—'
  }

  function listOf(d) {
    if (Array.isArray(d?.trains)) return d.trains
    if (Array.isArray(d?.list)) return d.list
    return []
  }

  function totalOf(d) {
    if (!d) return 0
    const n = Number(d.total)
    return Number.isFinite(n) && n > 0 ? n : listOf(d).length
  }

  function stopOf(t, nestedKey, flatPrefix) {
    const nested = t?.[nestedKey]
    const o = nested && typeof nested === 'object' ? nested : {}
    const flat = (suffix) => str(t?.[`${flatPrefix}_${suffix}`]).trim()
    return {
      code: str(o.code ?? flat('code')).trim(),
      name: str(o.name ?? o.station ?? flat('station')).trim(),
      time: str(o.time ?? flat('time')).trim()
    }
  }

  function validityOf(t) {
    const from = fmt(t?.validity_from)
    const to = fmt(t?.validity_to)
    if (from === '—' && to === '—') return '—'
    if (from === '—') return to
    if (to === '—') return from
    return `${from} – ${to}`
  }

  function dayFlags(raw) {
    const s = str(raw).trim().toLowerCase()
    const flags = [false, false, false, false, false, false, false]
    if (!s || s === '-' || s === '--') return { flags, any: false }
    let any = false
    ;['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'].forEach((name, i) => {
      if (s.includes(name)) {
        flags[i] = true
        any = true
      }
    })
    if (!any) {
      const letters = s.replace(/[^a-z]/g, '')
      if (letters.length === 7) {
        for (let i = 0; i < 7; i++) {
          if (letters[i] !== '-') {
            flags[i] = true
            any = true
          }
        }
      }
    }
    return { flags, any }
  }

  async function loadHeritage(sel) {
    hKey = sel
    hBusy = true
    hPhase = hData ? 'refreshing' : 'loading'
    hError = null
    const qs = sel ? `?selection=${encodeURIComponent(sel)}` : ''
    const res = await api(`/rail-api/ntes/heritage${qs}`)
    hBusy = false
    if (hKey !== sel) return
    if (res.ok) {
      hData = res.data
      hPhase = 'ok'
    } else {
      hPhase = 'error'
      hError = res.error || `HTTP ${res.status}`
    }
  }

  async function loadParcel() {
    pStarted = true
    pPhase = pData ? 'refreshing' : 'loading'
    pError = null
    const res = await api('/rail-api/ntes/parcel')
    if (res.ok) {
      pData = res.data
      pPhase = 'ok'
    } else {
      pPhase = 'error'
      pError = res.error || `HTTP ${res.status}`
    }
  }

  function applyFilter() {
    const kw = str(selInput).trim()
    navigate(kw ? `/extras/heritage/${encodeURIComponent(kw)}` : '/extras/heritage')
  }

  function onTabChange(next) {
    if (!next || next === tab) return
    navigate(`/extras/${next}`)
  }

  $effect(() => {
    const nextTab = view === 'parcel' ? 'parcel' : 'heritage'
    const sel = str(selection).trim()
    untrack(() => {
      tab = nextTab
      if (nextTab === 'heritage') {
        if (selInput !== sel) selInput = sel
        if (!hBusy && hKey !== sel) loadHeritage(sel)
      } else if (!pStarted) {
        loadParcel()
      }
    })
  })
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Extras</h1>
    <p class="text-sm text-muted-foreground">Heritage trains and running parcel specials, live from NTES.</p>
  </div>

  <Tabs.Root bind:value={tab} onValueChange={onTabChange}>
    <Tabs.List class="w-full justify-start">
      <Tabs.Trigger value="heritage"><LandmarkIcon class="mr-2 size-4" />Heritage</Tabs.Trigger>
      <Tabs.Trigger value="parcel"><PackageIcon class="mr-2 size-4" />Parcel</Tabs.Trigger>
    </Tabs.List>

    <Tabs.Content value="heritage" class="mt-4 grid gap-4">
      <Card.Root>
        <Card.Content class="flex flex-wrap items-center gap-3">
          <Input
            bind:value={selInput}
            placeholder="Selection keyword, blank = all"
            class="min-w-56 flex-1"
            aria-label="Heritage selection keyword"
            onkeydown={(e) => {
              if (e.key === 'Enter') applyFilter()
            }}
          />
          <Button onclick={applyFilter}><FilterIcon class="mr-2 size-4" />Filter</Button>
        </Card.Content>
      </Card.Root>

      {#if hPhase === 'loading' || hPhase === 'refreshing'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if hPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load heritage trains</Alert.Title>
          <Alert.Description>{hError}</Alert.Description>
        </Alert.Root>
      {:else if hData}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title>Heritage trains</Card.Title>
              <Card.Description>
                {hTotal} train{hTotal === 1 ? '' : 's'}{hCaption ? ` · ${hCaption}` : ''}
              </Card.Description>
            </div>
            <div class="flex flex-wrap items-center justify-end gap-2">
              {#if str(selection).trim()}<Badge variant="outline">keyword: {selection}</Badge>{/if}
              {#if hSource}<Badge variant="secondary">{hSource}</Badge>{/if}
            </div>
          </Card.Header>
          <Card.Content>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head class="w-24">Number</Table.Head>
                  <Table.Head>Name</Table.Head>
                  <Table.Head>Runs</Table.Head>
                  <Table.Head class="w-28">Duration</Table.Head>
                  <Table.Head>Source → Destination</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each hTrains as t, i (str(t.number) + '|' + str(t.name) + '|' + i)}
                  {@const src = stopOf(t, 'source', 'source')}
                  {@const dst = stopOf(t, 'destination', 'dest')}
                  <Table.Row>
                    <Table.Cell><Badge variant="outline" class="font-mono">{fmt(t.number)}</Badge></Table.Cell>
                    <Table.Cell class="font-medium">{fmt(t.name)}</Table.Cell>
                    <Table.Cell class="text-muted-foreground">{fmt(t.runs)}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmt(t.duration)}</Table.Cell>
                    <Table.Cell>
                      <div class="flex flex-wrap items-center gap-x-2 font-mono text-xs">
                        <span title={src.name}>{src.code || '—'}{src.time ? ` · ${src.time}` : ''}</span>
                        <span class="text-muted-foreground">→</span>
                        <span title={dst.name}>{dst.code || '—'}{dst.time ? ` · ${dst.time}` : ''}</span>
                      </div>
                    </Table.Cell>
                  </Table.Row>
                {:else}
                  <Table.Row>
                    <Table.Cell colspan={5} class="text-muted-foreground">No heritage trains match this selection.</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </Card.Content>
        </Card.Root>
      {:else}
        <div class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
          Press Filter to list heritage trains.
        </div>
      {/if}
    </Tabs.Content>

    <Tabs.Content value="parcel" class="mt-4 grid gap-4">
      {#if pPhase === 'loading' || pPhase === 'refreshing'}
        <div class="grid gap-2" aria-busy="true">
          {#each [0, 1, 2, 3] as i (i)}
            <Skeleton class="h-10 w-full" />
          {/each}
        </div>
      {:else if pPhase === 'error'}
        <Alert.Root variant="destructive" role="alert">
          <Alert.Title>Could not load parcel specials</Alert.Title>
          <Alert.Description>{pError}</Alert.Description>
        </Alert.Root>
      {:else if pData}
        <Card.Root>
          <Card.Header class="flex-row items-center justify-between space-y-0">
            <div class="grid gap-1">
              <Card.Title>Parcel special trains</Card.Title>
              <Card.Description>{pTotal} train{pTotal === 1 ? '' : 's'} currently listed</Card.Description>
            </div>
            {#if pSource}<Badge variant="secondary">{pSource}</Badge>{/if}
          </Card.Header>
          <Card.Content>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.Head class="w-24">Number</Table.Head>
                  <Table.Head>Name</Table.Head>
                  <Table.Head>Route</Table.Head>
                  <Table.Head class="w-48">Validity</Table.Head>
                  <Table.Head class="w-24">Travel time</Table.Head>
                  <Table.Head>Days</Table.Head>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {#each pTrains as t, i (str(t.number) + '|' + str(t.name) + '|' + i)}
                  {@const days = dayFlags(t.days_of_run)}
                  <Table.Row>
                    <Table.Cell><span class="font-mono text-xs text-muted-foreground">{fmt(t.number)}</span></Table.Cell>
                    <Table.Cell class="max-w-40 truncate font-medium" title={str(t.name)}>{fmt(t.name)}</Table.Cell>
                    <Table.Cell class="max-w-56"><div class="truncate" title={str(t.route)}>{fmt(t.route)}</div></Table.Cell>
                    <Table.Cell class="whitespace-nowrap font-mono text-xs">{validityOf(t)}</Table.Cell>
                    <Table.Cell class="font-mono text-xs">{fmt(t.travel_time)}</Table.Cell>
                    <Table.Cell>
                      {#if days.any}
                        <div class="flex gap-1">
                          {#each DAY_LETTERS as letter, di (di)}
                            {#if days.flags[di]}
                              <Badge variant="secondary" class="px-1.5 text-[10px]">{letter}</Badge>
                            {:else}
                              <Badge variant="outline" class="px-1.5 text-[10px] opacity-40">{letter}</Badge>
                            {/if}
                          {/each}
                        </div>
                      {:else}
                        <span class="text-xs text-muted-foreground">{fmt(t.days_of_run)}</span>
                      {/if}
                    </Table.Cell>
                  </Table.Row>
                {:else}
                  <Table.Row>
                    <Table.Cell colspan={6} class="text-muted-foreground">No parcel special trains are currently listed.</Table.Cell>
                  </Table.Row>
                {/each}
              </Table.Body>
            </Table.Root>
          </Card.Content>
        </Card.Root>
      {:else}
        <div class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
          Open the Parcel tab to list running parcel special trains.
        </div>
      {/if}
    </Tabs.Content>
  </Tabs.Root>
</section>
