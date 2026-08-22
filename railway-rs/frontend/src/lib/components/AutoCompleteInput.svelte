<script>
  import { api } from '$lib/api.js'
  import { Badge } from '$lib/components/ui/badge/index.js'

  let {
    id = null,
    placeholder = 'Search…',
    kind = 'any',
    value = $bindable(''),
    onpick = () => {},
    inputClass = ''
  } = $props()

  let items = $state([])
  let open = $state(false)
  let active = $state(-1)
  let pos = $state(null)
  let controller = null
  let timer = null
  let rootEl = $state(null)
  let listEl = $state(null)
  const uid = `ac-${Math.random().toString(36).slice(2, 9)}`

  const filtered = $derived(
    items.filter((r) => {
      if (kind === 'any') return true
      return r.type === (kind === 'station' ? 'station' : 'train')
    })
  )

  function abort() {
    if (controller) {
      controller.abort()
      controller = null
    }
  }

  async function lookup(q) {
    abort()
    const c = new AbortController()
    controller = c
    const res = await api(`/rail-api/search/suggest?q=${encodeURIComponent(q)}`)
    if (c.signal.aborted) return
    controller = null
    if (res.ok && Array.isArray(res.data)) {
      items = res.data
      active = filtered.length > 0 ? 0 : -1
      open = true
    } else {
      items = []
      open = false
    }
  }

  function onInput() {
    clearTimeout(timer)
    const q = value.trim()
    if (!q) {
      abort()
      items = []
      open = false
      return
    }
    timer = setTimeout(() => lookup(q), 180)
  }

  function pick(item) {
    open = false
    abort()
    clearTimeout(timer)
    value =
      item.type === 'station'
        ? `${item.code}`
        : item.type === 'train'
          ? `${item.number}`
          : item.name
    onpick(item)
  }

  function move(delta) {
    if (!open || filtered.length === 0) return
    active = (active + delta + filtered.length) % filtered.length
  }

  function onKeydown(e) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      move(1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      move(-1)
    } else if (e.key === 'Enter') {
      if (open && filtered.length > 0) {
        e.preventDefault()
        pick(filtered[active] ?? filtered[0])
      }
    } else if (e.key === 'Escape') {
      open = false
    }
  }

  function place() {
    if (!rootEl || !open) return
    const r = rootEl.getBoundingClientRect()
    const gap = 4
    const margin = 8
    const spaceBelow = window.innerHeight - r.bottom - margin
    const spaceAbove = r.top - margin
    const flip = spaceBelow < 96 && spaceAbove > spaceBelow
    const avail = flip ? spaceAbove - gap : spaceBelow - gap
    pos = flip
      ? {
          left: r.left,
          width: r.width,
          bottom: window.innerHeight - r.top + gap,
          maxHeight: Math.max(120, Math.min(288, avail))
        }
      : {
          left: r.left,
          width: r.width,
          top: r.bottom + gap,
          maxHeight: Math.max(120, Math.min(288, avail))
        }
  }

  function onDocumentClick(e) {
    const t = e.target
    if (rootEl && !rootEl.contains(t) && !(listEl && listEl.contains(t))) open = false
  }

  function portal(node) {
    document.body.appendChild(node)
    return {
      destroy() {
        node.remove()
      }
    }
  }

  $effect(() => {
    document.addEventListener('click', onDocumentClick)
    return () => document.removeEventListener('click', onDocumentClick)
  })

  $effect(() => {
    if (!open) return
    place()
    window.addEventListener('resize', place)
    window.addEventListener('scroll', place, true)
    return () => {
      window.removeEventListener('resize', place)
      window.removeEventListener('scroll', place, true)
    }
  })

  $effect(() => {
    return () => {
      abort()
      clearTimeout(timer)
    }
  })
</script>

<div class="relative" bind:this={rootEl}>
  <input
    {id}
    bind:value
    oninput={onInput}
    onfocus={onInput}
    onkeydown={onKeydown}
    {placeholder}
    autocomplete="off"
    class={`flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-xs transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 ${inputClass}`}
    aria-autocomplete="list"
    aria-expanded={open}
    aria-controls={open && filtered.length > 0 ? `${uid}-list` : undefined}
    role="combobox"
  />
</div>

{#if open && filtered.length > 0 && pos}
  <div
    use:portal
    bind:this={listEl}
    id={`${uid}-list`}
    role="listbox"
    style={`left:${pos.left}px;width:${pos.width}px;max-height:${pos.maxHeight}px;${
      pos.top !== undefined ? `top:${pos.top}px` : `bottom:${pos.bottom}px`
    }`}
    class="fixed z-50 overflow-y-auto rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
  >
    {#each filtered as r, i ((r.type ?? '') + (r.code ?? '') + (r.number ?? ''))}
      <button
        type="button"
        role="option"
        aria-selected={i === active}
        class={`flex w-full items-center justify-between gap-2 rounded-sm px-2 py-1.5 text-left text-sm ${
          i === active ? 'bg-accent text-accent-foreground' : ''
        }`}
        onmousedown={(e) => {
          e.preventDefault()
          pick(r)
        }}
        onmouseenter={() => (active = i)}
      >
        <span class="truncate">{r.name}</span>
        {#if r.code}<Badge variant="outline" class="shrink-0 font-mono">{r.code}</Badge>
        {:else if r.number}<Badge variant="outline" class="shrink-0 font-mono">{r.number}</Badge>{/if}
      </button>
    {/each}
  </div>
{/if}
