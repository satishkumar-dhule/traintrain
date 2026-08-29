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
  import History from 'lucide-svelte/icons/history'
  import Command from 'lucide-svelte/icons/command'
  import Mic from 'lucide-svelte/icons/mic'
  import MicOff from 'lucide-svelte/icons/mic-off'
  import Volume2 from 'lucide-svelte/icons/volume-2'
  import VolumeX from 'lucide-svelte/icons/volume-x'
  import Copy from 'lucide-svelte/icons/copy'
  import Share2 from 'lucide-svelte/icons/share-2'
  import Check from 'lucide-svelte/icons/check'
  import CheckCheck from 'lucide-svelte/icons/check-check'
  import ArrowDown from 'lucide-svelte/icons/arrow-down'
  import SearchIcon from 'lucide-svelte/icons/search'
  import { renderMarkdown } from '$lib/markdown.js'
  import { viewport } from '$lib/media.svelte.js'
  import ToolCard from '$lib/components/chat/ToolCards.svelte'
  import IntentForm from '$lib/components/chat/IntentForm.svelte'
  import SuggestedCarousel from '$lib/components/chat/SuggestedCarousel.svelte'
  import CommandPalette from '$lib/components/chat/CommandPalette.svelte'
  import HistoryDrawer from '$lib/components/chat/HistoryDrawer.svelte'
  import { classify, executePlan, PROJECTORS, nextActionsFor, buildPlanFor } from '$lib/chat/gate.js'
  import { createMemory, remember } from '$lib/chat/memory.js'
  import { createLiveEmulator, createAnchoring, haptic, tickSound } from '$lib/chat/liveEmulator.js'
  import BottomSpacer from '$lib/components/BottomSpacer.svelte'
  import TrackRule from '$lib/components/TrackRule.svelte'
  import EmptyState from '$lib/components/EmptyState.svelte'

  const HISTORY_KEY = 'rc-assistant-history'
  const MAX_TURNS = 20
  const BADGE_TIMEOUT_MS = 8000

  const DEFAULT_CHIPS = [
    { label: '12951 today', prompt: 'live status of 12951' },
    { label: 'SC→PUNE', prompt: 'trains from secunderabad to pune' },
    { label: 'SC→PUNE seats', prompt: 'seat availability from secunderabad to pune' },
    { label: 'PNR 1234567890', prompt: 'pnr status 1234567890' }
  ]

  const sessionMemory = createMemory()
  const REPLAY_TTL_MS = { live_status: 30_000, station_board: 30_000 }

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
  let streamingIdx = $state(null)
  let streamContent = $state('')
  let showCursor = $state(false)
  let thinking = $state(false)
  let paletteOpen = $state(false)
  let historyOpen = $state(false)
  let isOffline = $state(typeof navigator !== 'undefined' ? !navigator.onLine : false)
  let showJump = $state(false)
  let soundOn = $state(false)
  let listening = $state(false)
  let copiedIdx = $state(null)
  let feedbackMap = $state({})
  let kbOffset = $state(0)

  // sound pref from storage
  try { soundOn = localStorage.getItem('rc-sound') === 'on' } catch {}

  const canSend = $derived(!busy && draft.trim().length > 0)
  const lastAssistantTurn = $derived([...turns].reverse().find(t=>t.role==='assistant' && t.cards?.length))

  // adaptive chips
  let adaptiveChips = $derived.by(()=>{
    if (turns.length===0) return DEFAULT_CHIPS
    const last = [...turns].reverse().find(t=>t.role==='assistant')
    if (!last?.cards?.[0]) return last?.actions?.length ? last.actions.slice(0,4) : DEFAULT_CHIPS
    const kind = last.cards[0].kind
    const data = last.cards[0].data
    const nxt = nextActionsFor(kind, data)
    if (nxt?.length) return nxt.slice(0,4)
    return DEFAULT_CHIPS
  })

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
    haptic(10)
  }

  function formatTime(ts){
    try { return new Intl.DateTimeFormat('en-IN',{hour:'numeric',minute:'2-digit',hour12:true}).format(new Date(ts??Date.now())) } catch { return 'just now' }
  }

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

  // offline listener
  $effect(()=>{
    const onOnline = ()=> isOffline=false
    const onOffline = ()=> isOffline=true
    window.addEventListener('online', onOnline)
    window.addEventListener('offline', onOffline)
    return ()=>{ window.removeEventListener('online', onOnline); window.removeEventListener('offline', onOffline) }
  })

  // keyboard handler for palette
  $effect(()=>{
    const h=(e)=>{
      if((e.metaKey||e.ctrlKey) && e.key.toLowerCase()==='k'){ e.preventDefault(); paletteOpen=!paletteOpen }
      if(e.key==='/' && !busy && document.activeElement?.tagName!=='INPUT' && document.activeElement?.tagName!=='TEXTAREA'){ e.preventDefault(); paletteOpen=true }
    }
    window.addEventListener('keydown', h)
    return ()=> window.removeEventListener('keydown', h)
  })

  // visualViewport keyboard avoidance
  $effect(()=>{
    const vv = window.visualViewport
    if(!vv) return
    const upd=()=>{
      const off = Math.max(0, window.innerHeight - vv.height - vv.offsetTop)
      kbOffset = off
      document.documentElement.style.setProperty('--kb-offset', off+'px')
    }
    vv.addEventListener('resize', upd)
    vv.addEventListener('scroll', upd)
    upd()
    return ()=>{ vv.removeEventListener('resize',upd); vv.removeEventListener('scroll',upd) }
  })

  // anchoring & jump pill
  let anchoring = null
  $effect(()=>{
    if(!scroller) return
    anchoring = createAnchoring(scroller, { threshold: 100 })
    const onScroll=()=>{
      const near = anchoring.isNearBottom()
      showJump = !near && turns.length>2
    }
    scroller.addEventListener('scroll', onScroll, {passive:true})
    const ro = new ResizeObserver(()=> onScroll())
    ro.observe(scroller)
    return ()=>{ scroller.removeEventListener('scroll', onScroll); ro.disconnect() }
  })

  $effect(() => {
    void turns.length
    void (turns[turns.length - 1]?.content.length ?? 0)
    void streamContent.length
    void thinking
    untrack(() => {
      if (!scroller) return
      if (anchoring?.pinned ?? true) {
        anchoring?.maybeScroll(true)
      }
    })
  })

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
    turns.push({ role: 'user', content: echo, ts: Date.now() })
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
    turns.push({ role: 'user', content: 'Cancelled', ts: Date.now() })
    persist()
  }

  let liveEmulator = $state(null)
  let abortCtrl = null

  async function runTool(userText, plan, { userPushed = false } = {}) {
    busy = true
    streamError = null
    thinking = true
    showCursor = true
    if (!userPushed && userText !== null) turns.push({ role: 'user', content: userText, ts: Date.now() })
    // if offline and we have memory, try replay immediately as live-feel
    if (isOffline) {
      const memHit = sessionMemory ? (()=>{ try{ const {findReplay}=require; }catch{} })() : null
    }
    try {
      const dto = await executePlan(plan, (u) => fetch(u))
      const data = PROJECTORS[plan.cardKind](dto)
      const turn = {
        role: 'assistant',
        content: templateFor(plan.cardKind, data),
        reasoning: '',
        tokens: null,
        tools: [plan.cardKind],
        cards: [{ kind: plan.cardKind, data }],
        actions: nextActionsFor(plan.cardKind, data),
        ts: Date.now(),
        dataAge: Date.now()
      }
      // create shell turn for streaming
      const idx = turns.length
      turns.push({ role:'assistant', content: '', reasoning:'', tokens:null, tools:[plan.cardKind], cards: [], actions: [], ts: Date.now(), _streaming:true })
      streamingIdx = idx
      streamContent = ''
      persist() // not yet, will persist after stream? we keep shell not persisted final yet
      // stream
      liveEmulator = createLiveEmulator({ tokenMs: 26, jitterMs: 14, cardStaggerMs: 180, thinkingMin: 280, thinkingMax: 680 })
      abortCtrl = new AbortController()
      let finalCards = []
      let finalActions = []
      await liveEmulator.streamTurn(turn, {
        onThinking: (v)=> { thinking=v },
        onToken: (partial, done)=>{
          if(streamingIdx!==null && turns[streamingIdx]){
            turns[streamingIdx].content = partial
            streamContent = partial
            if(done) showCursor=false
            // trigger reactivity
            turns = [...turns]
            if (done) haptic(10)
          }
        },
        onCards: (cardsSlice, done)=>{
          if(streamingIdx!==null && turns[streamingIdx]){
            turns[streamingIdx].cards = cardsSlice.map(k=> ({...k}))
            turns = [...turns]
            if(soundOn) tickSound(true)
            haptic(6)
          }
          finalCards = cardsSlice
        },
        onActions: (acts)=>{
          if(streamingIdx!==null && turns[streamingIdx]){
            turns[streamingIdx].actions = acts
            turns = [...turns]
          }
          finalActions = acts
        },
        onDone: ()=>{
          if(streamingIdx!==null && turns[streamingIdx]){
            const t = turns[streamingIdx]
            t.content = turn.content
            t.cards = turn.cards
            t.actions = turn.actions
            t.tools = turn.tools
            t.ts = turn.ts
            t.dataAge = turn.dataAge
            delete t._streaming
            turns = [...turns]
            remember(sessionMemory, userText, { content: t.content, cards: t.cards, actions: t.actions }, {
              ttlMs: REPLAY_TTL_MS[plan.cardKind] ?? Infinity
            })
            persist()
            if(soundOn) tickSound(true)
            haptic([10,18,10])
          }
          streamingIdx=null
          showCursor=false
          thinking=false
        }
      }, abortCtrl.signal)

      // fallback if emulator was aborted early but we have data
      if(streamingIdx!==null){
        const t = turns[streamingIdx]
        if(t){
          t.content = turn.content
          t.cards = turn.cards
          t.actions = turn.actions
          delete t._streaming
          turns = [...turns]
          streamingIdx=null
          showCursor=false
          thinking=false
          persist()
        }
      }

    } catch (e) {
      // try offline cache replay
      let recovered = false
      try {
        const last = [...turns].reverse().find(t=>t.role==='assistant' && t.cards?.length)
        if(isOffline && last){
          turns.push({ role:'assistant', content: `**(offline)** showing last cached result — ${templateFor(last.cards[0].kind, last.cards[0].data)}`, reasoning:'', tokens:null, tools:[last.cards[0].kind], cards: last.cards, actions: last.actions, ts: Date.now(), stale:true })
          persist()
          recovered=true
        }
      } catch {}
      if(!recovered){
        streamError = e?.message ? e.message : String(e)
        // clean streaming shell if exists
        if(streamingIdx!==null && turns[streamingIdx]?._streaming){
          turns.splice(streamingIdx,1)
          turns=[...turns]
          streamingIdx=null
        }
        thinking=false
        showCursor=false
        persist()
      }
    } finally {
      busy = false
      if(!streamingIdx) { thinking=false; showCursor=false }
    }
  }

  async function sendText(text) {
    text = (text ?? '').trim()
    if (!text || busy) return
    if (liveEmulator) { try{ liveEmulator.abort() }catch{} }
    if (abortCtrl) try{ abortCtrl.abort() }catch{}
    streamingIdx=null
    showCursor=false
    thinking=false

    supersedeOpenConfirms()
    supersedeOpenForms()

    const verdict = classify(text, sessionMemory)
    if (verdict.kind === 'trivial') {
      pushLocal(text, verdict.reply, { actions: DEFAULT_CHIPS })
      return
    }
    if (verdict.kind === 'replay') {
      const a = verdict.entry.answer
      // replay also feels live — fast stream
      const idx = turns.length
      turns.push({ role:'user', content: text, ts: Date.now() })
      turns.push({ role:'assistant', content: '', tools: [], cards: [], actions: [], ts: Date.now(), _streaming:true })
      streamingIdx=idx+1
      busy=true; thinking=true; showCursor=true
      liveEmulator = createLiveEmulator({ tokenMs: 10, jitterMs: 6, thinkingMin: 120, thinkingMax: 220 })
      await liveEmulator.streamTurn({ content: `*(from memory)* ${a.content}`, cards: a.cards??[], actions: a.actions??[], tools:[] }, {
        onToken:(p,d)=>{ if(turns[streamingIdx]){ turns[streamingIdx].content=p; turns=[...turns]; if(d) showCursor=false } },
        onCards:(c)=>{ if(turns[streamingIdx]){ turns[streamingIdx].cards=c; turns=[...turns] } },
        onActions:(acts)=>{ if(turns[streamingIdx]){ turns[streamingIdx].actions=acts; turns=[...turns] } },
        onDone:()=>{
          const t=turns[streamingIdx]; if(t){ t.content=`*(from memory)* ${a.content}`; t.cards=a.cards??[]; t.actions=a.actions??[]; delete t._streaming; turns=[...turns]; persist() }
          streamingIdx=null; thinking=false; busy=false
        }
      })
      if(streamingIdx!==null){ const t=turns[streamingIdx]; t.content=`*(from memory)* ${a.content}`; t.cards=a.cards??[]; t.actions=a.actions??[]; delete t._streaming; turns=[...turns]; streamingIdx=null; thinking=false; busy=false; persist() }
      return
    }
    if (verdict.kind === 'tool') {
      draft = ''
      haptic(8)
      if(soundOn) tickSound(true)
      void runTool(text, verdict.plan)
      return
    }
    if (verdict.kind === 'confirm') {
      draft = ''
      turns.push({ role: 'user', content: text, ts: Date.now() })
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
        },
        ts: Date.now()
      })
      persist()
      return
    }
  if (verdict.kind === 'help') {
    draft = ''
    const emb = await tryEmbedMatch(text)
    if (emb && verdict.form) {
      const overrideId = emb.cardKind ?? emb.plan?.cardKind
      if (overrideId) verdict.form.intentId = overrideId
    }
    turns.push({ role: 'user', content: text, ts: Date.now() })
    turns.push({
      role: 'assistant',
      content: verdict.reply ?? `I need a bit more info to check **${verdict.form?.intentLabel ?? 'that'}**.`,
      reasoning: '',
      tokens: null,
      tools: [],
      cards: [],
      actions: [],
      form: verdict.form,
      ts: Date.now()
    })
    persist()
    return
  }
    pushLocal(
      text,
      "I couldn't match that to live data yet. I can check train status, routes, boards, delays, seats and PNR — try one of these:",
      { actions: DEFAULT_CHIPS }
    )
  }

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

  function pushLocal(userText, assistantText, extras = {}) {
    streamError = null
    draft = ''
    const idx = turns.length
    turns.push({ role: 'user', content: userText, ts: Date.now() })
    turns.push({
      role: 'assistant',
      content: '',
      reasoning: '',
      tokens: null,
      tools: [],
      cards: extras.cards ?? [],
      actions: extras.actions ?? [],
      ts: Date.now(),
      _streaming:true
    })
    const sIdx = idx+1
    streamingIdx=sIdx
    busy=true; thinking=true; showCursor=true
    const emu = createLiveEmulator({ tokenMs: 14, jitterMs: 8, thinkingMin: 160, thinkingMax: 320 })
    liveEmulator=emu
    emu.streamTurn({ content: assistantText, cards: extras.cards??[], actions: extras.actions??[], tools:[] },{
      onToken:(p,d)=>{ if(turns[sIdx]){ turns[sIdx].content=p; turns=[...turns]; if(d) showCursor=false } },
      onCards:(c)=>{ if(turns[sIdx]){ turns[sIdx].cards=c; turns=[...turns] } },
      onActions:(a)=>{ if(turns[sIdx]){ turns[sIdx].actions=a; turns=[...turns] } },
      onDone:()=>{
        const t=turns[sIdx]; if(t){ t.content=assistantText; t.cards=extras.cards??[]; t.actions=extras.actions??[]; delete t._streaming; turns=[...turns]; persist(); haptic(6) }
        streamingIdx=null; thinking=false; busy=false
      }
    })
  }

  function onTranscriptClick(e) {
    const confirmBtn = e.target?.closest?.('button[data-confirm-choice]')
    if (confirmBtn && scroller?.contains(confirmBtn)) {
      const idx = Number(confirmBtn.closest('[data-msg-id]')?.dataset?.msgId)
      const t = turns[idx]
      if (!t?.confirm || t.confirm.state !== 'open') return
      const label = confirmBtn.dataset.label ?? ''
      t.confirm.state = 'answered'
      t.confirm.answer = label
      turns.push({ role: 'user', content: label, ts: Date.now() })
      persist()
      haptic(10)
      if (confirmBtn.dataset.confirmChoice === '__exec') {
        void runTool(label, t.confirm.plan, { userPushed: true })
      }
      return
    }
    const chipBtn = e.target?.closest?.('button[data-prompt]')
    if (chipBtn && scroller?.contains(chipBtn)) {
      const prompt = chipBtn.dataset.prompt
      if (prompt) { haptic(8); sendText(prompt) }
      return
    }
    // copy/share/feedback delegation
    const copyBtn = e.target?.closest?.('button[data-copy]')
    if(copyBtn && scroller?.contains(copyBtn)){
      const idx = Number(copyBtn.closest('[data-msg-id]')?.dataset?.msgId)
      const t = turns[idx]
      if(t){
        const txt = String(t.content ?? '').replace(/\*\*/g,'')
        try{ navigator.clipboard.writeText(txt) }catch{}
        copiedIdx=idx
        haptic(6)
        setTimeout(()=>{ if(copiedIdx===idx) copiedIdx=null }, 1400)
      }
      return
    }
    const shareBtn = e.target?.closest?.('button[data-share]')
    if(shareBtn && scroller?.contains(shareBtn)){
      const idx = Number(shareBtn.closest('[data-msg-id]')?.dataset?.msgId)
      const t=turns[idx]
      if(t){
        const txt = String(t.content ?? '').replace(/\*\*/g,'')
        if(navigator.share){ navigator.share({title:'Train Bro', text: txt}).catch(()=>{}) } else { try{ navigator.clipboard.writeText(txt) }catch{}; copiedIdx=idx; setTimeout(()=>copiedIdx=null,1200) }
        haptic(8)
      }
      return
    }
    const fbBtn = e.target?.closest?.('button[data-feedback]')
    if(fbBtn && scroller?.contains(fbBtn)){
      const idx = Number(fbBtn.closest('[data-msg-id]')?.dataset?.msgId)
      const val = fbBtn.dataset.feedback
      feedbackMap[idx]=val
      feedbackMap={...feedbackMap}
      try{ localStorage.setItem('rc-feedback', JSON.stringify(feedbackMap)) }catch{}
      haptic(6)
      return
    }
  }

  function onKeydown(e) {
    if (e.key === 'Enter' && !e.shiftKey && !e.defaultPrevented) {
      e.preventDefault()
      sendText(draft)
    }
  }

  // voice
  let recog = null
  function toggleMic(){
    const SR = window.SpeechRecognition || window.webkitSpeechRecognition
    if(!SR){ listening=false; return }
    if(listening && recog){
      try{ recog.stop() }catch{}
      listening=false
      return
    }
    try{
      recog = new SR()
      recog.lang = 'en-IN'
      recog.interimResults = true
      recog.continuous = false
      recog.onstart=()=> listening=true
      recog.onend=()=> listening=false
      recog.onerror=()=> listening=false
      recog.onresult=(e)=>{
        let final=''
        for(let i=0;i<e.results.length;i++){ final+= e.results[i][0].transcript }
        draft = final
        if(e.results[e.results.length-1].isFinal){
          listening=false
          if(final.trim()) sendText(final.trim())
        }
      }
      recog.start()
      haptic(8)
    }catch{ listening=false }
  }

  function toggleSound(){
    soundOn=!soundOn
    try{ localStorage.setItem('rc-sound', soundOn?'on':'off') }catch{}
    haptic(6)
    if(soundOn) tickSound(true)
  }

  function jumpToLatest(){
    anchoring?.scrollToBottom(true)
    showJump=false
  }
</script>

<section class="grid gap-4 max-lg:gap-3">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div class="grid gap-1.5">
      <p class="flex flex-wrap items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
        <span class="rounded-sm bg-saffron px-1.5 py-px text-xs font-bold tracking-[0.12em] text-saffron-ink">Indian Railways</span>
        <span class="inline-flex items-center gap-1.5"><span class="size-1.5 rounded-full bg-signal-go lamp-pulse"></span>Live data</span>
        <span class="hidden sm:inline">· free &amp; local</span>
        {#if isOffline}<span class="rounded-full bg-signal-hold/15 border border-signal-hold/30 px-2 py-0.5 text-xs font-bold tracking-[0.12em] text-signal-hold-ink">Offline · cached</span>{/if}
      </p>
      <h1 class="signage text-2xl sm:text-3xl flex items-center gap-2">
        Ask Train Bro
        <Sparkles class="size-5 text-saffron hidden sm:inline" aria-hidden="true" />
      </h1>
    </div>
    <div class="hidden sm:flex items-center gap-2">
      {#if model}<Badge variant="secondary" class="data-num">{model}</Badge>{/if}
      <Badge variant="outline" class="gap-1.5"><SignalDot tone={isOffline?'hold':'go'} />{isOffline?'Offline':'Live'}</Badge>
      <Button variant="ghost" size="icon" class="size-8" onclick={()=>paletteOpen=true} aria-label="Commands"><Command class="size-4"/></Button>
      <Button variant="ghost" size="icon" class="size-8" onclick={()=>historyOpen=true} aria-label="History"><History class="size-4"/></Button>
    </div>
    <div class="flex sm:hidden items-center gap-1">
      <Button variant="ghost" size="icon" class="size-8" onclick={()=>paletteOpen=true} aria-label="Commands"><Command class="size-4"/></Button>
      <Button variant="ghost" size="icon" class="size-8" onclick={()=>historyOpen=true} aria-label="History"><History class="size-4"/></Button>
      <Badge variant="outline" class="gap-1"><SignalDot tone={isOffline?'hold':'go'}/>{isOffline?'Offline':'Live'}</Badge>
    </div>
  </div>

  <TrackRule />

  {#if isOffline}
    <div class="rounded-xl border border-signal-hold/30 bg-signal-hold/10 px-3 py-2 flex items-center gap-2 text-xs">
      <span class="size-2 rounded-full bg-signal-hold lamp-pulse"></span>
      <span class="font-medium">Offline — showing cached answers as live</span>
      <span class="text-muted-foreground hidden sm:inline">· will refresh when back online</span>
      <span class="ml-auto text-[11px] text-muted-foreground">static · no refresh needed</span>
    </div>
  {/if}

  <Card.Root class="overflow-hidden border shadow-sm max-lg:flex max-lg:flex-col max-lg:min-h-[min(68dvh,720px)]">
    <Card.Content class="p-0 flex flex-col min-h-0">
      <div class="px-4 sm:px-5 pt-3 flex items-center gap-1.5 text-[11px] text-muted-foreground">
        <span class="hidden sm:inline-flex items-center gap-1.5"><span class="size-1 rounded-full bg-signal-go animate-pulse"></span>static shell · live feel</span>
        <span class="sm:hidden inline-flex items-center gap-1"><span class="size-1 rounded-full bg-signal-go animate-pulse"></span>static · live</span>
        <span class="ml-auto flex items-center gap-1">
          <button class="rounded-full border bg-muted px-2 py-0.5 text-[11px] hover:bg-accent transition-colors" onclick={toggleSound} aria-label="Toggle sound">
            {#if soundOn}<Volume2 class="size-3 inline mr-1"/>Sound on{:else}<VolumeX class="size-3 inline mr-1"/>Quiet{/if}
          </button>
          <span class="hidden sm:inline">· {turns.length} turns</span>
        </span>
      </div>
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        bind:this={scroller}
        role="log"
        aria-live="polite"
        aria-label="Chat transcript"
        class="relative flex-1 min-h-[280px] max-h-[56vh] max-lg:max-h-none max-lg:min-h-0 overflow-y-auto overscroll-contain p-4 sm:p-5 max-lg:p-3 gap-3 grid content-start"
        onclick={onTranscriptClick}
      >
        {#each turns as t, i (i)}
          {@const isStreaming = t._streaming && i===streamingIdx}
          {@const isAssistant = t.role !== 'user'}
          <div data-msg-id={i} class={t.role === 'user' ? 'flex justify-end chat-in' : 'flex gap-3 justify-start min-w-0 chat-in'} style="animation-delay:{Math.min(i*18,120)}ms">
            {#if isAssistant}
              <div class="size-8 shrink-0 rounded-full bg-saffron/14 border border-saffron/20 flex items-center justify-center text-saffron-ink dark:text-saffron mt-0.5 max-lg:size-7 shadow-sm">
                <BotMessageSquareIcon class="size-4 max-lg:size-3.5" />
              </div>
            {/if}
            <div class="group flex max-w-[78%] max-lg:max-w-[84%] flex-col gap-1.5 min-w-0">
              <div class={t.role === 'user' ? 'rounded-[20px] rounded-br-[6px] bg-primary text-primary-foreground px-4 py-2.5 text-sm leading-[1.55] shadow-[0_2px_10px_oklch(0.40_0.13_272/0.18),0_1px_0_oklch(1_0_0/0.12)_inset] border border-primary/10 max-lg:px-3 max-lg:py-2 max-lg:text-[13px]' : 'rounded-[20px] rounded-tl-[6px] border bg-card px-4 py-3 text-sm leading-[1.55] shadow-[0_1px_0_oklch(1_0_0/0.7)_inset,0_4px_16px_oklch(0.235_0.028_265/0.06)] dark:shadow-[0_1px_0_oklch(1_0_0/0.08)_inset] max-lg:px-3 max-lg:py-2.5 max-lg:text-[13px] ' + (t.stale? ' border-signal-hold/30 bg-signal-hold/5' : ' border-border')}>
                {#if t.tools?.length}
                  <div class="mb-2 flex flex-wrap gap-1">
                    {#each t.tools as name}
                      <span class="inline-flex items-center gap-1.5 rounded-full border bg-muted px-2 py-0.5 text-[11px] text-muted-foreground">
                        <SignalDot tone={t.stale?'hold':'go'} />
                        {t.stale?'cached':'checked live data'}: {name}
                      </span>
                    {/each}
                    {#if t.dataAge}
                      <span class="inline-flex items-center rounded-full border bg-card px-2 py-0.5 text-[11px] text-muted-foreground data-num">{Math.max(0,Math.round((Date.now()-t.dataAge)/1000))}s ago</span>
                    {/if}
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
                    {#if isStreaming && showCursor}<span class="live-cursor" aria-hidden="true"></span>{/if}
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
                      <div class="card-stagger" style="animation-delay:{ci*90}ms">
                        <ToolCard kind={c.kind} data={c.data} />
                      </div>
                    {/each}
                  </div>
                {:else if isStreaming && thinking}
                  <div class="mt-3 space-y-2">
                    <div class="h-20 rounded-xl border bg-muted/50 animate-pulse"></div>
                    <div class="h-3 w-2/3 rounded bg-muted animate-pulse"></div>
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
              <div class="flex items-center gap-1.5 px-1 text-[11px] text-muted-foreground {t.role === 'user' ? 'justify-end' : 'justify-start'}">
                <span class="data-num flex items-center gap-1">
                  {t.role === 'user' ? 'You' : 'Train Bro'} · {formatTime(t.ts)}
                  {#if t.role==='user'}
                    {#if busy && i===turns.length-1}<span class="inline-flex items-center gap-1 text-muted-foreground"><span class="size-1 rounded-full bg-signal-hold animate-pulse"></span>sending</span>
                    {:else if streamError && i===turns.length-2}<span class="text-signal-stop-ink">failed</span>
                    {:else}<CheckCheck class="size-3 text-signal-go" />{/if}
                  {/if}
                </span>
                {#if isAssistant && !isStreaming}
                  <span class="ml-1 hidden group-hover:inline-flex items-center gap-1">
                    <button data-copy class="size-6 grid place-items-center rounded-full hover:bg-muted transition-colors" aria-label="Copy answer">
                      {#if copiedIdx===i}<Check class="size-3 text-signal-go"/>{:else}<Copy class="size-3"/>{/if}
                    </button>
                    <button data-share class="size-6 grid place-items-center rounded-full hover:bg-muted transition-colors" aria-label="Share answer"><Share2 class="size-3"/></button>
                    <span class="ml-1 flex items-center rounded-full border bg-card overflow-hidden">
                      <button data-feedback="up" class="px-1.5 py-0.5 text-[11px] {feedbackMap[i]==='up'?'bg-signal-go/15 text-signal-go-ink':'hover:bg-muted'}" aria-label="Helpful">👍</button>
                      <button data-feedback="down" class="px-1.5 py-0.5 text-[11px] {feedbackMap[i]==='down'?'bg-signal-stop/10 text-signal-stop-ink':'hover:bg-muted'}" aria-label="Not helpful">👎</button>
                    </span>
                  </span>
                {/if}
              </div>
            </div>
          </div>
        {:else}
          <div class="grid gap-4 py-2">
            <EmptyState icon={BotMessageSquareIcon} title="Where to, traveller?" hint="">
              <p class="text-sm text-muted-foreground max-w-[32ch] text-center">Ask like you speak — I pull live NTES data, but I render static for speed.</p>
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
                    <span class="size-8 shrink-0 rounded-lg bg-saffron/12 border border-saffron/15 flex items-center justify-center text-saffron-ink group-hover:bg-primary group-hover:text-primary-foreground transition-colors max-lg:size-7">
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
        {#if busy && !streamingIdx}
          <div class="flex gap-3 chat-in">
            <div class="size-8 rounded-full bg-saffron/14 border border-saffron/20 flex items-center justify-center shrink-0 max-lg:size-7"><BotMessageSquareIcon class="size-4 max-lg:size-3.5 text-saffron-ink dark:text-saffron" /></div>
            <div class="rounded-[20px] rounded-tl-[6px] border bg-card px-4 py-3 shadow-sm flex items-center gap-3 max-lg:px-3 max-lg:py-2.5 min-w-[160px]">
              <div class="flex items-center gap-1">
                <span class="size-2 rounded-full bg-primary typing-dot" style="animation-delay:0ms"></span>
                <span class="size-2 rounded-full bg-primary typing-dot" style="animation-delay:120ms"></span>
                <span class="size-2 rounded-full bg-primary typing-dot" style="animation-delay:240ms"></span>
              </div>
              <span class="text-xs text-muted-foreground shimmer-text">Checking live data…</span>
              {#if isOffline}<span class="text-[11px] text-signal-hold-ink border border-signal-hold/20 bg-signal-hold/10 rounded-full px-1.5 py-0.5">offline cache</span>{/if}
            </div>
          </div>
        {/if}
        {#if showJump}
          <div class="sticky bottom-2 z-10 flex justify-center pointer-events-none">
            <button onclick={jumpToLatest} class="pointer-events-auto rounded-full bg-primary text-primary-foreground shadow-lg px-3 py-1.5 text-xs font-medium flex items-center gap-1.5 hover:bg-primary/90">
              <ArrowDown class="size-3.5"/> Jump to latest
            </button>
          </div>
        {/if}
      </div>

      {#if adaptiveChips?.length && turns.length>0}
        <div class="border-t bg-muted/30 px-3 py-2">
          <SuggestedCarousel chips={adaptiveChips} onPick={(p)=>sendText(p)} />
        </div>
      {/if}

      {#if streamError}
        <div class="mx-4 sm:mx-5 max-lg:mx-3">
          <div class="my-2 rounded-xl border border-signal-stop/20 bg-signal-stop/5 px-3 py-2.5 flex flex-wrap items-center gap-2 text-sm">
            <span class="size-2 rounded-full bg-signal-stop"></span>
            <span class="font-medium">Couldn't reach live data</span>
            <span class="text-muted-foreground text-xs">{streamError}</span>
            <span class="ml-auto flex items-center gap-1.5">
              <button class="rounded-full bg-card border px-3 py-1 text-xs font-medium hover:bg-accent" onclick={()=>{ streamError=null; const lastUser=[...turns].reverse().find(t=>t.role==='user'); if(lastUser) sendText(lastUser.content) }}>Retry</button>
              <button class="rounded-full bg-muted px-2 py-1 text-xs" onclick={()=>streamError=null}>Dismiss</button>
            </span>
          </div>
        </div>
      {/if}

      <div class="border-t bg-card p-3 sm:p-4 max-lg:p-3" style="padding-bottom: max(12px, env(safe-area-inset-bottom))">
        <div class="flex items-end gap-2 rounded-[24px] border border-border bg-card p-1.5 shadow-[0_8px_24px_oklch(0.235_0.028_265/0.06)] focus-within:border-ring focus-within:shadow-[0_0_0_3px_oklch(0.52_0.1_269/0.18)] transition-all">
          <button class="size-9 shrink-0 grid place-items-center rounded-full hover:bg-muted transition-colors {listening?'bg-signal-stop text-white hover:bg-signal-stop/90':''}" onclick={toggleMic} aria-label="Voice input" title="Voice">
            {#if listening}<MicOff class="size-4" />{:else}<Mic class="size-4" />{/if}
          </button>
          <Textarea
            bind:value={draft}
            placeholder="Ask about trains, PNR, seats, stations… try / for commands"
            rows={1}
            class="min-h-10 max-h-32 resize-none border-0 bg-transparent px-2 py-2.5 text-sm shadow-none focus-visible:ring-0 max-lg:text-sm max-lg:min-h-9 field-sizing-content"
            onkeydown={onKeydown}
          ></Textarea>
          <Button type="button" size="icon" class="shrink-0 rounded-full size-9 shadow-sm" onclick={() => sendText(draft)} disabled={!canSend}>
            <SendHorizontalIcon class="size-4" />
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
            <button class="hidden sm:inline-flex items-center gap-1 rounded-full border bg-muted px-2 py-1 text-[11px] text-muted-foreground hover:bg-accent" onclick={()=>paletteOpen=true}><Command class="size-3"/> Commands</button>
            <span class="hidden sm:inline text-[11px] text-muted-foreground">· static · feels live</span>
          </div>
          <div class="flex items-center gap-1.5">
            <span class="text-[11px] text-muted-foreground hidden sm:inline-flex items-center gap-1"><SearchIcon class="size-3"/> ⏎ Send · ⇧⏎ New line</span>
            <span class="text-[11px] text-muted-foreground sm:hidden">⏎ Send</span>
          </div>
        </div>
      </div>
    </Card.Content>
  </Card.Root>
  <CommandPalette bind:open={paletteOpen} onSelect={(p)=>sendText(p)} />
  <HistoryDrawer bind:open={historyOpen} turns={turns} onRestore={(p)=>sendText(p)} onClear={clearChat} />
</section>
<BottomSpacer />

<style>
  @keyframes chat-in { from { opacity:0; transform: translateY(6px) scale(0.98) } to { opacity:1; transform:none } }
  .chat-in { animation: chat-in 180ms cubic-bezier(.22,.8,.3,1) both }
  @keyframes typing-dot { 0%,80%,100% { transform: scale(0.8); opacity:0.6 } 40% { transform: scale(1); opacity:1 } }
  .typing-dot { animation: typing-dot 1.1s ease-in-out infinite }
  @keyframes shimmer { 0% { background-position: -200% 0 } 100% { background-position: 200% 0 } }
  .shimmer-text { background: linear-gradient(90deg, var(--muted-foreground) 40%, var(--foreground) 50%, var(--muted-foreground) 60%); background-size:200% 100%; -webkit-background-clip:text; background-clip:text; color:transparent; animation: shimmer 1.2s ease infinite }
  .live-cursor { display:inline-block; width:2px; height:1.1em; background: var(--primary); margin-left:1px; vertical-align: text-bottom; animation: cursor-blink 1.06s step-end infinite }
  @keyframes cursor-blink { 0%,50% { opacity:1 } 51%,100% { opacity:0 } }
  @keyframes card-stagger { from { opacity:0; transform: translateY(8px) } to { opacity:1; transform:none } }
  .card-stagger { animation: card-stagger 220ms cubic-bezier(.22,.8,.3,1) both }
  @media (prefers-reduced-motion: reduce){
    .chat-in, .typing-dot, .shimmer-text, .live-cursor, .card-stagger { animation:none !important }
  }
</style>
