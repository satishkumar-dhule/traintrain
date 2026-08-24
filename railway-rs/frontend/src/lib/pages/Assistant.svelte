<script>
  import { untrack } from 'svelte'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import BotMessageSquareIcon from 'lucide-svelte/icons/bot-message-square'
  import SendHorizontalIcon from 'lucide-svelte/icons/send-horizontal'
  import { renderMarkdown } from '$lib/markdown.js'
  import { viewport } from '$lib/media.svelte.js'
  import ToolCard from '$lib/components/chat/ToolCards.svelte'
  import { classify, executePlan, PROJECTORS, nextActionsFor } from '$lib/chat/gate.js'
  import { createMemory, remember } from '$lib/chat/memory.js'

  const HISTORY_KEY = 'rc-assistant-history'
  const MAX_TURNS = 20
  const BADGE_TIMEOUT_MS = 8000

  // Zero-network starter chips for greetings/help.
  const DEFAULT_CHIPS = [
    { label: '12951 today', prompt: 'live status of 12951' },
    { label: 'SC→PUNE', prompt: 'trains from secunderabad to pune' },
    { label: 'SC→PUNE seats', prompt: 'seat availability from secunderabad to pune' }
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
            ((typeof t.content === 'string' && t.content.length > 0) ||
              (t.role === 'assistant' && t.confirm))
        )
        .slice(-MAX_TURNS)
        // Stale buttons are never clickable after reload: an open prompt that
        // survived persistence is dead on arrival.
        .map((t) =>
          t?.confirm?.state === 'open'
            ? { ...t, confirm: { ...t.confirm, state: 'expired' } }
            : t
        )
    } catch {
      return []
    }
  }

  let model = $state('')
  let turns = $state(untrack(loadHistory))
  let draft = $state(seed)
  let busy = $state(false)
  let streamError = $state(null)
  let scroller = $state(null)

  const canSend = $derived(!busy && draft.trim().length > 0)

  function persist() {
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(turns.slice(-MAX_TURNS)))
    } catch {}
  }

  function clearChat() {
    if (busy) return
    turns = []
    streamError = null
    try {
      localStorage.removeItem(HISTORY_KEY)
    } catch {}
  }

  // Best-effort badge only: show the server's model name when reachable.
  // Failure here must never block input — chat is fully client-side now.
  $effect(() => {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), BADGE_TIMEOUT_MS)
    fetch('/rail-api/ai/status', { signal: controller.signal })
      .then(async (res) => {
        if (!res.ok) return
        const body = await res.json()
        model = String(body?.model ?? '')
      })
      .catch(() => {})
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

  // Exactly ONE open confirm at a time: any newer message retires older ones.
  function supersedeOpenConfirms() {
    let changed = false
    for (const t of turns) {
      if (t?.confirm?.state === 'open') {
        t.confirm.state = 'superseded'
        changed = true
      }
    }
    if (changed) persist()
  }

  async function runTool(userText, plan, { userPushed = false } = {}) {
    busy = true
    streamError = null
    if (!userPushed && userText !== null) turns.push({ role: 'user', content: userText })
    try {
      const dto = await executePlan(plan, (u) => fetch(u))
      const data = PROJECTORS[plan.cardKind](dto)
      // Build the turn COMPLETELY before pushing: mutating a raw object
      // after push() bypasses the $state proxy and the UI never sees it.
      const turn = {
        role: 'assistant',
        content: templateFor(plan.cardKind, data),
        reasoning: '',
        tokens: null,
        tools: [plan.cardKind],
        cards: [{ kind: plan.cardKind, data }],
        actions: nextActionsFor(plan.cardKind, data)
      }
      turns.push(turn)
      remember(sessionMemory, userText, { content: turn.content, cards: turn.cards, actions: turn.actions }, {
        ttlMs: REPLAY_TTL_MS[plan.cardKind] ?? Infinity
      })
      persist()
    } catch (e) {
      streamError = e?.message ? e.message : String(e)
      persist()
    } finally {
      busy = false
    }
  }

  async function sendText(text) {
    text = (text ?? '').trim()
    if (!text || busy) return

    // Typed free text (or any chip) while a choice prompt is open closes it.
    supersedeOpenConfirms()

    // Local-first gate: everything below is served without any LLM round-trip.
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
      draft = ''
      void runTool(text, verdict.plan)
      return
    }
    if (verdict.kind === 'confirm') {
      draft = ''
      turns.push({ role: 'user', content: text })
      turns.push({
        role: 'assistant',
        content: verdict.text ?? '',
        reasoning: '',
        tokens: null,
        tools: [],
        cards: [],
        actions: [],
        confirm: {
          state: 'open',
          text: verdict.text ?? '',
          choices: Array.isArray(verdict.choices) ? verdict.choices : [],
          plan: verdict.plan
        }
      })
      persist()
      return
    }
  if (verdict.kind === 'help') {
    draft = ''
    const emb = await tryEmbedMatch(text)
    if (emb) {
      turns.push({ role: 'user', content: text })
      turns.push({
        role: 'assistant',
        content: emb.text,
        reasoning: '',
        tokens: null,
        tools: [],
        cards: [],
        actions: [],
        confirm: { state: 'open', text: emb.text, choices: emb.choices, plan: emb.plan }
      })
      persist()
      return
    }
    pushLocal(text, verdict.reply ?? '', { actions: verdict.actions ?? [] })
    return
  }
    // Unknown/unmatched input: never dead-end, offer the local starters.
    pushLocal(
      text,
      "I couldn't match that to live data yet. I can check train status, routes, boards, delays and seats — try one of these:",
      { actions: DEFAULT_CHIPS }
    )
  }

  // ---- optional semantic tier (lazy, never blocks, degrades silently) ----
  // Only fires when the lexical matcher returned help. The embed model lives
  // on a CDN (~30 MB first visit, cached afterwards), so a cold start must
  // not stall the help reply: race a tiny budget and fall through to help.
  const EMBED_BUDGET_MS = 400
  let embedModPromise = null
  function loadEmbedModule() {
    embedModPromise ??= import('$lib/chat/embed.js').catch(() => null)
    return embedModPromise
  }
  const EMBED_INTENT_LABELS = {
    live_status: 'live running status of a train',
    average_delay: 'average delay history of a train',
    train_schedule: 'full route / timetable of a train',
    trains_between: 'trains between two stations',
    station_board: 'arrivals board for a station',
    seat_availability: 'seat availability between stations',
    chart_status: 'chart preparation status'
  }

  async function tryEmbedMatch(text) {
    try {
      const mod = await Promise.race([
        loadEmbedModule().then((m) => (m?.embedClassify ? m.embedClassify(text) : null)),
        new Promise((r) => setTimeout(() => r(null), EMBED_BUDGET_MS))
      ])
      if (!mod?.plan) return null
      // Embedding hits are inherently low-confidence: confirm-first, and only
      // when the plan is actually executable with what we extracted.
      const p = mod.plan.params ?? {}
      const runnable =
        (p.train && p.src && p.dst) ||
        (p.train && !p.src && !p.dst && !mod.plan.resolve) ||
        ((p.src && p.dst) ?? false)
      if (!runnable) return null
      return {
        plan: mod.plan,
        text: `Did you mean: **${EMBED_INTENT_LABELS[mod.cardKind] ?? mod.cardKind}**?`,
        choices: [
          { label: 'Yes, fetch it', value: '__exec' },
          { label: 'Cancel', value: '__cancel' }
        ]
      }
    } catch {
      return null
    }
  }

  function pushLocal(userText, assistantText, extras = {}) {    streamError = null
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

  // Delegated click handling for confirm buttons. data-msg-id locates the
  // turn; only state==='open' reacts, so stale/superseded rows are safe.
  function onTranscriptClick(e) {
    const btn = e.target?.closest?.('button[data-confirm-choice]')
    if (!btn || !scroller?.contains(btn)) return
    const idx = Number(btn.closest('[data-msg-id]')?.dataset?.msgId)
    const t = turns[idx]
    if (!t?.confirm || t.confirm.state !== 'open') return
    const label = btn.dataset.label ?? ''
    t.confirm.state = 'answered'
    t.confirm.answer = label
    // Echo the choice as a user bubble so the transcript reads naturally.
    turns.push({ role: 'user', content: label })
    persist()
    if (btn.dataset.confirmChoice === '__exec') {
      void runTool(label, t.confirm.plan, { userPushed: true })
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
        Live-data answers about Indian Railways — served locally.
        {#if model}
          <Badge variant="secondary" class="font-mono">{model}</Badge>
        {/if}
      </p>
    </div>
  {/if}

  <Alert.Root>
    <BotMessageSquareIcon class="size-4" />
    <Alert.Title>Answers are served locally</Alert.Title>
    <Alert.Description>
      Every reply is matched and fetched right in your browser against live railway data. Free-form
      AI is off; RAILWAY_AI_ENABLED only affects the badge above.
    </Alert.Description>
  </Alert.Root>

  <Card.Root class="min-w-0 overflow-hidden max-lg:h-full max-lg:flex max-lg:flex-col">
    <Card.Content class="grid min-h-0 gap-3 max-lg:h-full max-lg:min-h-0 max-lg:gap-2">
      <!-- Intentional event delegation: only real <button data-confirm-choice> targets react. -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        bind:this={scroller}
        role="log"
        aria-label="Chat transcript"
        class="grid max-h-[55vh] max-h-[50dvh] min-h-40 gap-3 overflow-y-auto overscroll-contain pr-1 max-lg:max-h-none max-lg:min-h-0 max-lg:flex-1"
        onclick={onTranscriptClick}
      >
        {#each turns as t, i (i)}
          <div
            data-msg-id={i}
            class={t.role === 'user' ? 'flex justify-end' : 'flex justify-start min-w-0'}
          >
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
                  {@html renderMarkdown(t.content)}
                </div>
              {/if}
              {#if t.cards?.length}
                <div class="mt-2 grid gap-2">
                  {#each t.cards as c, ci (ci)}
                    <ToolCard kind={c.kind} data={c.data} />
                  {/each}
                </div>
              {/if}
              {#if t.actions?.length}
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
              {#if t.confirm}
                {#if t.confirm.state === 'open'}
                  <div class="mt-2 flex flex-wrap gap-1.5" data-testid="confirm-actions">
                    {#each t.confirm.choices as c (c.value)}
                      <button
                        type="button"
                        data-confirm-choice={c.value}
                        data-label={c.label}
                        class="rounded-full border bg-muted/60 px-2.5 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground max-lg:min-h-11 max-lg:items-center max-lg:px-4 max-lg:inline-flex"
                      >{c.label}</button>
                    {/each}
                  </div>
                {:else if t.confirm.state === 'answered'}
                  <p class="mt-2 text-xs font-medium">✓ {t.confirm.answer}</p>
                  <div aria-disabled="true" class="pointer-events-none mt-1.5 flex flex-wrap gap-1.5 opacity-50 select-none">
                    {#each t.confirm.choices as c (c.value)}
                      <button
                        type="button"
                        tabindex="-1"
                        aria-disabled="true"
                        data-confirm-choice={c.value}
                        data-label={c.label}
                        class="rounded-full border bg-muted/60 px-2.5 py-0.5 text-xs text-muted-foreground max-lg:min-h-11 max-lg:items-center max-lg:px-4 max-lg:inline-flex"
                      >{c.label}</button>
                    {/each}
                  </div>
                {:else}
                  <p class="mt-2 text-xs italic text-muted-foreground">
                    {t.confirm.state === 'expired'
                      ? 'Offer expired — ask again to run it.'
                      : 'Superseded by a newer message.'}
                  </p>
                {/if}
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
          <Alert.Title>The lookup could not finish</Alert.Title>
          <Alert.Description>{streamError}</Alert.Description>
        </Alert.Root>
      {/if}

      <div class="sticky bottom-0 grid gap-2 bg-card max-lg:p-2 max-lg:border-t max-lg:-mx-3 max-lg:-mb-3 max-lg:px-4">
        <Textarea
          bind:value={draft}
          placeholder="Ask about trains, delays, seats…"
          rows={2}
          class="min-h-11 max-lg:min-h-[40px] max-lg:text-sm"
          onkeydown={onKeydown}
        ></Textarea>
        <div class="flex flex-wrap items-center justify-between gap-2">
          <Button
            type="button"
            variant="ghost"
            size="xs"
            class="max-lg:h-9 max-lg:px-3 max-lg:text-xs"
            onclick={clearChat}
            disabled={busy || turns.length === 0}
          >
            Clear chat
          </Button>
          <Button type="button" size="sm" class="max-lg:h-9 max-lg:px-3 max-lg:text-xs shrink-0" onclick={() => sendText(draft)} disabled={!canSend}>
            <SendHorizontalIcon class="size-4 max-lg:size-3.5" />
            {busy ? 'Checking…' : 'Send'}
          </Button>
        </div>
      </div>
    </Card.Content>
  </Card.Root>
</section>
<div class="h-20 lg:hidden"></div>
