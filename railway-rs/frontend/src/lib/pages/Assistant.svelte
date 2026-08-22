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

  const HISTORY_KEY = 'rc-assistant-history'
  const TIMEOUT_MS = 12000
  const MAX_TURNS = 20

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

  let phase = $state('boot') // boot | disabled | statusError | ready
  let model = $state('')
  let statusError = $state(null)
  let turns = $state(untrack(loadHistory))
  let draft = $state(seed)
  let streaming = $state(false)
  let streamError = $state(null)
  let scroller = $state(null)

  const canSend = $derived(!streaming && phase === 'ready' && draft.trim().length > 0)

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
        phase = body?.enabled ? 'ready' : 'disabled'
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
    } else if (evt.type === 'done') {
      last.tokens = evt.completion_tokens ?? 0
    } else if (evt.type === 'error') {
      streamError = evt.message || evt.error || 'The assistant hit an upstream error.'
      if (!last.content && !last.reasoning) turns.pop()
    }
  }

  async function send() {
    const text = draft.trim()
    if (!text || streaming || phase !== 'ready') return
    streamError = null
    draft = ''
    const history = [...turns, { role: 'user', content: text }].map((t) => ({
      role: t.role,
      content: t.content,
    }))
    turns.push({ role: 'user', content: text })
    turns.push({ role: 'assistant', content: '', reasoning: '', tokens: null, tools: [] })
    streaming = true
    try {
      const res = await fetch('/rail-api/ai/chat', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ messages: history }),
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
      send()
    }
  }
</script>

<section class="grid gap-6">
  <div class="grid gap-1">
    <h1 class="text-2xl font-semibold tracking-tight">Ask Train Bro</h1>
    <p class="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
      Live AI answers about Indian Railways, powered by
      <Badge variant="secondary" class="font-mono">{model}</Badge>
    </p>
  </div>

  {#if phase === 'disabled'}
    <Alert.Root>
      <BotMessageSquareIcon class="size-4" />
      <Alert.Title>AI assistant is disabled</Alert.Title>
      <Alert.Description>
        This server runs without the AI feature. An operator must set RAILWAY_AI_ENABLED=1
        (optionally RAILWAY_AI_BASE / RAILWAY_AI_MODEL) to enable it.
      </Alert.Description>
    </Alert.Root>
  {:else if phase === 'statusError'}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not reach the AI status endpoint</Alert.Title>
      <Alert.Description>{statusError}</Alert.Description>
    </Alert.Root>
  {:else}
    <Card.Root>
      <Card.Content class="grid gap-3">
        {#if phase === 'boot'}
          <div class="grid gap-2" aria-busy="true">
            {#each [0, 1, 2] as i (i)}
              <Skeleton class="h-12 w-full" />
            {/each}
          </div>
        {:else}
          <div bind:this={scroller} class="grid max-h-[55vh] min-h-40 gap-3 overflow-y-auto pr-1">
            {#each turns as t, i (i)}
              <div class={t.role === 'user' ? 'flex justify-end' : 'flex justify-start'}>
                <div
                  class="max-w-[85%] rounded-lg border px-3 py-2 text-sm leading-relaxed {t.role ===
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
                  <p class="whitespace-pre-wrap break-words">
                    {t.content}{#if streaming && i === turns.length - 1}<span class="animate-pulse"
                        >▍</span
                      >{/if}
                  </p>
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
            {/each}
          </div>

          {#if streamError}
            <Alert.Root variant="destructive" role="alert">
              <Alert.Title>The assistant could not finish this reply</Alert.Title>
              <Alert.Description>{streamError}</Alert.Description>
            </Alert.Root>
          {/if}

          <div class="grid gap-2">
            <Textarea
              bind:value={draft}
              placeholder={
                phase === 'ready' ? 'Ask about trains, PNR rules, stations…' : 'Waiting for AI…'
              }
              rows={2}
              disabled={streaming || phase !== 'ready'}
              onkeydown={onKeydown}
            ></Textarea>
            <div class="flex items-center justify-between gap-2">
              <Button
                type="button"
                variant="ghost"
                size="xs"
                onclick={clearChat}
                disabled={streaming || turns.length === 0}
              >
                Clear chat
              </Button>
              <Button type="button" size="sm" onclick={() => send()} disabled={!canSend}>
                <SendHorizontalIcon class="size-4" />
                {streaming ? 'Answering…' : 'Send'}
              </Button>
            </div>
          </div>
        {/if}
      </Card.Content>
    </Card.Root>
  {/if}
</section>
