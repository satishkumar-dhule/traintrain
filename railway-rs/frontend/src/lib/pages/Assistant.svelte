<script>
  import { untrack } from 'svelte'
  import * as Card from '$lib/components/ui/card/index.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import { Textarea } from '$lib/components/ui/textarea/index.js'
  import { Badge } from '$lib/components/ui/badge/index.js'
  import SignalDot from '$lib/components/SignalDot.svelte'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import BotMessageSquareIcon from 'lucide-svelte/icons/bot-message-square'
  import SendHorizontalIcon from 'lucide-svelte/icons/send-horizontal'
  import Sparkles from 'lucide-svelte/icons/sparkles'
  import TrainFront from 'lucide-svelte/icons/train-front'
  import Building2 from 'lucide-svelte/icons/building-2'
  import Ticket from 'lucide-svelte/icons/ticket'
  import ClockIcon from 'lucide-svelte/icons/clock'
  import { renderMarkdown } from '$lib/markdown.js'
  import { viewport } from '$lib/media.svelte.js'
  import ToolCard from '$lib/components/chat/ToolCards.svelte'
  import IntentForm from '$lib/components/chat/IntentForm.svelte'
  import { classify, executePlan, PROJECTORS, nextActionsFor, buildPlanFor } from '$lib/chat/gate.js'
  import { createMemory, remember } from '$lib/chat/memory.js'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'
  import TrackRule from '$lib/components/TrackRule.svelte'
  import EmptyState from '$lib/components/EmptyState.svelte'

  const HISTORY_KEY = 'rc-assistant-history'
  const MAX_TURNS = 20
  const BADGE_TIMEOUT_MS = 8000

  // Zero-network starter chips for greetings/help.
  const DEFAULT_CHIPS = [
    { label: '12951 today', prompt: 'live status of 12951' },
    { label: 'SC→PUNE', prompt: 'trains from secunderabad to pune' },
    { label: 'SC→PUNE seats', prompt: 'seat availability from secunderabad to pune' },
    { label: 'PNR 1234567890', prompt: 'pnr status 1234567890' }
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
      case 'seat_availability':
        return `Seat availability **${d.from ?? ''} → ${d.to ?? ''}** ${d.date ? 'on ' + d.date : ''}:`
      case 'chart_status':
        return `Chart for **${d.train_number ?? ''}** ${d.journey_date ? 'on ' + d.journey_date : ''}:`
      case 'pnr_status':
        return `PNR **${d.pnr ?? ''}** — ${d.train_name ? d.train_name + ' (' + (d.train_number ?? '') + ')' : d.train_number ? 'Train ' + d.train_number : 'booking details'}:`
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
              (t.role === 'assistant' && (t.confirm || t.form)))
        )
        .slice(-MAX_TURNS)
        // Stale buttons are never clickable after reload: an open prompt that
        // survived persistence is dead on arrival.
        .map((t) => {
          if (t?.confirm?.state === 'open') return { ...t, confirm: { ...t.confirm, state: 'expired' } }
          if (t?.form && !t.form._submitted && !t.form._superseded && !t.form._expired)
            return { ...t, form: { ...t.form, _expired: true } }
          return t
        })
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
      scroller.scrollTo({ top: scroller.scrollHeight, behavior: 'smooth' })
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

  function supersedeOpenForms() {
    let changed = false
    for (const t of turns) {
      if (t?.form && !t.form._submitted && !t.form._superseded && !t.form._expired) {
        t.form._superseded = true
        changed = true
      }
    }
    if (changed) persist()
  }

  function formatSubmission(intentId, values) {
    const train = String(values.train ?? '').trim()
    const pnr = String(values.pnr ?? '').trim()
    const src = String(values.src ?? '').trim()
    const dst = String(values.dst ?? '').trim()
    const station = String(values.station ?? '').trim()
    const date = String(values.date ?? '').trim()
    switch (intentId) {
      case 'live_status':
        return train ? `live status of ${train}` : 'live status'
      case 'average_delay':
        return train ? `average delay of ${train}` : 'average delay'
      case 'train_schedule':
        return train ? `route of ${train}` : 'train schedule'
      case 'trains_between': {
        if (src && dst) {
          const base = `trains from ${src} to ${dst}`
          return date ? `${base} on ${date}` : base
        }
        return `trains between ${src || ''} and ${dst || ''}`.trim()
      }
      case 'station_board':
        return station ? `station board ${station}` : 'station board'
      case 'seat_availability': {
        if (src && dst) {
          const base = `seat availability from ${src} to ${dst}`
          // example spec: "seat availability SC→PUNE on 2026-08-25" — keep from/to for clarity
          return date ? `${base} on ${date}` : base
        }
        if (src || dst) return `seat availability ${src} → ${dst}`.trim()
        return 'seat availability'
      }
      case 'chart_status':
        return train ? `chart status of ${train}${date ? ` on ${date}` : ''}` : 'chart status'
      case 'pnr_status':
        return pnr ? `pnr status ${pnr}` : 'pnr status'
      default:
        return pnr ? `pnr status ${pnr}` : train ? `live status of ${train}` : src && dst ? `trains from ${src} to ${dst}` : station ? `station board ${station}` : intentId || 'request'
    }
  }

  function handleFormSubmit(turnIdx, { intentId, values }) {
    const t = turns[turnIdx]
    if (!t?.form) return
    t.form._submitted = true
    const echo = formatSubmission(intentId, values)
    turns.push({ role: 'user', content: echo })
    persist()
    const plan = buildPlanFor(
      intentId,
      { train: values.train, pnr: values.pnr, date: values.date },
      { srcQuery: values.src, dstQuery: values.dst, stationQuery: values.station, src: values.src, dst: values.dst, station: values.station }
    )
    void runTool(echo, plan, { userPushed: true })
  }

  function handleFormCancel(turnIdx) {
    const t = turns[turnIdx]
    if (!t?.form) return
    t.form._submitted = true
    t.form._cancelled = true
    turns.push({ role: 'user', content: 'Cancelled' })
    persist()
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
    supersedeOpenForms()

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
    // keep embedding tier: if it fires, prefer its intent to override the lexical guess
    const emb = await tryEmbedMatch(text)
    if (emb && verdict.form) {
      const overrideId = emb.cardKind ?? emb.plan?.cardKind
      if (overrideId) verdict.form.intentId = overrideId
    }
    turns.push({ role: 'user', content: text })
    turns.push({
      role: 'assistant',
      content: verdict.reply ?? `I need a bit more info to check **${verdict.form?.intentLabel ?? 'that'}**.`,
      reasoning: '',
      tokens: null,
      tools: [],
      cards: [],
      actions: [],
      form: verdict.form
    })
    persist()
    return
  }
    // Unknown/unmatched input: never dead-end, offer the local starters.
    pushLocal(
      text,
      "I couldn't match that to live data yet. I can check train status, routes, boards, delays, seats and PNR — try one of these:",
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
    chart_status: 'chart preparation status',
    pnr_status: 'PNR booking status'
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
        (p.pnr) ||
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

  // Deep delegation: single handler for all transcript interactions
  // - confirm buttons via [data-confirm-choice] + [data-msg-id] lookup
  // - next-action / starter chips via [data-prompt]
  // N² fan-out: one listener covers every future bubble without per-item closures.
  function onTranscriptClick(e) {
    // 1) Confirm-choice delegation (state-guarded, only 'open' reacts)
    const confirmBtn = e.target?.closest?.('button[data-confirm-choice]')
    if (confirmBtn && scroller?.contains(confirmBtn)) {
      const idx = Number(confirmBtn.closest('[data-msg-id]')?.dataset?.msgId)
      const t = turns[idx]
      if (!t?.confirm || t.confirm.state !== 'open') return
      const label = confirmBtn.dataset.label ?? ''
      t.confirm.state = 'answered'
      t.confirm.answer = label
      // Echo the choice as a user bubble so the transcript reads naturally.
      turns.push({ role: 'user', content: label })
      persist()
      if (confirmBtn.dataset.confirmChoice === '__exec') {
        void runTool(label, t.confirm.plan, { userPushed: true })
      }
      return
    }
    // 2) Chip / starter delegation via [data-prompt] — covers next-actions, empty-state starters
    const chipBtn = e.target?.closest?.('button[data-prompt]')
    if (chipBtn && scroller?.contains(chipBtn)) {
      const prompt = chipBtn.dataset.prompt
      if (prompt) sendText(prompt)
      return
    }
  }

  function onKeydown(e) {
    if (e.key === 'Enter' && !e.shiftKey && !e.defaultPrevented) {
      e.preventDefault()
      sendText(draft)
    }
  }
</script>

<section class="grid gap-4 max-lg:gap-3">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div class="grid gap-1.5">
      <p class="flex flex-wrap items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
        <span class="rounded-sm bg-saffron px-1.5 py-px text-[10px] font-bold tracking-[0.12em] text-saffron-ink">Indian Railways</span>
        <span class="inline-flex items-center gap-1.5"><span class="size-1.5 rounded-full bg-signal-go lamp-pulse"></span>Live data</span>
        <span class="hidden sm:inline">· free &amp; local</span>
      </p>
      <h1 class="signage text-2xl sm:text-3xl flex items-center gap-2">
        Ask Train Bro
        <Sparkles class="size-5 text-saffron hidden sm:inline" aria-hidden="true" />
      </h1>
      <p class="text-sm text-muted-foreground max-w-2xl leading-relaxed">
        Your on-device railway assistant — live running status, PNR, seat availability, station boards, routes &amp; delays. No hallucinations, just railway data.
      </p>
    </div>
    <div class="hidden sm:flex items-center gap-2">
      {#if model}<Badge variant="secondary" class="data-num">{model}</Badge>{/if}
      <Badge variant="outline" class="gap-1.5"><SignalDot tone="go" />Local-first</Badge>
    </div>
  </div>

  <TrackRule />

  <Alert.Root class="border-primary/15 bg-primary/[0.03]">
    <BotMessageSquareIcon class="size-4 text-primary" />
    <Alert.Title class="text-sm">Answers are served locally</Alert.Title>
    <Alert.Description class="text-xs leading-relaxed">
      Every reply is matched and fetched right in your browser against live railway APIs. Free-form AI is off — <span class="font-medium">RAILWAY_AI_ENABLED</span> only affects the badge above. Your chat stays on this device.
    </Alert.Description>
  </Alert.Root>

  <Card.Root class="overflow-hidden border shadow-sm max-lg:flex max-lg:flex-col max-lg:min-h-[min(68dvh,720px)]">
    <Card.Content class="p-0 flex flex-col min-h-0">
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        bind:this={scroller}
        role="log"
        aria-label="Chat transcript"
        class="flex-1 min-h-[280px] max-h-[56vh] max-lg:max-h-none max-lg:min-h-0 overflow-y-auto overscroll-contain p-4 sm:p-5 max-lg:p-3 gap-4 grid content-start"
        onclick={onTranscriptClick}
      >
        {#each turns as t, i (i)}
          <div data-msg-id={i} class={t.role === 'user' ? 'flex justify-end' : 'flex gap-3 justify-start min-w-0'}>
            {#if t.role !== 'user'}
              <div class="size-8 shrink-0 rounded-full bg-primary/10 border border-primary/15 flex items-center justify-center text-primary mt-0.5 max-lg:size-7">
                <BotMessageSquareIcon class="size-4 max-lg:size-3.5" />
              </div>
            {/if}
            <div class="group flex max-w-[78%] max-lg:max-w-[84%] flex-col gap-1.5 min-w-0">
              <div class={t.role === 'user' ? 'rounded-2xl rounded-br-md bg-primary text-primary-foreground px-4 py-2.5 text-sm leading-relaxed shadow-sm max-lg:px-3 max-lg:py-2 max-lg:text-[13px]' : 'rounded-2xl rounded-tl-md border bg-card px-4 py-3 text-sm leading-relaxed shadow-sm max-lg:px-3 max-lg:py-2.5 max-lg:text-[13px]'}>
                {#if t.tools?.length}
                  <div class="mb-2 flex flex-wrap gap-1">
                    {#each t.tools as name}
                      <span class="inline-flex items-center gap-1.5 rounded-full border bg-muted px-2 py-0.5 text-[11px] text-muted-foreground">
                        <SignalDot tone="go" />
                        checked live data: {name}
                      </span>
                    {/each}
                  </div>
                {/if}
                {#if t.reasoning}
                  <details class="mb-2 text-xs text-muted-foreground">
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
                {#if t.form}
                  <div class="mt-3">
                    {#if t.form._submitted}
                      <p class="mt-2 text-xs font-medium">✓ Submitted — checking...</p>
                    {:else if t.form._superseded}
                      <p class="mt-2 text-xs italic text-muted-foreground">Superseded by a newer message.</p>
                    {:else if t.form._expired}
                      <p class="mt-2 text-xs italic text-muted-foreground">Offer expired — ask again to run it.</p>
                    {:else}
                      <IntentForm form={t.form} onSubmit={(payload)=>handleFormSubmit(i, payload)} onCancel={()=>handleFormCancel(i)} />
                    {/if}
                  </div>
                {/if}
                {#if t.cards?.length}
                  <div class="mt-3 grid gap-2">
                    {#each t.cards as c, ci (ci)}
                      <ToolCard kind={c.kind} data={c.data} />
                    {/each}
                  </div>
                {/if}
                {#if t.actions?.length}
                  <div class="mt-2.5 flex flex-wrap gap-1.5" data-testid="next-actions">
                    {#each t.actions as a (a.label)}
                      <button
                        type="button"
                        data-prompt={a.prompt}
                        class="rounded-full border bg-accent px-3 py-1 text-xs font-medium text-accent-foreground transition-colors hover:bg-primary hover:text-primary-foreground hover:border-primary max-lg:min-h-9 max-lg:px-3.5"
                      >{a.label}</button>
                    {/each}
                  </div>
                {/if}
                {#if t.confirm}
                  {#if t.confirm.state === 'open'}
                    <div class="mt-2.5 flex flex-wrap gap-1.5" data-testid="confirm-actions">
                      {#each t.confirm.choices as c (c.value)}
                        <button
                          type="button"
                          data-confirm-choice={c.value}
                          data-label={c.label}
                          class="rounded-full border bg-accent px-3 py-1 text-xs font-medium text-accent-foreground transition-colors hover:bg-primary hover:text-primary-foreground max-lg:min-h-9 max-lg:px-3.5"
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
                          class="rounded-full border bg-accent px-3 py-1 text-xs font-medium text-accent-foreground max-lg:min-h-9 max-lg:px-3.5"
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
                  <p class="data-num mt-2 text-xs text-muted-foreground">{model} · {t.tokens} tokens</p>
                {/if}
              </div>
              <span class="px-1 text-[11px] text-muted-foreground {t.role === 'user' ? 'text-right' : 'text-left'}">
                {t.role === 'user' ? 'You' : 'Train Bro'} · just now
              </span>
            </div>
          </div>
        {:else}
          <div class="grid gap-4 py-2">
            <EmptyState icon={BotMessageSquareIcon} title="Ask anything about Indian Railways" hint="Try a starter below — I’ll open the right form instantly, prefilled from your words. No wrong form anymore.">
              <div class="mt-3 grid w-full max-w-2xl grid-cols-1 gap-2 sm:grid-cols-2">
                {#each [
                  {icon: TrainFront, label: 'Live status 12951', prompt: 'live status of 12951', desc: 'Where is the train now?'},
                  {icon: Building2, label: 'Board at Pune', prompt: 'station board pune', desc: 'Next arrivals in 2h'},
                  {icon: Ticket, label: 'PNR status', prompt: 'pnr status 1234567890', desc: 'Booking & berth details'},
                  {icon: ClockIcon, label: 'Avg delay 12626', prompt: 'average delay of 12626', desc: 'Worst stations history'}
                ] as c (c.label)}
                  <button
                    type="button"
                    data-prompt={c.prompt}
                    class="group flex items-start gap-3 rounded-xl border bg-card p-3 text-left transition-all hover:-translate-y-0.5 hover:border-primary/40 hover:shadow-md max-lg:p-2.5"
                  >
                    <span class="size-8 shrink-0 rounded-lg bg-primary/10 border border-primary/10 flex items-center justify-center text-primary group-hover:bg-primary group-hover:text-primary-foreground transition-colors max-lg:size-7">
                      <svelte:component this={c.icon} class="size-4 max-lg:size-3.5" />
                    </span>
                    <span class="grid gap-0.5 min-w-0">
                      <span class="text-sm font-medium leading-none max-lg:text-xs">{c.label}</span>
                      <span class="text-xs text-muted-foreground leading-tight max-lg:text-[11px]">{c.desc}</span>
                    </span>
                  </button>
                {/each}
              </div>
              <div class="mt-3 flex flex-wrap justify-center gap-1.5" data-testid="starter-chips">
                {#each [
                  {label: 'SC → PUNE trains', prompt: 'trains from secunderabad to pune'},
                  {label: 'Seats SC → PUNE', prompt: 'seat availability from secunderabad to pune'},
                  {label: 'Route 12951', prompt: 'route of 12951'}
                ] as c (c.label)}
                  <button
                    type="button"
                    data-prompt={c.prompt}
                    class="rounded-full border bg-muted px-3 py-1 text-xs font-medium text-muted-foreground transition-colors hover:bg-primary hover:text-primary-foreground hover:border-primary"
                  >{c.label}</button>
                {/each}
              </div>
            </EmptyState>
          </div>
        {/each}
        {#if busy}
          <div class="flex gap-3">
            <div class="size-8 rounded-full bg-primary/10 border flex items-center justify-center shrink-0 max-lg:size-7"><BotMessageSquareIcon class="size-4 max-lg:size-3.5 text-primary" /></div>
            <div class="rounded-2xl rounded-tl-md border bg-card px-4 py-3 shadow-sm flex items-center gap-1.5 max-lg:px-3 max-lg:py-2.5">
              <span class="size-2 rounded-full bg-primary animate-bounce [animation-delay:-0.3s]"></span>
              <span class="size-2 rounded-full bg-primary animate-bounce [animation-delay:-0.15s]"></span>
              <span class="size-2 rounded-full bg-primary animate-bounce"></span>
              <span class="ml-2 text-xs text-muted-foreground">Checking live data…</span>
            </div>
          </div>
        {/if}
      </div>

      {#if streamError}
        <div class="mx-4 sm:mx-5 max-lg:mx-3">
          <Alert.Root variant="destructive" role="alert" class="my-2">
            <Alert.Title>The lookup could not finish</Alert.Title>
            <Alert.Description>{streamError}</Alert.Description>
          </Alert.Root>
        </div>
      {/if}

      <div class="border-t bg-card p-3 sm:p-4 max-lg:p-3">
        <div class="flex items-end gap-2 rounded-2xl border bg-background p-2 shadow-sm ring-offset-background transition-colors focus-within:ring-1 focus-within:ring-ring focus-within:border-ring max-lg:p-1.5">
          <Textarea
            bind:value={draft}
            placeholder="Ask about trains, PNR, seats, stations… e.g. “live status 12951”"
            rows={1}
            class="min-h-10 max-h-32 resize-none border-0 bg-transparent px-3 py-2.5 text-sm shadow-none focus-visible:ring-0 max-lg:text-sm max-lg:min-h-9"
            onkeydown={onKeydown}
          ></Textarea>
          <Button type="button" size="icon" class="shrink-0 rounded-xl size-10 max-lg:size-9 shadow-sm" onclick={() => sendText(draft)} disabled={!canSend}>
            <SendHorizontalIcon class="size-4 max-lg:size-3.5" />
          </Button>
        </div>
        <div class="mt-2 flex flex-wrap items-center justify-between gap-2">
          <div class="flex items-center gap-1.5">
            <Button
              type="button"
              variant="ghost"
              size="xs"
              class="h-7 px-2.5 text-xs text-muted-foreground hover:text-foreground max-lg:h-8"
              onclick={clearChat}
              disabled={busy || turns.length === 0}
            >
              Clear chat
            </Button>
            <span class="hidden sm:inline text-[11px] text-muted-foreground">· history stays on this device</span>
          </div>
          <span class="text-[11px] text-muted-foreground hidden sm:inline">⏎ Send · ⇧⏎ New line</span>
        </div>
      </div>
    </Card.Content>
  </Card.Root>

  <p class="text-center text-[11px] text-muted-foreground leading-relaxed">Train Bro is local-first · PNR requires 10 digits · station codes auto-resolved · data source shown on every card</p>
</section>
<BottomSpacer />
