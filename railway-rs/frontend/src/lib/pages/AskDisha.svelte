<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import * as Select from '$lib/components/ui/select/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import SearchIcon from 'lucide-svelte/icons/search'
  import CalendarClockIcon from 'lucide-svelte/icons/calendar-clock'
  import MessageCircleQuestionIcon from 'lucide-svelte/icons/message-circle-question'
  import BotMessageSquareIcon from 'lucide-svelte/icons/bot-message-square'
  import DataTable from '$lib/components/DataTable.svelte'
import EmptyState from '$lib/components/EmptyState.svelte'

  const TRAIN_RE = /^[0-9]{1,5}$/
  const DATE_RE = /^\d{4}-\d{2}-\d{2}$/
  const LANGS = [
    { value: 'en', label: 'English' },
    { value: 'hi', label: 'हिन्दी' },
    { value: 'gu', label: 'ગુજરાતી' }
  ]
  const RUN_DAYS = [
    { label: 'Mon', key: 'trainRunsOnMon' },
    { label: 'Tue', key: 'trainRunsOnTue' },
    { label: 'Wed', key: 'trainRunsOnWed' },
    { label: 'Thu', key: 'trainRunsOnThu' },
    { label: 'Fri', key: 'trainRunsOnFri' },
    { label: 'Sat', key: 'trainRunsOnSat' },
    { label: 'Sun', key: 'trainRunsOnSun' }
  ]
  const STATION_LIMIT = 20

  let moduleDisabled = $state(false)

  function isDisabledRes(res) {
    if (res && res.status === 404) {
      moduleDisabled = true
      return true
    }
    return false
  }

  function fmt(v) {
    return v && v !== '-' && v !== '--' ? v : '—'
  }

  let tab = $state('stations')

  let stationQuery = $state('')
  let stPhase = $state('idle')
  let stError = $state(null)
  let stData = $state(null)
  let stSearched = $state('')
  let stKey = ''

  const stRows = $derived(
    Array.isArray(stData?.stations) ? stData.stations.slice(0, STATION_LIMIT) : []
  )

  const schSchedule = $derived(schData?.schedule ?? null)
  const schStops = $derived(Array.isArray(schSchedule?.stationList) ? schSchedule.stationList : [])

  const adStationCols = [
    { key: 'code', label: 'Code', class: 'w-24', cellClass: 'font-mono text-xs', value: (s) => fmt(s.code) },
    { key: 'name', label: 'Name', cellClass: 'font-medium', value: (s) => fmt(s.name) },
    { key: 'nameHi', label: 'हिंदी', cellClass: 'text-sm', value: (s) => fmt(s.nameHi) },
    { key: 'state', label: 'State', class: 'w-40', cellClass: 'text-xs text-muted-foreground', value: (s) => fmt(s.state) },
  ]

  const adRouteCols = [
    {
      key: 'n',
      label: '#',
      class: 'w-10',
      sortable: false,
      value: (stop) => String(schStops.indexOf(stop) + 1),
    },
    { key: 'code', label: 'Code', class: 'w-20', cellClass: 'font-mono text-xs', value: (s) => fmt(s.stationCode) },
    { key: 'station', label: 'Station', cellClass: 'font-medium', value: (s) => fmt(s.stationName) },
    { key: 'arr', label: 'Arr', class: 'w-20', cellClass: 'font-mono text-xs', value: (s) => fmt(s.arrivalTime) },
    { key: 'dep', label: 'Dep', class: 'w-20', cellClass: 'font-mono text-xs', value: (s) => fmt(s.departureTime) },
    { key: 'halt', label: 'Halt', class: 'w-20', cellClass: 'font-mono text-xs', value: (s) => fmt(s.haltTime) },
  ]

  async function loadStations(term) {
    if (moduleDisabled) return
    const q = String(term ?? '').trim()
    if (!q) return
    const k = `${q.toLowerCase()}`
    stKey = k
    stSearched = q
    stPhase = stData ? 'refreshing' : 'loading'
    stError = null
    const res = await api(`/rail-api/askdisha/stations?q=${encodeURIComponent(q)}`)
    if (stKey !== k) return
    if (isDisabledRes(res)) return
    if (res.ok) {
      stData = res.data
      stPhase = 'ok'
    } else {
      stPhase = 'error'
      stError = res.error || `HTTP ${res.status}`
    }
  }

  $effect(() => {
    const term = stationQuery.trim()
    if (term.length < 2) {
      untrack(() => {
        stKey = ''
        stSearched = ''
        stPhase = 'idle'
        stError = null
        stData = null
      })
      return
    }
    const timer = setTimeout(() => loadStations(term), 300)
    return () => clearTimeout(timer)
  })

  let trainNo = $state('')
  let schedDate = $state('')
  let schedFrom = $state('')
  let schPhase = $state('idle')
  let schError = $state(null)
  let schData = $state(null)
  let schKey = ''

  const trainValid = $derived(TRAIN_RE.test(trainNo.trim()))
  const schedBusy = $derived(schPhase === 'loading' || schPhase === 'refreshing')
  const pageIdle = $derived(
    !moduleDisabled &&
      (tab === 'stations'
        ? stPhase === 'idle'
        : tab === 'schedule'
          ? schPhase === 'idle'
          : fqPhase === 'idle')
  )

  async function loadSchedule() {
    if (moduleDisabled) return
    const t = trainNo.trim()
    if (!TRAIN_RE.test(t)) return
    const d = DATE_RE.test(schedDate.trim()) ? schedDate.trim() : ''
    const f = schedFrom.trim().toUpperCase()
    const k = `${t}|${d}|${f}`
    schKey = k
    schPhase = schData ? 'refreshing' : 'loading'
    schError = null
    const qs = []
    if (d) qs.push(`date=${encodeURIComponent(d)}`)
    if (f) qs.push(`from=${encodeURIComponent(f)}`)
    const res = await api(
      `/rail-api/askdisha/schedule/${encodeURIComponent(t)}${qs.length ? `?${qs.join('&')}` : ''}`
    )
    if (schKey !== k) return
    if (isDisabledRes(res)) return
    if (res.ok) {
      schData = res.data
      schPhase = 'ok'
    } else {
      schPhase = 'error'
      schError = res.error || `HTTP ${res.status}`
    }
  }

  let faqLang = $state('en')
  let faqFilter = $state('')
  let fqPhase = $state('idle')
  let fqError = $state(null)
  let fqData = $state(null)
  let fqKey = ''

  const fqAll = $derived(Array.isArray(fqData?.faqs) ? fqData.faqs : [])
  const filteredFaqs = $derived(filterFaqs(fqAll, faqFilter))
  const fqBusy = $derived(fqPhase === 'loading' || fqPhase === 'refreshing')

  function filterFaqs(list, needleRaw) {
    const needle = String(needleRaw ?? '').trim().toLowerCase()
    if (!needle) return list
    return list.filter((f) => String(f).toLowerCase().includes(needle))
  }

  function langLabel(value) {
    return LANGS.find((l) => l.value === value)?.label ?? value
  }

  async function loadFaqs(target) {
    if (moduleDisabled) return
    const lang = LANGS.some((l) => l.value === target) ? target : 'en'
    const k = lang
    fqKey = k
    fqPhase = fqData ? 'refreshing' : 'loading'
    fqError = null
    const res = await api(`/rail-api/askdisha/faqs?lang=${encodeURIComponent(lang)}`)
    if (fqKey !== k) return
    if (isDisabledRes(res)) return
    if (res.ok) {
      fqData = res.data
      fqPhase = 'ok'
    } else {
      fqPhase = 'error'
      fqError = res.error || `HTTP ${res.status}`
    }
  }

  $effect(() => {
    if (tab !== 'faqs') return
    if (fqKey === faqLang) return
    untrack(() => loadFaqs(faqLang))
  })
</script>

{#snippet adNumCell(stop)}
  <span class="text-muted-foreground">{schStops.indexOf(stop) + 1}</span>
{/snippet}

<section class="grid gap-6" class:idle-center={pageIdle}>
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Ask DISHA</h1>
    <p class="text-sm text-muted-foreground">
      Station search, train schedule and passenger FAQs from the Indian Railways Ask DISHA
      assistant.
    </p>
  </div>

  {#if moduleDisabled}
    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2">
          <BotMessageSquareIcon class="size-5" />
          Ask DISHA module disabled
        </Card.Title>
        <Card.Description>Set ASKDISHA_ENABLED=1 to enable the Ask DISHA endpoints.</Card.Description>
      </Card.Header>
    </Card.Root>
  {:else}
    <Tabs.Root bind:value={tab}>
      <Tabs.List class="w-full justify-start">
        <Tabs.Trigger value="stations"><SearchIcon class="mr-2 size-4" />Stations</Tabs.Trigger>
        <Tabs.Trigger value="schedule"
          ><CalendarClockIcon class="mr-2 size-4" />Schedule</Tabs.Trigger
        >
        <Tabs.Trigger value="faqs"
          ><MessageCircleQuestionIcon class="mr-2 size-4" />FAQs</Tabs.Trigger
        >
      </Tabs.List>

      <Tabs.Content value="stations" class="mt-4 grid gap-4">
        <Card.Root>
          <Card.Content class="grid gap-3">
            <div class="grid gap-2">
              <Label for="ad-station-q">Search stations</Label>
              <Input
                id="ad-station-q"
                bind:value={stationQuery}
                placeholder="Station name or code, e.g. rajkot or Rajdhani"
              />
              {#if stationQuery.trim().length < 2}
                <p class="text-xs text-muted-foreground">
                  Type at least 2 characters to search names and codes.
                </p>
              {/if}
            </div>
          </Card.Content>
        </Card.Root>

        {#if stPhase === 'loading'}
          <div class="grid gap-2" aria-busy="true">
            {#each [0, 1, 2, 3] as i (i)}
              <Skeleton class="h-10 w-full" />
            {/each}
          </div>
        {:else if stPhase === 'error'}
          <Alert.Root variant="destructive" role="alert">
            <Alert.Title>Could not search stations</Alert.Title>
            <Alert.Description>{stError}</Alert.Description>
          </Alert.Root>
        {:else if stRows.length > 0}
          <Card.Root>
            <Card.Header class="flex-row items-center justify-between space-y-0">
              <div class="grid gap-1">
                <Card.Title>Matching stations</Card.Title>
                <Card.Description>Results for “{stSearched}”</Card.Description>
              </div>
              <Badge variant="secondary">{stRows.length} shown</Badge>
            </Card.Header>
            <Card.Content>
              <DataTable
                columns={adStationCols}
                rows={stRows}
                rowKey={(s, i) => `${i}|${s?.code ?? ''}`}
                empty={`No stations matched “${stSearched}”.`}
              />
            </Card.Content>
          </Card.Root>
          <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>source:</span>
            <Badge variant="outline" class="font-normal">{stData?.source ?? 'corover-api'}</Badge>
            {#if stData?.cached}<Badge variant="secondary">cached</Badge>{/if}
          </div>
        {:else if stData}
          <EmptyState
            icon={SearchIcon}
            title="No matches"
            hint={`No stations matched “${stSearched}”. Try a different name or code.`}
          />
        {:else}
          <EmptyState
            icon={SearchIcon}
            title="Search the directory"
            hint="Start typing to search the Ask DISHA station directory."
          />
        {/if}
      </Tabs.Content>

      <Tabs.Content value="schedule" class="mt-4 grid gap-4">
        <Card.Root>
          <Card.Content class="flex flex-wrap items-end gap-3">
            <div class="grid gap-2">
              <Label for="ad-train-no">Train number</Label>
              <Input
                id="ad-train-no"
                bind:value={trainNo}
                placeholder="e.g. 12951"
                inputmode="numeric"
                maxlength={5}
                class="w-32 font-mono"
                onkeydown={(e) => e.key === 'Enter' && !e.defaultPrevented && loadSchedule()}
              />
              {#if trainNo.trim() && !trainValid}
                <p class="text-xs text-muted-foreground">Train number must be 1–5 digits.</p>
              {/if}
            </div>
            <div class="grid gap-2">
              <Label for="ad-sched-date">Journey date (optional)</Label>
              <Input id="ad-sched-date" type="date" bind:value={schedDate} class="w-44" />
            </div>
            <div class="grid gap-2">
              <Label for="ad-sched-from">From code (optional)</Label>
              <Input
                id="ad-sched-from"
                bind:value={schedFrom}
                placeholder="e.g. BCT"
                maxlength={10}
                class="w-28 font-mono uppercase"
              />
            </div>
            <Button type="button" onclick={() => loadSchedule()} disabled={!trainValid || schedBusy}>
              {schPhase === 'refreshing' ? 'Refreshing…' : 'Get schedule'}
            </Button>
          </Card.Content>
        </Card.Root>

        {#if schPhase === 'loading'}
          <div class="grid gap-2" aria-busy="true">
            {#each [0, 1, 2, 3, 4] as i (i)}
              <Skeleton class="h-10 w-full" />
            {/each}
          </div>
        {:else if schPhase === 'error'}
          <Alert.Root variant="destructive" role="alert">
            <Alert.Title>Could not load schedule</Alert.Title>
            <Alert.Description>{schError}</Alert.Description>
          </Alert.Root>
        {:else if schData?.schedule}
          {@const schedule = schData.schedule}
          {@const stops = Array.isArray(schedule.stationList) ? schedule.stationList : []}
          {@const fromCode = schedule.stationFrom || stops[0]?.stationCode}
          {@const toCode = schedule.stationTo || stops[stops.length - 1]?.stationCode}
          <Card.Root>
            <Card.Header class="gap-3 space-y-0">
              <div class="flex flex-wrap items-center justify-between gap-2">
                <Card.Title>
                  <span class="font-mono">{fmt(schedule.trainNumber)}</span> ·
                  {schedule.trainName ?? 'Train schedule'}
                </Card.Title>
                <Badge variant="secondary">{stops.length} stops</Badge>
              </div>
              <Card.Description>
                {fmt(fromCode)} → {fmt(toCode)}
              </Card.Description>
              <div class="flex flex-wrap items-center gap-1.5 pt-1">
                <span class="mr-1 text-xs text-muted-foreground">Runs on</span>
                {#each RUN_DAYS as day (day.key)}
                  {#if schedule[day.key] === true}
                    <Badge variant="secondary" class="px-1.5 text-[10px]">{day.label}</Badge>
                  {:else}
                    <Badge variant="outline" class="px-1.5 text-[10px] opacity-40">{day.label}</Badge>
                  {/if}
                {/each}
              </div>
            </Card.Header>
            <Card.Content>
              <DataTable
                columns={adRouteCols}
                rows={schStops}
                rowKey={(stop, i) => `${i}|${stop?.stationCode ?? ''}`}
                cells={{ n: adNumCell }}
                empty="No stops returned for this train."
              />
            </Card.Content>
          </Card.Root>
          <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>source:</span>
            <Badge variant="outline" class="font-normal">{schData?.source ?? 'corover-api'}</Badge>
            {#if schData?.cached}<Badge variant="secondary">cached</Badge>{/if}
          </div>
        {:else if schData}
          <EmptyState
            icon={CalendarClockIcon}
            title="No schedule found"
            hint={`No schedule found for “${trainNo.trim()}”.`}
          />
        {:else}
          <EmptyState
            icon={CalendarClockIcon}
            title="Look up a schedule"
            hint="Enter a train number to see its route, timings and running days."
          />
        {/if}
      </Tabs.Content>

      <Tabs.Content value="faqs" class="mt-4 grid gap-4">
        <Card.Root>
          <Card.Content class="flex flex-wrap items-end gap-3">
            <div class="grid gap-2">
              <Label for="ad-faq-lang">Language</Label>
              <Select.Root type="single" bind:value={faqLang}>
                <Select.Trigger class="w-36" aria-label="FAQ language">
                  {langLabel(faqLang)}
                </Select.Trigger>
                <Select.Content>
                  {#each LANGS as l (l.value)}
                    <Select.Item value={l.value} label={l.label} />
                  {/each}
                </Select.Content>
              </Select.Root>
            </div>
            <div class="grid min-w-56 flex-1 gap-2">
              <Label for="ad-faq-filter">Filter questions</Label>
              <Input
                id="ad-faq-filter"
                bind:value={faqFilter}
                placeholder="Filter loaded FAQs, e.g. ticket"
              />
            </div>
          </Card.Content>
        </Card.Root>

        {#if fqPhase === 'loading'}
          <div class="grid gap-2" aria-busy="true">
            {#each [0, 1, 2, 3, 4, 5] as i (i)}
              <Skeleton class="h-10 w-full" />
            {/each}
          </div>
        {:else if fqPhase === 'error'}
          <Alert.Root variant="destructive" role="alert">
            <Alert.Title>Could not load FAQs</Alert.Title>
            <Alert.Description>{fqError}</Alert.Description>
          </Alert.Root>
        {:else if fqData}
          <Card.Root>
            <Card.Header class="flex-row items-center justify-between space-y-0">
              <div class="grid gap-1">
                <Card.Title>Frequently asked questions</Card.Title>
                <Card.Description>{langLabel(faqLang)}</Card.Description>
              </div>
              <Badge variant="secondary">{filteredFaqs.length} of {fqAll.length}</Badge>
            </Card.Header>
            <Card.Content class="grid gap-2">
              {#each filteredFaqs as faq, i (i)}
                <p class="rounded-md border px-3 py-2 text-sm leading-relaxed">{faq}</p>
              {:else}
                <p class="text-sm text-muted-foreground">
                  {fqAll.length === 0
                    ? 'No FAQs returned for this language.'
                    : 'No FAQs match your filter.'}
                </p>
              {/each}
            </Card.Content>
          </Card.Root>
          <div class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span>source:</span>
            <Badge variant="outline" class="font-normal">{fqData?.source ?? 'corover-cdn'}</Badge>
            {#if fqData?.cached}<Badge variant="secondary">cached</Badge>{/if}
          </div>
        {:else}
          <div class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            Loading passenger FAQs…
          </div>
        {/if}
      </Tabs.Content>
    </Tabs.Root>
  {/if}
</section>
