<script>
  import { untrack } from 'svelte'
  import { api } from '$lib/api.js'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { StatusBadge } from '$lib/components/badges/index.js'
  import PageHeader from '$lib/components/PageHeader.svelte'
  import TrackRule from '$lib/components/TrackRule.svelte'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'
  import DataTable from '$lib/components/DataTable.svelte'
  import { humanBytes, fmtCompact } from '$lib/format.js'

  import TrendingUp from 'lucide-svelte/icons/trending-up'
  import Gauge from 'lucide-svelte/icons/gauge'
  import RefreshCw from 'lucide-svelte/icons/refresh-cw'
  import Sparkles from 'lucide-svelte/icons/sparkles'
  import ShieldCheck from 'lucide-svelte/icons/shield-check'
  import CircleCheck from 'lucide-svelte/icons/circle-check'
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert'
  import Hammer from 'lucide-svelte/icons/hammer'

  let data = $state({ phase: 'loading', data: null })
  let daemon = $state({ phase: 'loading', data: null })
  let busy = $state(false)
  let note = $state('')

  async function load() {
    busy = true
    const [res, dres] = await Promise.all([api('/data/kaizen.json'), api('/data/kaizen-status.json')])
    if (res.ok) {
      data = { phase: 'ok', data: res.data }
      note = ''
    } else {
      if (data?.data == null) data = { phase: 'error', data: null }
      note = `could not load /data/kaizen.json (${res.error})`
    }
    if (dres.ok) daemon = { phase: 'ok', data: dres.data }
    else daemon = { phase: 'error', data: null }
    busy = false
  }

  $effect(() => {
    untrack(() => load())
  })

  const toneChip = {
    go: 'bg-signal-go/10 text-signal-go-ink',
    hold: 'bg-signal-hold/10 text-signal-hold-ink',
    muted: 'bg-muted text-muted-foreground'
  }

  const MICRO_LABEL = 'text-xs font-semibold uppercase tracking-wider text-muted-foreground'
  const TILE_TITLE = 'text-xs font-medium uppercase tracking-wide text-muted-foreground'
  const TILE_VALUE = 'data-num text-lg font-semibold tabular-nums lg:text-2xl'
  const TILE_CARD = 'gap-1 py-2 lg:gap-2 lg:py-4'

  const DIM_LABEL = {
    'bundle/disk': 'Bundle / disk',
    'quality/code-health': 'Code health',
    'ux/a11y': 'UX / accessibility',
    reliability: 'Reliability',
    correctness: 'Correctness',
    'perf/throughput': 'Throughput',
    'perf/latency': 'Latency',
    'security/deps': 'Security / deps',
    dx: 'Developer experience'
  }

  const PICK_LABEL = {
    'stale-assets-prune': 'Stale JS bundle cleanup',
    'stale-embed-prune': 'Stale embed bundle cleanup',
    'fmt-drift': 'cargo fmt drift',
    'a11y-10px-font': 'Sub-12px font sizes',
    'a11y-11px-font': '11px font sizes',
    'unwrap-hardening': 'unwrap() hardening',
    'disk-hygiene': 'Workspace disk hygiene',
    'clippy-warnings': 'Clippy warnings',
    'svelte-import-audit': 'Svelte import audit',
    'todo-prune': 'TODO / FIXME cleanup',
    'cargo-audit-vuln': 'Cargo dependency audit',
    'npm-audit-vuln': 'npm dependency audit',
    'cache-hit-rate': 'Cache hit rate',
    'bundle-hash-mismatch': 'Bundle hash mismatch'
  }

  function dimLabel(d) {
    return DIM_LABEL[d] ?? (String(d || '').replace(/[-_]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()) || '—')
  }

  function pickLabel(id) {
    return PICK_LABEL[id] ?? (String(id || '').replace(/[-_]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase()) || '—')
  }

  function num(v) {
    const n = Number(v)
    return Number.isFinite(n) ? n : null
  }

  const runsNewest = $derived.by(() => {
    if (data.phase !== 'ok') return []
    const runs = Array.isArray(data.data?.runs) ? data.data.runs : []
    return [...runs].sort((a, b) => num(b.run) - num(a.run))
  })

  const totalBytesFreed = $derived(
    runsNewest.reduce((acc, r) => {
      const b = num(r.before?.bytes)
      const a = num(r.after?.bytes)
      if (b === null || a === null) return acc
      return acc + Math.max(0, b - a)
    }, 0)
  )

  const committedRuns = $derived(runsNewest.filter((r) => r.commit))

  const daemonStale = $derived.by(() => {
    if (daemon.phase !== 'ok' || data.phase !== 'ok') return null
    const dTs = Date.parse(daemon.data?.ts || '')
    const kTs = Date.parse(data.data?.updated_ts || runsNewest[0]?.ts || '')
    if (!Number.isFinite(dTs) || !Number.isFinite(kTs)) return null
    const diffMin = Math.round((dTs - kTs) / 60000)
    if (Math.abs(diffMin) > 5) return `${Math.abs(diffMin)} min ${diffMin > 0 ? 'ahead of' : 'behind'} data`
    return null
  })

  const heroTiles = $derived.by(() => {
    const latest = runsNewest[0]
    return [
      {
        key: 'runs',
        icon: CircleCheck,
        label: 'Improvements shipped',
        value: String(runsNewest.length),
        sub: runsNewest.length ? `every verified run with gates green` : 'no runs recorded yet',
        tone: 'go'
      },
      {
        key: 'bytes',
        icon: Gauge,
        label: 'Static bytes freed',
        value: totalBytesFreed > 0 ? humanBytes(totalBytesFreed) : '—',
        sub: 'accumulated across bundle/disk runs',
        tone: 'hold'
      },
      {
        key: 'llm',
        icon: Sparkles,
        label: 'LLM-proposed',
        value: String(runsNewest.filter((r) => r.source === 'llm').length),
        sub: 'researched by the LLM discovery phase',
        tone: 'muted'
      },
      {
        key: 'latest',
        icon: Hammer,
        label: 'Latest run',
        value: latest ? `#${latest.run}` : '—',
        sub: latest ? pickLabel(latest.pick) : 'run the kaizen loop to start',
        tone: 'go'
      }
    ]
  })

  function afterText(r) {
    const bA = num(r.before?.assets), aA = num(r.after?.assets)
    if (bA !== null && aA !== null && bA !== aA) return `${r.before.assets} → ${r.after.assets} bundles`
    const bB = num(r.before?.bytes), aB = num(r.after?.bytes)
    if (bB !== null && aB !== null && bB !== aB) return `${humanBytes(r.before.bytes)} → ${humanBytes(r.after.bytes)}`
    // no change in the primary metric — show the one that has data
    if (bA !== null && aA !== null) return `${aA} bundles`
    if (bB !== null && aB !== null) return humanBytes(aB)
    return '—'
  }

  function deltaTone(r) {
    const d = num(r.delta_pct)
    if (d === null || Math.abs(d) < 1) return 'muted'
    const isBundle = /bundle|disk/.test(String(r.dimension || ''))
    const good = isBundle ? d < 0 : d > 0
    return good ? 'go' : 'hold'
  }

  function deltaText(r, tone) {
    const d = num(r.delta_pct)
    if (d === null) return '—'
    const bundle = /bundle|disk/.test(String(r.dimension || ''))
    const pct = Math.abs(d).toFixed(1)
    if (bundle && d < 0) return `−${pct}%`
    if (!bundle && d > 0) return `+${pct}%`
    return `${d > 0 ? '+' : d < 0 ? '−' : ''}${pct}%`
  }

  function sourceLabel(r) {
    const s = String(r.source || '')
    if (s === 'llm') return 'LLM'
    if (s === 'deterministic') return 'deterministic'
    if (s === 'scan') return 'scan'
    return s || '—'
  }
  function gatesText(r) {
    const g = r.gates
    if (!g || typeof g !== 'object') return '—'
    const parts = []
    if (g.fmt) parts.push(`fmt:${g.fmt}`)
    if (g.clippy) parts.push(`clippy:${g.clippy}`)
    if (g.imports) parts.push(`imports:${g.imports}`)
    return parts.join(' · ') || '—'
  }
  const runCols = [
    { key: 'run', label: 'Run', class: 'w-14', cellClass: 'data-num text-xs', value: (r) => `#${r.run}`, sortValue: (r) => num(r.run) ?? 0 },
    { key: 'pick', label: 'What improved', cellClass: 'font-medium', value: (r) => pickLabel(r.pick), sortValue: (r) => String(r.pick) },
    { key: 'dimension', label: 'Dimension', class: 'w-32', value: (r) => dimLabel(r.dimension), sortValue: (r) => String(r.dimension) },
    { key: 'delta', label: 'Δ', class: 'w-20', cellClass: 'data-num text-xs', value: (r) => '', sortValue: (r) => num(r.delta_pct) ?? 0 },
    { key: 'after', label: 'Before → after', cellClass: 'data-num text-xs', value: afterText, sortValue: () => 0 },
    { key: 'source', label: 'Found by', class: 'w-20', value: sourceLabel, sortValue: (r) => (r.source === 'llm' ? 2 : r.source === 'deterministic' ? 1 : 0) },
    { key: 'gates', label: 'Gates', class: 'w-28', cellClass: 'text-xs', value: gatesText, sortValue: () => 0 },
    { key: 'date', label: 'Date', class: 'w-24', cellClass: 'data-num text-xs', value: (r) => String(r.ts || '').slice(0, 10) },
    { key: 'commit', label: 'Commit', class: 'w-20', cellClass: 'data-num font-mono text-xs', value: (r) => String(r.commit || '').slice(0, 7), sortValue: (r) => String(r.commit || '') }
  ]

  /* Keep long pick text from wrapping the delta column — pinch horizontally. */
  const subLabel = 'text-xs text-muted-foreground'
</script>

{#snippet deltaCell(r)}
  <span class={`inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 text-xs font-medium ${toneChip[deltaTone(r)] ?? toneChip.muted}`}>
    {deltaText(r, deltaTone(r))}
  </span>
{/snippet}

{#snippet sourceCell(r)}
  {#if r.source === 'llm'}
    <StatusBadge tone="info" dot>
      <span class="inline-flex items-center gap-1"><Sparkles class="size-3" />LLM</span>
    </StatusBadge>
  {:else if r.source === 'deterministic'}
    <StatusBadge tone="neutral">deterministic</StatusBadge>
  {:else}
    <StatusBadge tone="neutral">{sourceLabel(r)}</StatusBadge>
  {/if}
{/snippet}

<section class="grid gap-6">
  <PageHeader title="Improvements" description="Every kaizen run, measured and committed. Each run keeps coverage, fmt, clippy and imports green.">
    {#snippet children()}
      {#if note}
        <p class="text-xs text-destructive">{note}</p>
      {/if}
    {/snippet}
    {#snippet actions()}
      <Button type="button" variant="outline" size="sm" onclick={() => load()} disabled={busy}>
        <RefreshCw class={`mr-2 size-4${busy ? ' animate-spin' : ''}`} />
        {busy ? 'Refreshing…' : 'Refresh'}
      </Button>
    {/snippet}
  </PageHeader>

  <TrackRule />

  {#if daemon.phase === 'ok' && daemon.data}
    {@const rc = daemon.data.last_rc}
    {@const rcTone = rc === 0 ? 'bg-signal-go animate-pulse' : rc === 2 ? 'bg-signal-hold' : 'bg-destructive'}
    {@const rcText = rc === 0 ? 'committed' : rc === 2 ? 'no auto-fix' : `rc=${rc ?? '?'}`}
    <div class="flex flex-wrap items-center gap-2 rounded-lg border bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
      <span class="inline-flex items-center gap-1.5">
        <span class="size-2 rounded-full {rcTone}"></span>
        autonomous daemon — cycle {daemon.data.cycle ?? '—'} · {rcText}
      </span>
      <span class="hidden sm:inline">·</span>
      <span>last: {daemon.data.last_pick || '—'} {daemon.data.last_delta_pct ? `Δ ${daemon.data.last_delta_pct}%` : ''}</span>
      <span class="hidden sm:inline">·</span>
      <span>next in ~{Math.max(0, Math.round((daemon.data.next_cycle_secs || 3600) / 60))} min</span>
      <span class="hidden sm:inline">·</span>
      <span class="data-num">{daemon.data.runs_total ?? '—'} runs · {daemon.data.innovations_unique ?? '—'} unique</span>
      {#if daemon.data.ts}
        <span class="hidden sm:inline">·</span>
        <span class="data-num" title={daemon.data.ts}>{String(daemon.data.ts).slice(0,16).replace('T',' ')}</span>
      {/if}
      {#if daemonStale}
        <span class="inline-flex items-center gap-1 rounded bg-signal-hold/15 px-1.5 py-0.5 text-[11px] font-medium text-signal-hold-ink">⚠ data {daemonStale}</span>
      {/if}
    </div>
  {/if}

  <div class="grid gap-2">
    <h2 class={MICRO_LABEL}>Track record</h2>
    {#if data.phase === 'loading'}
      <div class="grid grid-cols-2 gap-3 lg:grid-cols-4" aria-busy="true">
        {#each Array.from({ length: 4 }, (_, i) => i) as i (i)}
          <Skeleton class="h-24" />
        {/each}
      </div>
    {:else if data.phase === 'error'}
      <Alert.Root variant="destructive">
        <Alert.Title>Improvement history unavailable</Alert.Title>
        <Alert.Description>Could not load /data/kaizen.json. It is created by the kaizen loop on the next run.</Alert.Description>
      </Alert.Root>
    {:else if runsNewest.length === 0}
      <Alert.Root>
        <Alert.Title>No runs yet</Alert.Title>
        <Alert.Description>Beat a measurable metric by at least 1% without degrading others; this page then records what exactly improved and by how much.</Alert.Description>
      </Alert.Root>
    {:else}
      <div class="grid grid-cols-2 gap-2 lg:grid-cols-4 lg:gap-3">
        {#each heroTiles as t (t.key)}
          <Card.Root class={TILE_CARD}>
            <Card.Content class="flex items-start justify-between gap-2 px-3 lg:px-4">
              <div class="grid gap-0.5 lg:gap-1">
                <Card.Title class={TILE_TITLE}>{t.label}</Card.Title>
                <Card.Description class={TILE_VALUE}>{t.value}</Card.Description>
              </div>
              <span class={`rounded-md p-1.5 lg:p-2 ${toneChip[t.tone] ?? toneChip.muted}`} aria-hidden="true">
                <t.icon class="size-3.5 lg:size-4" />
              </span>
            </Card.Content>
            <Card.Content class="px-3 lg:px-4">
              <p class="text-xs leading-tight text-muted-foreground">{t.sub}</p>
            </Card.Content>
          </Card.Root>
        {/each}
      </div>
    {/if}
  </div>

  <div class="grid gap-2">
    <h2 class={MICRO_LABEL}>Run history</h2>
    {#if data.phase === 'ok' && runsNewest.length > 0}
      <Card.Root>
        <Card.Header class="max-lg:p-3">
          <Card.Title class="text-sm lg:text-base">What has improved — newest first</Card.Title>
          <Card.Description class="max-lg:text-xs">
            {committedRuns.length} committed · <span class="data-num">{fmtCompact(totalBytesFreed)}</span> static bytes freed
            {#if runsNewest[0]?.ts}
              · last updated <span class="data-num">{String(runsNewest[0].ts).slice(0, 10)}</span>
            {/if}
          </Card.Description>
        </Card.Header>
        <Card.Content class="max-lg:p-3">
          <DataTable
            columns={runCols}
            rows={runsNewest}
            primary="pick"
            rowKey={(r) => `run-${r.run}`}
            cells={{ delta: deltaCell, source: sourceCell }}
            empty="No improvement runs yet."
          />
          <p class="mt-3 flex items-start gap-1.5 text-xs text-muted-foreground">
            <ShieldCheck class="mt-0.5 size-3.5 shrink-0 text-signal-go-ink" />
            Each run records the exact pick, measured before → after delta, quality gates, and the commit that shipped it.
            Data is regenerated from <code class="font-mono">.agents/skills/kaizen/ledger.json</code> on every run
            ({runsNewest.filter((r) => r.source === 'llm').length} were proposed by the LLM research phase).
          </p>
        </Card.Content>
      </Card.Root>
    {/if}

    {#if data.phase === 'ok' && runsNewest.length > 0}
      <div class="grid gap-2 lg:grid-cols-2 lg:gap-3">
        <Card.Root>
          <Card.Header class="max-lg:p-3">
            <Card.Title class="flex items-center gap-1.5 text-sm lg:text-base">
              <TrendingUp class="size-4 text-chart-3" /> Method
            </Card.Title>
            <Card.Description class="max-lg:text-xs">The loop behind these rows.</Card.Description>
          </Card.Header>
          <Card.Content class="max-lg:p-3">
            <ol class="grid gap-2 text-xs text-muted-foreground lg:text-sm">
              <li class="flex gap-2"><span class="data-num text-foreground">1</span> Baseline the quality gates (fmt, clippy, imports) and snapshot dirty files</li>
              <li class="flex gap-2"><span class="data-num text-foreground">2</span> Discover candidates — deterministic probes plus, in research mode, an LLM proposing new aspects</li>
              <li class="flex gap-2"><span class="data-num text-foreground">3</span> Prove the winner with a real command (no fabricated numbers) and measure before</li>
              <li class="flex gap-2"><span class="data-num text-foreground">4</span> Implement the smallest fix, verify no gate regressed and the metric moved ≥ 1%</li>
              <li class="flex gap-2"><span class="data-num text-foreground">5</span> Record this row and commit only the changed files</li>
            </ol>
          </Card.Content>
        </Card.Root>

        <Card.Root>
          <Card.Header class="max-lg:p-3">
            <Card.Title class="flex items-center gap-1.5 text-sm lg:text-base">
              <TriangleAlert class="size-4 text-signal-hold-ink" /> Guardrail
            </Card.Title>
            <Card.Description class="max-lg:text-xs">Nothing ships without clearing the bar.</Card.Description>
          </Card.Header>
          <Card.Content class="max-lg:p-3">
            <ul class="grid gap-2 text-xs text-muted-foreground lg:text-sm">
              <li class="flex gap-2"><span class="text-signal-go-ink">post gates ≥ pre gates</span><span class="data-num">fmt · clippy · svelte imports</span></li>
              <li class="flex gap-2">measured improvement <span class="data-num">≥ 1%</span> on exactly one dimension</li>
              <li class="flex gap-2">no other dimension degrades — verified, else the run aborts and reverts its own files only</li>
            </ul>
            <p class={subLabel}>Full policy lives in <code class="font-mono">.agents/skills/kaizen/SKILL.md</code>.</p>
          </Card.Content>
        </Card.Root>
      </div>
    {/if}
  </div>
</section>
<BottomSpacer />