<script>
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Input } from '$lib/components/ui/input/index.js'
  import { Label } from '$lib/components/ui/label/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import * as Tabs from '$lib/components/ui/tabs/index.js'
  import { DataSourceBadge } from '$lib/components/badges/index.js'
  import ActivityIcon from 'lucide-svelte/icons/activity'
  import ChartColumnIcon from 'lucide-svelte/icons/chart-column'
  import RouteIcon from 'lucide-svelte/icons/route'
  import { renderMarkdown } from '$lib/markdown.js'
  import SparklesIcon from 'lucide-svelte/icons/sparkles'

  // LLM completions legitimately run long; the backend serves this route
  // outside the global 30s timeout, so the client waits generously too.
  const TIMEOUT_MS = 120000

  const KINDS = [
    { value: 'live_status', label: 'Live Status', icon: ActivityIcon },
    { value: 'average_delay', label: 'Average Delay', icon: ChartColumnIcon },
    { value: 'trains_between', label: 'Trains Between', icon: RouteIcon },
  ]

  let { kind = '', train = '', src = '', dst = '' } = $props()

  function isKnownKind(k) {
    return KINDS.some((x) => x.value === k)
  }

  let selected = $state(isKnownKind(kind) ? kind : 'live_status')
  let trainInput = $state(String(train))
  let srcInput = $state(String(src))
  let dstInput = $state(String(dst))

  let phase = $state('idle') // idle | loading | ok | error
  let result = $state(null)
  let errorMsg = $state(null)

  function asText(v) {
    return String(v ?? '').trim()
  }

  function normCode(v) {
    return asText(v).toUpperCase()
  }

  // Mirror the backend contract exactly.
  function trainOk(v) {
    const t = asText(v)
    return /^\d{5}$/.test(t) && t !== '00000'
  }

  function codeOk(v) {
    return /^[A-Z0-9]{2,5}$/.test(normCode(v))
  }

  const trainValid = $derived(trainOk(trainInput))
  const pairValid = $derived(
    codeOk(srcInput) && codeOk(dstInput) && normCode(srcInput) !== normCode(dstInput)
  )
  const valid = $derived(selected === 'trains_between' ? pairValid : trainValid)
  const busy = $derived(phase === 'loading')

  function resetResult() {
    result = null
    errorMsg = null
    if (phase !== 'loading') phase = 'idle'
  }

  async function explain() {
    if (!valid || busy) return
    result = null
    errorMsg = null
    phase = 'loading'
    const params =
      selected === 'trains_between'
        ? { src: normCode(srcInput), dst: normCode(dstInput) }
        : { train: asText(trainInput) }
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), TIMEOUT_MS)
    try {
      const res = await fetch('/rail-api/ai/insight', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ kind: selected, params }),
        signal: controller.signal,
      })
      const body = await res.json().catch(() => null)
      if (!res.ok) {
        errorMsg =
          body && typeof body === 'object' && body.error ? body.error : `HTTP ${res.status}`
        phase = 'error'
      } else {
        result = body
        phase = 'ok'
      }
    } catch (err) {
      errorMsg =
        err && err.name === 'AbortError'
          ? `Request timed out after ${TIMEOUT_MS / 1000}s. The model may be busy - try again.`
          : err && err.message
            ? err.message
            : String(err)
      phase = 'error'
    } finally {
      clearTimeout(timer)
    }
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Insights</h1>
    <p class="text-sm text-muted-foreground">
      Plain-language explanations grounded in live rail data
    </p>
  </div>

  <Card.Root>
    <Card.Content class="grid gap-4">
      <Tabs.Root bind:value={selected} onValueChange={resetResult}>
        <Tabs.List class="w-full justify-start">
          {#each KINDS as k (k.value)}
            <Tabs.Trigger value={k.value}>
              <k.icon class="mr-2 size-4" />{k.label}
            </Tabs.Trigger>
          {/each}
        </Tabs.List>
      </Tabs.Root>

      {#if selected === 'trains_between'}
        <div class="flex flex-wrap items-end gap-3">
          <div class="grid min-w-32 flex-1 gap-2 sm:max-w-44">
            <Label for="ins-src">From station</Label>
            <Input
              id="ins-src"
              bind:value={srcInput}
              placeholder="e.g. NDLS"
              oninput={resetResult}
              onkeydown={(e) => e.key === 'Enter' && !e.defaultPrevented && explain()}
            />
          </div>
          <div class="grid min-w-32 flex-1 gap-2 sm:max-w-44">
            <Label for="ins-dst">To station</Label>
            <Input
              id="ins-dst"
              bind:value={dstInput}
              placeholder="e.g. MMCT"
              oninput={resetResult}
              onkeydown={(e) => e.key === 'Enter' && !e.defaultPrevented && explain()}
            />
          </div>
        </div>
        {#if (srcInput || dstInput) && !pairValid}
          <p class="text-xs text-muted-foreground">
            Two different station codes (2-5 letters/digits) are required.
          </p>
        {/if}
      {:else}
        <div class="flex flex-wrap items-end gap-3">
          <div class="grid min-w-48 flex-1 gap-2">
            <Label for="ins-train">Train number</Label>
            <Input
              id="ins-train"
              bind:value={trainInput}
              placeholder="e.g. 12951"
              inputmode="numeric"
              maxlength={5}
              oninput={resetResult}
              onkeydown={(e) => e.key === 'Enter' && !e.defaultPrevented && explain()}
            />
          </div>
        </div>
        {#if trainInput && !trainValid}
          <p class="text-xs text-muted-foreground">Train must be exactly 5 digits.</p>
        {/if}
      {/if}

      <div class="flex items-center gap-3">
        <Button type="button" onclick={explain} disabled={!valid || busy}>
          <SparklesIcon class="mr-2 size-4" />
          {busy ? 'Thinking…' : 'Explain'}
        </Button>
        <span class="text-xs text-muted-foreground">Answers come only from live upstream data.</span>
      </div>
    </Card.Content>
  </Card.Root>

  {#if phase === 'loading'}
    <div class="grid gap-2" aria-busy="true">
      <Skeleton class="h-6 w-52" />
      {#each [0, 1, 2] as i (i)}
        <Skeleton class="h-12 w-full" />
      {/each}
    </div>
  {:else if phase === 'error'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not generate an insight</Alert.Title>
      <Alert.Description>{errorMsg}</Alert.Description>
    </Alert.Root>
  {:else if result}
    <Card.Root>
      <Card.Header class="gap-3 space-y-0">
        <Card.Title>Summary</Card.Title>
        <div class="flex flex-wrap items-center gap-1.5">
          <DataSourceBadge
            source={result.data_source}
            cached={result.cached}
            freshness={result.cached ? '' : 'fresh'}
          />
          {#if result.model}<Badge variant="secondary">{result.model}</Badge>{/if}
        </div>
      </Card.Header>
      <Card.Content>
        <div class="md text-sm leading-relaxed break-words">{@html renderMarkdown(result.summary)}</div>
      </Card.Content>
    </Card.Root>
  {/if}
</section>
