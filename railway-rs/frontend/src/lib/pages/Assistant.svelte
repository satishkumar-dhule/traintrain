<script>
  import { untrack } from 'svelte'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import BotMessageSquareIcon from 'lucide-svelte/icons/bot-message-square'
  import SendHorizontalIcon from 'lucide-svelte/icons/send-horizontal'
  import { renderMarkdown } from '$lib/markdown.js'
  import { viewport } from '$lib/media.svelte.js'
  import ToolCard from '$lib/components/chat/ToolCards.svelte'
  import { classify, executePlan, PROJECTORS, nextActionsFor } from '$lib/chat/gate.js'
  import { createMemory, remember, compact } from '$lib/chat/memory.js'

  const HISTORY_KEY = 'rc-assistant-history'
  const TIMEOUT_MS = 12000
  const MAX_TURNS = 20

  // Zero-network starter chips for greetings/help.
  const DEFAULT_CHIPS = [
    { label: '12951 today', prompt: 'live status of 12951' },
    { label: 'SC→PUNE', prompt: 'trains from secunderabad to pune' }
  ]

  const sessionMemory = createMemory()

  // Live answers go stale; only replay them briefly. Static kinds replay all session.
  const REPLAY_TTL_MS = { live_status: 30_000, station_board: 30_000 }

  // One-line lead-in above a locally-served card; details live in the card.
  function templateFor(kind, d = {}) {
    switch (kind) {
      case 'live_status':
        return `**${d.train_name || 'Train ' + (d.train_number ?? '')}** — here's where it is right now:`
      case 'trains_between':
        return `Found **${d.total_found ?? d.trains?.length ?? 0} trains** ${d.from ?? ''} → ${d.to ?? ''}:`
      case 'average_delay':
        return `Worst delays on **${d.train_no ?? ''} ${d.train_name ?? ''}**:`
      case 'train_schedule':
        return `Route of **${d.train_number ?? ''} ${d.train_name ?? ''}** (${d.total_stops ?? '?'} stops):`
      case 'station_board':
        return `Next arrivals at **${d.station_code ?? ''}** (next ${d.hours ?? 2}h):`
      default:
        return 'Here’s what I found:'
    }
  }

  let { seed = '' } = $props()

  function loadHistory() {
    try {
      const raw = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? '[]')
      if (!Array.isArray(raw)) return []
      return raw
        .filter(
          (t) =>
            t &&
            (t.role === 'user' || t.role === 'assistant') &&
            typeof t.content === 'string' &&
            t.content.length > 0
        )
        .slice(-MAX_TURNS)
    } catch {
      return []
    }
  }

  let phase = $state('boot') // boot | degraded | statusError | ready
  let model = $state('')
  let statusError = $state(null)
  let turns = $state(untrack(loadHistory))
  let draft = $state(seed)
  let streaming = $state(false)
  let streamError = $state(null)
  let scroller = $state(null)

  const canSend = $derived(
    !streaming && (phase === 'ready' || phase === 'degraded') && draft.trim().length > 0
  )

  function persist() {
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(turns.slice(-MAX_TURNS)))
    } catch {}
  }

  function clearChat() {
    if (streaming) return
    turns = []
    streamError = null
    try {
      localStorage.removeItem(HISTORY_KEY)
    } catch {}
  }

  $effect(() => {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), TIMEOUT_MS)
    fetch('/rail-api/ai/status', { signal: controller.signal })
      .then(async (res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        const body = await res.json()
        model = String(body?.model ?? '')
        phase = body?.enabled ? 'ready' : 'degraded'
      })
      .catch((err) => {
        phase = 'statusError'
        statusError =
          err && err.name === 'AbortError'
            ? `Request timed out after ${TIMEOUT_MS}ms`
            : err && err.message
              ? err.message
              : String(err)
      })
      .finally(() => clearTimeout(timer))
    return () => clearTimeout(timer)
  })

  $effect(() => {
    void turns.length
    void (turns[turns.length - 1]?.content.length ?? 0)
    untrack(() => {
      if (!scroller) return
      scroller.scrollTo({ top: scroller.scrollHeight })
    })
  })

  // One `data:` SSE frame -> live update of the in-progress assistant turn.
  function handleFrame(frame) {
    const line = frame.split('\n').find((l) => l.startsWith('data:'))
    if (!line) return
    let evt
    try {
      evt = JSON.parse(line.slice(5).trim())
    } catch {
      return
    }
    const last = turns[turns.length - 1]
    if (!last || last.role !== 'assistant') return
    if (evt.type === 'delta') {
      last.content += evt.text ?? ''
    } else if (evt.type === 'reasoning') {
      last.reasoning = (last.reasoning ?? '') + (evt.text ?? '')
    } else if (evt.type === 'tools') {
      last.tools = [...(last.tools ?? []), ...(evt.names ?? [])]
    } else if (evt.type === 'card') {
      if (evt.kind) last.cards = [...(last.cards ?? []), { kind: evt.kind, data: evt.data ?? {} }]
    } else if (evt.type === 'actions') {
      last.actions = (evt.items ?? []).filter((a) => a?.label && a?.prompt)
    } else if (evt.type === 'done') {
      last.tokens = evt.completion_tokens ?? 0
    } else if (evt.type === 'error') {
      streamError = evt.message || evt.error || 'The assistant hit an upstream error.'
      if (!last.content && !last.reasoning) turns.pop()
    }
  }

  async function sendText(text) {
    text = (text ?? '').trim()
    if (!text || streaming || (phase !== 'ready' && phase !== 'degraded')) return

    // Local-first gate: serve what we can without the LLM.
    const verdict = classify(text, sessionMemory)
    if (verdict.kind === 'trivial') {
      pushLocal(text, verdict.reply, { actions: DEFAULT_CHIPS })
      return
    }
    if (verdict.kind === 'replay') {
      const a = verdict.entry.answer
      pushLocal(text, `*(answered earlier)* ${a.content}`, {
        cards: a.cards ?? [],
        actions: a.actions ?? []
      })
      return
    }
    if (verdict.kind === 'tool') {
      streamError = null
      draft = ''
      streaming = true
      let toolFailure = null
      try {
        const dto = await executePlan(verdict.plan, (u) => fetch(u))
        const data = PROJECTORS[verdict.plan.cardKind](dto)
        // Build the turn COMPLETELY before pushing: mutating a raw object
        // after push() bypasses the $state proxy and the UI never sees it.
        const turn = {
          role: 'assistant',
          content: templateFor(verdict.plan.cardKind, data),
          reasoning: '',
          tokens: null,
          tools: [verdict.plan.cardKind],
          cards: [{ kind: verdict.plan.cardKind, data }],
          actions: nextActionsFor(verdict.plan.cardKind, data)
        }
        turns.push({ role: 'user', content: text })
        turns.push(turn)
        remember(sessionMemory, text, { content: turn.content, cards: turn.cards, actions: turn.actions }, {
          ttlMs: REPLAY_TTL_MS[verdict.plan.cardKind] ?? Infinity
        })
        persist()
        return
      } catch (e) {
        // Tool path failed (unresolved station, upstream 5xx) -> LLM fallback,
        // or an inline error when the LLM itself is off.
        toolFailure = e
      } finally {
        // Success clears it before returning; on fallback streamLlm re-sets it.
        streaming = false
      }
      if (toolFailure && phase !== 'ready') {
        streamError = toolFailure?.message ? toolFailure.message : String(toolFailure)
        return
      }
    }
    if (phase !== 'ready') {
      streamError = 'AI is disabled'
      return
    }
    await streamLlm(text)
  }

  function pushLocal(userText, assistantText, extras = {}) {
    streamError = null
    draft = ''
    turns.push({ role: 'user', content: userText })
    turns.push({
      role: 'assistant',
      content: assistantText,
      reasoning: '',
      tokens: null,
      tools: [],
      cards: extras.cards ?? [],
      actions: extras.actions ?? []
    })
    persist()
  }

  // The only path that talks to /ai/chat. History is auto-compacted so long
  // sessions stay under the server's caps with tiny payloads.
  async function streamLlm(text) {
    streamError = null
    draft = ''
    const { messages } = compact([...turns, { role: 'user', content: text }])
    turns.push({ role: 'user', content: text })
    turns.push({ role: 'assistant', content: '', reasoning: '', tokens: null, tools: [], cards: [], actions: [] })
    streaming = true
    try {
      const res = await fetch('/rail-api/ai/chat', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ messages }),
      })
      if (!res.ok) {
        let msg = `HTTP ${res.status}`
        try {
          const body = await res.json()
          if (body && typeof body.error === 'string') msg = body.error
        } catch {}
        throw new Error(msg)
      }
      if (!res.body) throw new Error('Streaming responses are not supported here.')
      const reader = res.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ''
      for (;;) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })
        let sep
        while ((sep = buffer.indexOf('\n\n')) !== -1) {
          handleFrame(buffer.slice(0, sep))
          buffer = buffer.slice(sep + 2)
        }
      }
    } catch (err) {
      streamError = err && err.message ? err.message : String(err)
      const last = turns[turns.length - 1]
      if (last?.role === 'assistant' && !last.content && !last.reasoning) turns.pop()
    } finally {
      streaming = false
      persist()
    }
  }

  function onKeydown(e) {
    if (e.key === 'Enter' && !e.shiftKey && !e.defaultPrevented) {
      e.preventDefault()
      sendText(draft)
    }
  }
</script>

<section class="grid gap-6 max-lg:gap-3 max-lg:h-[calc(100dvh-4rem)] max-lg:grid-rows-[auto_1fr]">
  {#if !viewport.narrow}
    <div class="grid gap-1">
      <h1 class="text-2xl font-semibold tracking-tight">Ask Train Bro</h1>
      <p class="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
        Live AI answers about Indian Railways, powered by
        <Badge variant="secondary" class="font-mono">{model}</Badge>
      </p>
    </div>
  {/if}

  {#if phase === 'statusError'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not reach the AI status endpoint</Alert.Title>
      <Alert.Description>{statusError}</Alert.Description>
    </Alert.Root>
  {:else}
    {#if phase === 'degraded'}
      <Alert.Root>
        <BotMessageSquareIcon class="size-4" />
        <Alert.Title>AI answers unavailable — live-data lookups still work</Alert.Title>
        <Alert.Description>
          This server runs without the AI feature (RAILWAY_AI_ENABLED=0), so free-form answers are
          off. Live status, routes, boards and delay lookups keep working below.
        </Alert.Description>
      </Alert.Root>
    {/if}
    <Card.Root class="min-w-0 overflow-hidden max-lg:h-full max-lg:flex max-lg:flex-col">
      <Card.Content class="grid min-h-0 gap-3 max-lg:h-full max-lg:min-h-0 max-lg:gap-2">
        {#if phase === 'boot'}
          <div class="grid gap-2" aria-busy="true">
            {#each [0, 1, 2] as i (i)}
              <Skeleton class="h-12 w-full" />
            {/each}
          </div>
        {:else}
          <div bind:this={scroller} class="grid max-h-[55vh] max-h-[50dvh] min-h-40 gap-3 overflow-y-auto overscroll-contain pr-1 max-lg:max-h-none max-lg:min-h-0 max-lg:flex-1">
            {#each turns as t, i (i)}
              <div class={t.role === 'user' ? 'flex justify-end' : 'flex justify-start min-w-0'}>
                <div
                  class="max-w-[85%] min-w-0 overflow-hidden rounded-lg border px-3 py-2 text-sm leading-relaxed max-lg:max-w-[92%] max-lg:px-2 max-lg:py-1.5 max-lg:text-[13px] {t.role ===
                  'user'
                    ? 'bg-muted'
                    : ''}"
                >
                  {#if t.tools?.length}
                    <div class="mb-1.5 flex flex-wrap gap-1">
                      {#each t.tools as name}
                        <span
                          class="rounded-full border bg-muted px-2 py-0.5 text-[11px] text-muted-foreground"
                        >checked live data: {name}</span>
                      {/each}
                    </div>
                  {/if}
                  {#if t.reasoning}
                    <details class="mb-1.5 text-xs text-muted-foreground">
                      <summary class="cursor-pointer select-none">Thinking…</summary>
                      <p class="mt-1 whitespace-pre-wrap break-words">{t.reasoning}</p>
                    </details>
                  {/if}
                  {#if t.role === 'user'}
                    <p class="whitespace-pre-wrap break-words [overflow-wrap:anywhere]">{t.content}</p>
                  {:else}
                    <div class="md text-sm leading-relaxed break-words min-w-0 overflow-hidden [overflow-wrap:anywhere] [&_pre]:max-w-full [&_pre]:overflow-x-auto [&_table]:block [&_table]:max-w-full [&_table]:overflow-x-auto [&_code]:break-all [&_a]:break-all">
                      {@html renderMarkdown(t.content)}{#if streaming && i === turns.length - 1}<span
                          class="animate-pulse">▍</span
                        >{/if}
                    </div>
                  {/if}
                  {#if t.cards?.length}
                    <div class="mt-2 grid gap-2">
                      {#each t.cards as c, ci (ci)}
                        <ToolCard kind={c.kind} data={c.data} />
                      {/each}
                    </div>
                  {/if}
                  {#if t.actions?.length && !(streaming && i === turns.length - 1)}
                    <div class="mt-2 flex flex-wrap gap-1.5" data-testid="next-actions">
                      {#each t.actions as a (a.label)}
                        <button
                          type="button"
                          class="rounded-full border bg-muted/60 px-2.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground max-lg:min-h-11 max-lg:items-center max-lg:px-4 max-lg:inline-flex"
                          onclick={() => sendText(a.prompt)}
                        >{a.label}</button>
                      {/each}
                    </div>
                  {/if}
                  {#if typeof t.tokens === 'number'}
                    <p class="mt-1 text-xs text-muted-foreground">{model} · {t.tokens} tokens</p>
                  {/if}
                </div>
              </div>
            {:else}
              <div
                class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
              >
                Ask anything about Indian Railways — trains, stations, tickets.
              </div>
              <div class="flex flex-wrap justify-center gap-1.5" data-testid="starter-chips">
                {#each DEFAULT_CHIPS as c (c.label)}
                  <button
                    type="button"
                    class="rounded-full border bg-muted/60 px-2.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground max-lg:min-h-11 max-lg:items-center max-lg:px-4 max-lg:inline-flex"
                    onclick={() => sendText(c.prompt)}
                  >{c.label}</button>
                {/each}
              </div>
            {/each}
          </div>

          {#if streamError}
            <Alert.Root variant="destructive" role="alert">
              <Alert.Title>The assistant could not finish this reply</Alert.Title>
              <Alert.Description>{streamError}</Alert.Description>
            </Alert.Root>
          {/if}

          <div class="sticky bottom-0 grid gap-2 bg-card max-lg:p-2 max-lg:border-t max-lg:-mx-3 max-lg:-mb-3 max-lg:px-4">
            <Textarea
              bind:value={draft}
              placeholder={
                phase === 'ready' || phase === 'degraded'
                  ? 'Ask about trains, PNR rules, stations…'
                  : 'Waiting for AI…'
              }
              rows={2}
              class="min-h-11 max-lg:min-h-[40px] max-lg:text-sm"
              disabled={streaming || (phase !== 'ready' && phase !== 'degraded')}
              onkeydown={onKeydown}
            ></Textarea>
            <div class="flex flex-wrap items-center justify-between gap-2">
              <Button
                type="button"
                variant="ghost"
                size="xs"
                class="max-lg:h-9 max-lg:px-3 max-lg:text-xs"
                onclick={clearChat}
                disabled={streaming || turns.length === 0}
              >
                Clear chat
              </Button>
              <Button type="button" size="sm" class="max-lg:h-9 max-lg:px-3 max-lg:text-xs shrink-0" onclick={() => sendText(draft)} disabled={!canSend}>
                <SendHorizontalIcon class="size-4 max-lg:size-3.5" />
                {streaming ? 'Answering…' : 'Send'}
              </Button>
            </div>
          </div>
        {/if}
      </Card.Content>
    </Card.Root>
  {/if}
</section>
<div class="h-20 lg:hidden"></div>
