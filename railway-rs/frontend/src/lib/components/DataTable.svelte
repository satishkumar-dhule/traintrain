<script>
  import { viewport } from '$lib/media.svelte.js'
  import * as Table from '$lib/components/ui/table/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import ArrowUpIcon from 'lucide-svelte/icons/arrow-up'
  import ArrowDownIcon from 'lucide-svelte/icons/arrow-down'
  import ArrowUpDownIcon from 'lucide-svelte/icons/arrow-down-up'
  import SearchIcon from 'lucide-svelte/icons/search'
  import ListFilterIcon from 'lucide-svelte/icons/list-filter'

  let {
    columns = [],
    rows = [],
    rowKey = null,
    cell = null,
    cells = {},
    empty = 'No rows to show.',
    filteredEmpty = 'No rows match the current filters.',
    showMeta = true,
    /* Card view (mobile): column key used as the card title. Defaults to a
       heuristic (first identity-ish column). */
    primary = null,
    /* Optional (row) => string override for the card title text. */
    titleText = null,
    /* Optional snippet (row) rendered as a trailing action cell (desktop)
       / card footer (mobile). Excluded from sort + filter + chips. */
    actions = null,
  } = $props()

  let filters = $state({})
  let sortKey = $state(null)
  let sortDir = $state(0) // 1 asc · -1 desc · 0 none
  let mobileQuery = $state('')

  /* --- memoized text / sort caches (WeakMap per row -> Map<colKey,text>) --- */
  const _textCache = new WeakMap()
  const _sortCache = new WeakMap()

  function textOf(row, col) {
    if (row && typeof row === 'object') {
      let m = _textCache.get(row)
      if (m?.has(col.key)) return m.get(col.key)
      const v = col.value ? col.value(row) : row?.[col.key]
      const s = v == null ? '' : String(v).trim()
      if (!m) {
        m = new Map()
        _textCache.set(row, m)
      }
      m.set(col.key, s)
      return s
    }
    const v = col.value ? col.value(row) : row?.[col.key]
    return v == null ? '' : String(v).trim()
  }

  function sortVal(row, col) {
    if (row && typeof row === 'object') {
      let m = _sortCache.get(row)
      if (m?.has(col.key)) return m.get(col.key)
      let out
      if (col.sortValue) out = col.sortValue(row)
      else {
        const s = textOf(row, col)
        if (!s || s === '-' || s === '--' || s === '—') out = null
        else {
          const t = /^(\d{1,2}):(\d{2})(?::(\d{2}))?$/.exec(s)
          if (t) out = +t[1] * 3600 + +t[2] * 60 + (+t[3] || 0)
          else {
            const cleaned = s
              .replace(/[,\s₹$]/g, '')
              .replace(/(min(?:ute)?s?|sec(?:ond)?s?|hrs?|hours?|kms?|ms|%)$/i, '')
            if (/^-?\d+(\.\d+)?$/.test(cleaned)) out = Number(cleaned)
            else out = s.toLowerCase()
          }
        }
      }
      if (!m) {
        m = new Map()
        _sortCache.set(row, m)
      }
      m.set(col.key, out)
      return out
    }
    if (col.sortValue) return col.sortValue(row)
    const s = textOf(row, col)
    if (!s || s === '-' || s === '--' || s === '—') return null
    const t = /^(\d{1,2}):(\d{2})(?::(\d{2}))?$/.exec(s)
    if (t) return +t[1] * 3600 + +t[2] * 60 + (+t[3] || 0)
    const cleaned = s
      .replace(/[,\s₹$]/g, '')
      .replace(/(min(?:ute)?s?|sec(?:ond)?s?|hrs?|hours?|kms?|ms|%)$/i, '')
    if (/^-?\d+(\.\d+)?$/.test(cleaned)) return Number(cleaned)
    return s.toLowerCase()
  }

  const visible = $derived.by(() => {
    let out = Array.isArray(rows) ? rows : []
    const q = mobileQuery.trim().toLowerCase()
    if (q) out = out.filter((r) => columns.some((c) => textOf(r, c).toLowerCase().includes(q)))
    const active = []
    for (const c of columns) {
      const f = String(filters[c.key] ?? '').trim().toLowerCase()
      if (f) active.push([c, f])
    }
    if (active.length) {
      out = out.filter((r) => active.every(([c, q]) => textOf(r, c).toLowerCase().includes(q)))
    }
    if (sortKey && sortDir !== 0) {
      const col = columns.find((c) => c.key === sortKey)
      if (col) {
        out = out.slice().sort((a, b) => {
          const va = sortVal(a, col)
          const vb = sortVal(b, col)
          if (va == null && vb == null) return 0
          if (va == null) return 1
          if (vb == null) return -1
          let c = 0
          if (typeof va === 'number' && typeof vb === 'number') c = va - vb
          else c = String(va).localeCompare(String(vb), undefined, { numeric: true, sensitivity: 'base' })
          return c * sortDir
        })
      }
    }
    return out
  })

  const filtering = $derived(
    mobileQuery.trim() !== '' || columns.some((c) => String(filters[c.key] ?? '').trim() !== '')
  )
  const sorting = $derived(!!(sortKey && sortDir !== 0))

  function toggleSort(col) {
    if (sortKey !== col.key || sortDir === 0) {
      sortKey = col.key
      sortDir = 1
    } else if (sortDir === 1) {
      sortDir = -1
    } else {
      sortKey = null
      sortDir = 0
    }
  }

  /* super n2 delegation: single handler reading data-col-key */
  function handleSortClick(e) {
    const key = e.currentTarget?.dataset?.colKey
    if (!key) return
    const col = columns.find((c) => c.key === key)
    if (!col) return
    toggleSort(col)
  }

  function clearAll() {
    filters = {}
    sortKey = null
    sortDir = 0
    mobileQuery = ''
  }

  function keyOf(row, i) {
    if (rowKey) {
      const k = rowKey(row, i)
      return k == null ? i : k
    }
    return i
  }

  /* ----- card-view helpers ----- */
  const primaryKey = $derived(
    primary ??
      columns.find((c) => /train|station|name|date|kind|origin|event/i.test(c.key))?.key ??
      columns[0]?.key
  )
  const primaryCol = $derived(columns.find((c) => c.key === primaryKey) ?? null)
  const chipCols = $derived(columns.filter((c) => c.key !== primaryKey))
  const sortableCols = $derived(columns.filter((c) => c.sortable !== false))

  function titleOf(row) {
    if (titleText) return titleText(row)
    return primaryCol ? textOf(row, primaryCol) : ''
  }

  function renderCell(row, col) {
    if (cells && cells[col.key]) return cells[col.key](row, col)
    if (cell) return cell(row, col)
    return textOf(row, col) || '—'
  }

  const EMPTY_VALUES = new Set(['', '-', '--', '—'])

  /* Mobile toolbar: filter input is hidden behind a toggle so the sort chips
     row doubles as the only control row. */
  let filterOpen = $state(false)
</script>

{#if viewport.narrow}
  <!-- ===== Mobile: search + sort chips + stacked cards (no <table>) ===== -->
  <div class="grid gap-2">
    {#if filterOpen}
      <div class="relative">
        <SearchIcon
          class="pointer-events-none absolute top-1/2 left-3 size-[1.125rem] -translate-y-1/2 text-muted-foreground"
        />
        <Input
          type="text"
          placeholder={`Filter ${rows.length} rows…`}
          aria-label="Filter rows"
          bind:value={mobileQuery}
          onkeydown={(e) => {
            if (e.key === 'Enter') e.preventDefault()
          }}
          class="h-11 pl-11"
        />
      </div>
    {/if}

    <div class="flex min-w-0 items-start gap-1">
      {#if sortableCols.length > 0}
        <div
          class="flex min-w-0 flex-1 flex-wrap gap-1 pb-0.5"
          role="group"
          aria-label="Sort by"
        >
          {#each sortableCols as col (col.key)}
            {@const activeSort = sorting && sortKey === col.key}
            <button
              type="button"
              aria-pressed={activeSort}
              data-col-key={col.key}
              onclick={handleSortClick}
              title={col.label}
              class={`flex min-h-8 shrink-0 items-center gap-1 rounded-full border px-2.5 py-1 text-xs font-medium transition-colors max-lg:min-h-8 max-lg:px-2.5 max-lg:text-xs ${
                activeSort
                  ? 'border-primary bg-primary text-primary-foreground'
                  : 'border-border bg-card text-muted-foreground hover:bg-muted hover:text-foreground'
              }`}
            >
              <span class="max-sm:hidden">{col.label}</span>
              <span class="hidden max-sm:inline" aria-hidden="true">
                {#if col.label.toLowerCase().includes('station') }Sta
                {:else if col.label.toLowerCase().includes('sched') }Sch
                {:else if col.label.toLowerCase().includes('actual') }Act
                {:else if col.label.toLowerCase().includes('delay') }Dly
                {:else if col.label.toLowerCase().includes('status') }St
                {:else if col.label.toLowerCase().includes('arrival') }Arr
                {:else if col.label.toLowerCase().includes('departure') }Dep
                {:else if col.label.toLowerCase().includes('platform') }PF
                {:else}{col.label.slice(0,3)}
                {/if}
              </span>
              {#if activeSort}
                {#if sortDir === 1}
                  <ArrowUpIcon class="size-3" />
                {:else}
                  <ArrowDownIcon class="size-3" />
                {/if}
              {:else}
                <ArrowUpDownIcon class="size-3 opacity-40" />
              {/if}
            </button>
          {/each}
        </div>
      {:else}
        <div class="min-w-0 flex-1"></div>
      {/if}
      {#if rows.length > 3}
        <button
          type="button"
          onclick={() => (filterOpen = !filterOpen)}
          aria-expanded={filterOpen}
          aria-label="Filter rows"
          title="Filter rows"
          class={`flex size-11 shrink-0 items-center justify-center rounded-full border transition-colors ${
            filterOpen || mobileQuery
              ? 'border-primary bg-primary text-primary-foreground'
              : 'border-border bg-card text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <ListFilterIcon class="size-4" />
        </button>
      {/if}
    </div>

    <ul class="grid gap-1.5">
      {#each visible as row, i (keyOf(row, i))}
        {@const title = titleOf(row)}
        <li class="rounded-lg border bg-card">
          <div class="flex items-center gap-2 border-b px-3 py-2.5">
            <div class="min-w-0 flex-1 text-[0.95rem] leading-snug font-medium break-words">
              {#if primaryCol && cells && cells[primaryCol]}
                {@render cells[primaryCol](row, primaryCol)}
              {:else}
                {title || '—'}
              {/if}
            </div>
          </div>
          {#if chipCols.length}
            <div class="grid grid-cols-[repeat(auto-fill,minmax(6.25rem,1fr))] gap-x-2.5 gap-y-2 px-3 py-2.5">
              {#each chipCols as col (col.key)}
                {@const v = textOf(row, col)}
                {#if !EMPTY_VALUES.has(v)}
                  <div class="grid content-start gap-0.5">
                    <span class="text-[11px] leading-tight font-medium tracking-wide uppercase text-muted-foreground">
                      {col.label}
                    </span>
                    <span class="text-[15px] leading-snug break-words">
                      {#if cells && cells[col.key]}
                        {@render cells[col.key](row, col)}
                      {:else if cell}
                        {@render cell(row, col)}
                      {:else}
                        {v}
                      {/if}
                    </span>
                  </div>
                {/if}
              {/each}
            </div>
          {/if}
          {#if actions}
            <div class="flex flex-wrap items-center gap-x-2 gap-y-1 border-t px-3 py-2">
              {@render actions(row)}
            </div>
          {/if}
        </li>
      {:else}
        <li class="rounded-lg border bg-card px-3 py-6 text-center text-sm text-muted-foreground">
          {rows.length && filtering ? filteredEmpty : empty}
        </li>
      {/each}
    </ul>

    {#if showMeta && (filtering || sorting)}
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="text-xs text-muted-foreground">
          Showing {visible.length} of {(Array.isArray(rows) ? rows : []).length} rows
        </p>
        <Button variant="ghost" size="xs" onclick={clearAll}>
          Clear filters &amp; sort
        </Button>
      </div>
    {/if}
  </div>
{:else}
  <!-- ===== Desktop: full table with per-column filters ===== -->
  <div class="grid gap-2">
    <Table.Root>
      <Table.Header>
        <Table.Row>
          {#each columns as col (col.key)}
            <Table.Head
              class={col.class}
              aria-sort={sortKey === col.key && sortDir !== 0 ? (sortDir === 1 ? 'ascending' : 'descending') : undefined}
            >
              {#if col.sortable === false}
                <span class="px-0.5">{col.label}</span>
              {:else}
                <button
                  type="button"
                  data-col-key={col.key}
                  class="inline-flex cursor-pointer items-center gap-1 rounded-sm outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                  onclick={handleSortClick}
                  title="Sort by {col.label}"
                >
                  <span>{col.label}</span>
                  {#if sorting && sortKey === col.key}
                    {#if sortDir === 1}
                      <ArrowUpIcon class="size-3" />
                    {:else}
                      <ArrowDownIcon class="size-3" />
                    {/if}
                  {:else}
                    <ArrowUpDownIcon class="size-3 opacity-30" />
                  {/if}
                </button>
              {/if}
            </Table.Head>
          {/each}
          {#if actions}
            <Table.Head class="w-px"><span class="sr-only">Actions</span></Table.Head>
          {/if}
        </Table.Row>
        <Table.Row class="border-b-0">
          {#each columns as col (col.key)}
            <Table.Head class="h-auto py-1.5 {col.class ?? ''}">
              <Input
                type="text"
                placeholder="Filter…"
                aria-label={'Filter ' + (col.filterLabel ?? col.label)}
                class="h-7 px-2 text-xs"
                bind:value={filters[col.key]}
                onkeydown={(e) => {
                  if (e.key === 'Enter') e.preventDefault()
                }}
              />
            </Table.Head>
          {/each}
          {#if actions}
            <Table.Head class="h-auto w-px py-1.5"></Table.Head>
          {/if}
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {#each visible as row, i (keyOf(row, i))}
          <Table.Row>
            {#each columns as col (col.key)}
              <Table.Cell class={col.cellClass}>
                {#if cells && cells[col.key]}
                  {@render cells[col.key](row, col)}
                {:else if cell}
                  {@render cell(row, col)}
                {:else}
                  {textOf(row, col) || '—'}
                {/if}
              </Table.Cell>
            {/each}
            {#if actions}
              <Table.Cell class="w-px whitespace-nowrap">
                {@render actions(row)}
              </Table.Cell>
            {/if}
          </Table.Row>
        {:else}
          <Table.Row>
            <Table.Cell colspan={columns.length + (actions ? 1 : 0)} class="text-muted-foreground">
              {rows.length && filtering ? filteredEmpty : empty}
            </Table.Cell>
          </Table.Row>
        {/each}
      </Table.Body>
    </Table.Root>
    {#if showMeta && (filtering || sorting)}
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="text-xs text-muted-foreground">Showing {visible.length} of {(Array.isArray(rows) ? rows : []).length} rows</p>
        <Button variant="ghost" size="xs" onclick={clearAll}>Clear filters &amp; sort</Button>
      </div>
    {/if}
  </div>
{/if}
