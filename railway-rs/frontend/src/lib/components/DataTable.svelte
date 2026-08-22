<script>
  import * as Table from '$lib/components/ui/table/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import ArrowUpIcon from 'lucide-svelte/icons/arrow-up'
  import ArrowDownIcon from 'lucide-svelte/icons/arrow-down'
  import ArrowUpDownIcon from 'lucide-svelte/icons/arrow-down-up'

  let {
    columns = [],
    rows = [],
    rowKey = null,
    cell = null,
    cells = {},
    empty = 'No rows to show.',
    filteredEmpty = 'No rows match the current filters.',
    showMeta = true,
  } = $props()

  let filters = $state({})
  let sortKey = $state(null)
  let sortDir = $state(0) // 1 asc · -1 desc · 0 none

  function textOf(row, col) {
    const v = col.value ? col.value(row) : row?.[col.key]
    return v == null ? '' : String(v).trim()
  }

  function sortVal(row, col) {
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
    const active = []
    for (const c of columns) {
      const q = String(filters[c.key] ?? '').trim().toLowerCase()
      if (q) active.push([c, q])
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

  const filtering = $derived(columns.some((c) => String(filters[c.key] ?? '').trim() !== ''))
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

  function clearAll() {
    filters = {}
    sortKey = null
    sortDir = 0
  }

  function keyOf(row, i) {
    if (rowKey) {
      const k = rowKey(row, i)
      return k == null ? i : k
    }
    return i
  }
</script>

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
                class="inline-flex cursor-pointer items-center gap-1 rounded-sm outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                onclick={() => toggleSort(col)}
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
        </Table.Row>
      {:else}
        <Table.Row>
          <Table.Cell colspan={columns.length} class="text-muted-foreground">
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
