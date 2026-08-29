<script>
  import * as Dialog from '$lib/components/ui/dialog/index.js'
  import * as Command from '$lib/components/ui/command/index.js'
  import { api } from '$lib/api.js'
  import { navigate } from '$lib/router.svelte.js'
  import { theme, toggleTheme } from '$lib/theme.svelte.js'
  import { palette, closePalette } from '$lib/palette.svelte.js'

  let q = $state('')
  let selected = $state('')
  let loading = $state(false)
  let stations = $state([])
  let trains = $state([])

  let timer = null
  let controller = null

  const actions = [
    { label: 'Go Home', to: '/' },
    { label: 'Live Status', to: '/train' },
    { label: 'Station Board', to: '/station' },
    { label: 'Journeys', to: '/journeys' },
    { label: 'Availability', to: '/availability' },
    { label: 'Chart Vacancy', to: '/chart' },
    { label: 'PNR', to: '/pnr' },
    { label: 'System', to: '/system' }
  ]

  const term = $derived(q.trim())
  const pnrValue = $derived(/^\d{10}$/.test(term) ? term : '')
  const showEmpty = $derived(term.length > 0 && !loading && stations.length === 0 && trains.length === 0)

  function abort() {
    if (controller) {
      controller.abort()
      controller = null
    }
  }

  async function lookup(text) {
    abort()
    const c = new AbortController()
    controller = c
    loading = true
    const res = await api(`/rail-api/search/suggest?q=${encodeURIComponent(text)}`)
    if (c.signal.aborted) return
    controller = null
    loading = false
    const rows = res.ok && Array.isArray(res.data) ? res.data : []
    stations = rows.filter((r) => r.type === 'station').slice(0, 8)
    trains = rows.filter((r) => r.type === 'train').slice(0, 8)
  }

  $effect(() => {
    if (!palette.open) {
      clearTimeout(timer)
      abort()
      return
    }
    q = ''
    stations = []
    trains = []
    loading = false
    selected = ''
  })

  $effect(() => {
    const text = q.trim()
    clearTimeout(timer)
    abort()
    if (!text) {
      stations = []
      trains = []
      loading = false
      return
    }
    timer = setTimeout(() => lookup(text), 200)
    return () => {
      clearTimeout(timer)
      abort()
    }
  })

  function go(to) {
    navigate(to)
    closePalette()
  }

  function pickTheme() {
    toggleTheme()
    closePalette()
  }
</script>

<Dialog.Root bind:open={() => palette.open, (v) => (palette.open = v)}>
  <Dialog.Content class="sm:max-w-xl p-0">
    <Dialog.Title class="sr-only">Command palette</Dialog.Title>
    <Dialog.Description class="sr-only">Search stations, trains and pages.</Dialog.Description>
    <Command.Root shouldFilter={false} bind:value={selected}>
      <Command.Input placeholder="Search stations, trains, pages…" bind:value={q} />
      <Command.List>
        {#if showEmpty}
          <Command.Empty forceMount>No results found.</Command.Empty>
        {/if}
        {#if pnrValue}
          <Command.Group heading="PNR">
            <Command.Item value={`pnr-${pnrValue}`} onSelect={() => go(`/pnr/${pnrValue}`)}>
              <span>Open PNR {pnrValue}</span>
            </Command.Item>
          </Command.Group>
        {/if}
        {#if stations.length > 0}
          <Command.Group heading="Stations">
            {#each stations as s (s.code)}
              <Command.Item value={`station-${s.code}`} onSelect={() => go(`/station/${s.code}`)}>
                <div class="min-w-0">
                  <div class="truncate">{s.name} · {s.code}</div>
                  {#if s.name_hi || s.name_gu}
                    <div class="truncate text-xs text-muted-foreground">
                      {s.name_hi || s.name_gu}{s.district ? ` · ${s.district}` : ''}
                    </div>
                  {/if}
                </div>
              </Command.Item>
            {/each}
          </Command.Group>
        {/if}
        {#if trains.length > 0}
          <Command.Group heading="Trains">
            {#each trains as t (t.number)}
              <Command.Item value={`train-${t.number}`} onSelect={() => go(`/train/${t.number}`)}>
                <span>{t.number} · {t.name}</span>
              </Command.Item>
            {/each}
          </Command.Group>
        {/if}
        <Command.Group heading="Actions">
          {#each actions as a (a.to)}
            <Command.Item value={`action-${a.label}`} onSelect={() => go(a.to)}>
              <span>{a.label}</span>
            </Command.Item>
          {/each}
        </Command.Group>
        <Command.Group heading="Display">
          <Command.Item value="display-toggle-theme" onSelect={pickTheme}>
            <span>{theme.mode === 'dark' ? 'Switch to bright theme' : 'Switch to dark theme'}</span>
          </Command.Item>
        </Command.Group>
      </Command.List>
    </Command.Root>
  </Dialog.Content>
</Dialog.Root>
