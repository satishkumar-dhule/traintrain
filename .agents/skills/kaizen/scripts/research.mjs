#!/usr/bin/env node
// Research phase for kaizen — LLM-proposed improvement aspects.
//
// Flow: build codebase digest -> ask an LLM for NEW candidate ideas -> prove
// each idea with a real shell command -> build an idea bank (dedup) -> emit a
// validated candidate pool in the same JSON shape as audit.mjs.
//
// "The LLM proposes, the machine proves, the agent implements." A proposal with
// no passing proof command never enters the pool, so no unfabricated numbers.
//
// Providers (first healthy wins):
//   1. KAIZEN_LLM_BASE / KAIZEN_LLM_MODEL / KAIZEN_LLM_API_KEY  (any OpenAI-compatible)
//   2. GitHub Copilot via opencode's stored oauth token (~/.local/share/opencode/auth.json)
//   3. the running app relay  POST localhost:3000/rail-api/ai/chat (SSE, last resort)
//
// This script NEVER prints a token/key.
import { readFileSync, writeFileSync, existsSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { execSync, spawnSync } from 'node:child_process'
import crypto from 'node:crypto'

const ROOT = join(import.meta.dirname, '..', '..', '..', '..')
const RR = join(ROOT, 'railway-rs')
const BANK = join(import.meta.dirname, '..', 'ideas-bank.json')
const MAX_LLM_SECS = 120

function sh(cmd, opts = {}) {
  try { return { ok: true, out: execSync(cmd, { encoding: 'utf8', cwd: ROOT, timeout: 12000, stdio: ['pipe','pipe','pipe'], ...opts }).trim() } }
  catch (e) { return { ok: false, out: (e.stdout||'') + '\n' + (e.stderr||''), err: e } }
}

// ---------------------------------------------------------------------------
// Proof-command sandbox. LLM-supplied `proof_command` runs with full user
// privileges, so it is gated to a read-only allowlist BEFORE execution:
//   - only known read-only tools per pipe segment
//   - no destructive git/subprocess/build tools, no `-e`/`-c` code exec
//   - no file redirection except `>/dev/null`, no brace/command-subst chains
// ---------------------------------------------------------------------------
const SAFE_PROGRAMS = new Set([
  'grep','rg','egrep','fgrep','wc','ls','du','cat','head','tail','cut','sort','uniq','tr',
  'sed','awk','nl','expr','bc','printf','echo','env','date','stat','file','basename','dirname',
  'seq','true','false','git','find','node','curl','jq','sha1sum','md5sum','readlink','realpath',
])
const GIT_READONLY = new Set(['log','status','diff','rev-parse','show','ls-files','shortlog','grep','remote'])
const CMD_WORDS = /^\s*([A-Za-z_][A-Za-z0-9_]*)/

function stripQuotes(s) {
  s = s.trim()
  if (s.length >= 2) {
    const q = s[0]
    if ((q === '"' || q === "'") && s.endsWith(q)) return s.slice(1,-1)
  }
  return s
}

function denyProof(cmd, why) {
  return { allowed:false, why }
}

function proofAllowed(cmd) {
  const c = String(cmd||'').trim()
  if (!c) return denyProof(c, 'empty')
  if (c.length > 400) return denyProof(c, 'too-long')

  // 1) destructive / privilege / build tools — never permitted. Standalone
  //    tokens only (lookbehind: a flag like `-sh` or path `pwd.go` is NOT a tool).
  const banned = /(?<![\w-])(rm|mv|cp|dd|touch|truncate|tee|shred|mkfs[\w.]*|chmod|chown|chgrp|ln|mount|umount|fdisk|tar|gzip|bzip2|xz|zip|unzip|7z|rar|scp|rsync|rclone|install|sudo|su|kill|pkill|killall|pwd|whoami|useradd|usermod|passwd|make|cargo|npm|npx|pnpm|yarn|deno|bun|python|python3|perl|ruby|bash|zsh|sh)\b/.test(c)
  if (banned) return denyProof(c, 'banned-tool')

  // 2) git only in read-only forms.
  if (/\bgit\s+[a-z@]/.test(c)) {
    const gm = c.match(/\bgit\s+([a-z@-]+)/)
    if (!gm || !GIT_READONLY.has(gm[1])) return denyProof(c, 'git-non-readonly')
  }

  // 3) shell metacharacter / substitution / mutation-flag injection.
  if (/[`;]/.test(c) || /\$\(/.test(c) || /\$\{/.test(c)) return denyProof(c, 'code-injection')
  if (/[\s;|&](-delete|-exec|-ok)\b/.test(c)) return denyProof(c, 'mutate-flag')
  if (/\b(node|curl|find|awk|sed)\s+(-e|-c|--execute)\b/.test(c)) return denyProof(c, 'inline-exec')
  if (/\bcurl\b[^|]*\s(-o|-O|--output|--output-dir|--create-dirs|--proxy)\b/.test(c)) return denyProof(c, 'curl-write')
  if (/\bsed\s+-i\b/.test(c)) return denyProof(c, 'sed-inplace')
  if (/\b(node|curl|git)\b[\s]*[|><]\s*(>|\|)\s*\S+/.test(c)) return denyProof(c, 'pipe-to-write')

  // 4) file redirection: only `N>/dev/null`, `N>&M`, `N>&-` forms allowed.
  const stripped = c
    .replace(/[0-9]?[<>]\/dev\/null/g, '')   // 2>/dev/null, >/dev/null
    .replace(/[0-9]?[<>]&[0-9]?/g, '')       // 2>&1, 1>&2, 2>&-, >&1
  if (/[<>]/.test(stripped)) return denyProof(c, 'file-redirect')
  // 5) `|` pipe is fine (allows grep | wc), but a bare `|` joining to a
  //    non-pipe segment is rejected by per-segment tool checks below.

  // 6) per-pipe-segment allowlist: first token of each segment is the binary.
  const segments = c.split(/\s*\|\s*/)
  for (const seg of segments) {
    let s = seg.trim()
    // strip leading env assignments (FOO=bar tool ...) — harmless for read-only tools
    while (/^[A-Za-z_][A-Za-z0-9_]*=(\S+)?\s/.test(s)) s = s.replace(/^[A-Za-z_][A-Za-z0-9_]*=(\S+)?\s/, '')
    const m = CMD_WORDS.exec(s)
    const prog = stripQuotes(m ? m[1] : '')
    if (!SAFE_PROGRAMS.has(prog)) return denyProof(c, `unsafe-tool:${prog||'?'}`)
    if (prog === 'node') {
      // node must run a repo script, never `-e`/`-p` code or stdin
      const body = s.slice(m[0].length).trim()
      if (/-e\b|-p\b|--eval/.test(body)) return denyProof(c, 'node-eval')
      if (!/\.(mjs|js|cjs)$/.test(stripQuotes(body.split(/\s+/)[0]||''))) return denyProof(c, 'node-not-script')
    }
    if (prog === 'curl') {
      const body = s.slice(m[0].length)
      if (!/\blocalhost(:|\s|\/)|127\.0\.0\.1/.test(body)) return denyProof(c, 'curl-nonlocal')
    }
  }

  return { allowed:true }
}
function exists(p) { try { statSync(p); return true } catch { return false } }
function readJson(p, fb) { try { return JSON.parse(readFileSync(p,'utf8')) } catch { return fb } }
function writeJson(p, o) { writeFileSync(p, JSON.stringify(o, null, 2) + '\n') }

// ---------------------------------------------------------------------------
// Digest — compact, factual, all produced by commands on THIS machine.
// ---------------------------------------------------------------------------
function slugify(s) { return String(s||'').toLowerCase().replace(/[^a-z0-9]+/g,'-').replace(/^-+|-+$/g,'').slice(0,60) }

function buildDigest() {
  const parts = []
  parts.push(`# kaizen research digest for ${ROOT}`)
  parts.push(`generated: ${new Date().toISOString()}`)

  const gitLog = sh('git log --oneline -8').out
  if (gitLog) parts.push(`## recent commits\n${gitLog}`)

  // rust surface
  const slices = exists(join(RR,'src','slices')) ? readdirSync(join(RR,'src','slices')).filter(f=>f.endsWith('.rs')||!f.includes('.')) : []
  const sliceLoc = slices.map(s => {
    const d = join(RR,'src','slices', s)
    let n = 0
    try { if (statSync(d).isDirectory()) for (const f of readdirSync(d)) if (f.endsWith('.rs')) n += readFileSync(join(d,f),'utf8').split('\n').length
          else n += readFileSync(d,'utf8').split('\n').length } catch {}
    return `${s}:${n}`
  })
  parts.push(`## rust slices (name:loc)\n${sliceLoc.join('\n') || 'none'}`)

  // frontend surface
  const lib = join(RR,'frontend','src','lib')
  if (exists(lib)) {
    const walk = (d) => readdirSync(d,{withFileTypes:true}).flatMap(e => {
      const p = join(d,e.name); if (e.isDirectory()) return walk(p); return /\.(svelte|js)$/.test(e.name)?[p]:[]
    })
    const files = walk(lib).sort((a,b)=> statSync(b).size - statSync(a).size).slice(0,14)
    parts.push(`## largest frontend files (top 14 by bytes)\n${files.map(f=>`${f.replace(RR+'/','')}:${statSync(f).size}`).join('\n')}`)
  }

  // static artifacts
  const assets = join(RR,'static','assets')
  if (exists(assets)) {
    const js = readdirSync(assets).filter(f=>/^index-.*\.js$/.test(f)).map(f=>`${f}:${statSync(join(assets,f)).size}`)
    parts.push(`## static index bundles\n${js.join('\n') || 'none'}`)
  }

  // quality counters (deterministic probes)
  const counters = {
    clippy_warnings: parseInt(sh(`grep -rn "warning:" railway-rs/src --include="*.rs" 2>/dev/null | wc -l`).out||'0'),
    todos: parseInt(sh(`grep -rn "TODO\\|FIXME" railway-rs/src --include="*.rs" 2>/dev/null | wc -l`).out||'0'),
    unwraps: parseInt(sh(`grep -rn "\\.unwrap()" railway-rs/src --include="*.rs" 2>/dev/null | grep -v tests.rs | wc -l`).out||'0'),
    px10: parseInt(sh(`grep -rn "10px" railway-rs/frontend/src railway-rs/static --include="*.svelte" --include="*.js" --include="*.css" 2>/dev/null | wc -l`).out||'0'),
    tests: parseInt(sh(`grep -rc "\\#\\[tokio::test\\]\\|\\#\\[test\\]" railway-rs/tests railway-rs/src --include="*.rs" 2>/dev/null | awk -F: '{s+=$2} END{print s}'`).out||'0'),
    index_bundles: parseInt(sh(`ls railway-rs/static/assets/index-*.js 2>/dev/null | wc -l`).out||'0'),
  }
  parts.push(`## quality counters\n${JSON.stringify(counters, null, 1)}`)

  // running server observability (real runtime facts, if up)
  const obs = sh(`curl -s --max-time 3 localhost:3000/rail-api/observability 2>&1 | head -c 1500`)
  if (obs.ok && obs.out.startsWith('{')) parts.push(`## live observability\n${obs.out.slice(0,1200)}`)

  return parts.join('\n\n')
}

// ---------------------------------------------------------------------------
// LLM providers
// ---------------------------------------------------------------------------
async function httpJson(url, headers, body, timeoutMs) {
  // node >=18 has fetch built-in
  const ctl = new AbortController()
  const t = setTimeout(()=>ctl.abort(), timeoutMs)
  try {
    const r = await fetch(url, { method:'POST', headers, body: JSON.stringify(body), signal: ctl.signal })
    const txt = await r.text()
    return { ok: r.ok, status: r.status, txt }
  } catch (e) { return { ok:false, status:0, txt:String(e) } }
  finally { clearTimeout(t) }
}

function copilotToken() {
  const p = join(ROOT, '.local', 'share', 'opencode', 'auth.json')
  if (!exists(p)) return null
  try { return JSON.parse(readFileSync(p,'utf8'))['github-copilot']?.access || null } catch { return null }
}

async function pingProvider(p) {
  const r = await httpJson(p.base.replace(/\/$/,'') + '/chat/completions', {
    'Content-Type':'application/json',
    ...(p.key ? { 'Authorization': `Bearer ${p.key}` } : {}),
  }, { model: p.model, messages:[{role:'user',content:'Say exactly: PONG'}], max_tokens:8, stream:false }, 15000)
  if (!r.ok) return { healthy:false, reason:`http ${r.status} ${r.txt.slice(0,120)}` }
  try {
    const j = JSON.parse(r.txt)
    const content = j?.choices?.[0]?.message?.content || ''
    return { healthy: content.includes('PONG'), reason: content.slice(0,60) || '(empty)' }
  } catch { return { healthy:false, reason:'bad json' } }
}

async function chatProvider(p, messages, maxTokens) {
  const r = await httpJson(p.base.replace(/\/$/,'') + '/chat/completions', {
    'Content-Type':'application/json',
    ...(p.key ? { 'Authorization': `Bearer ${p.key}` } : {}),
  }, { model: p.model, messages, max_tokens: maxTokens, stream:false, temperature: 0.4 }, MAX_LLM_SECS*1000)
  if (!r.ok) throw new Error(`provider ${p.name} http ${r.status}: ${r.txt.slice(0,200)}`)
  const j = JSON.parse(r.txt)
  return j?.choices?.[0]?.message?.content || ''
}

function buildProviders() {
  const providers = []
  if (process.env.KAIZEN_LLM_BASE) {
    providers.push({
      name: 'env',
      base: process.env.KAIZEN_LLM_BASE,
      model: process.env.KAIZEN_LLM_MODEL || 'gpt-4.1',
      key: process.env.KAIZEN_LLM_API_KEY || null,
    })
  }
  const tok = copilotToken()
  if (tok) providers.push({ name: 'copilot', base: 'https://api.githubcopilot.com', model: 'gpt-4.1', key: tok })
  return providers
}

// ---------------------------------------------------------------------------
// Idea bank (dedup across runs)
// ---------------------------------------------------------------------------
const bank = readJson(BANK, { version:1, ideas: [] })
const bankFps = new Set((bank.ideas||[]).map(i=>i.fp))

function fpOf(id, proof) { return crypto.createHash('sha1').update(`${id}::${proof}`).digest('hex').slice(0,16) }

// ---------------------------------------------------------------------------
// Validation: run the LLM's proof command, require a real match.
// ---------------------------------------------------------------------------
function validateIdea(raw, recentPicks) {
  const id = slugify(raw.id||raw.title)
  if (!id) return null
  const title = String(raw.title||raw.id||'').trim().slice(0,120)
  if (!title) return null
  const proof = String(raw.proof_command||'').trim()
  if (!proof) return null
  const regexRaw = String(raw.proof_match_regex||'').trim()
  if (!regexRaw) return null
  const direction = String(raw.metric_direction||'').trim()
  if (direction !== 'up' && direction !== 'down') return null
  const dim = String(raw.dimension||'').trim().toLowerCase()
  const allowed = new Set(['perf-latency','perf-throughput','bundle','correctness','quality','ux-a11y','dx','reliability','security-deps','disk-deploy','perf','perf/throughput','perf/latency','bundle/disk','ux/a11y','security/deps'])
  if (!allowed.has(dim)) return null
  const fp = fpOf(id, proof)
  if (bankFps.has(fp)) return { skip:'bank' }
  if ((recentPicks||[]).includes(id)) return { skip:'recent' }

  const safe = proofAllowed(proof)
  if (!safe.allowed) return { skip:'unsafe-proof', why: safe.why }

  const r = sh(proof)
  if (!r.ok) return { skip:'proof-failed', stderr: r.out.slice(0,160) }
  let re
  try { re = new RegExp(regexRaw) } catch { return { skip:'bad-regex' } }
  const m = r.out.match(re)
  if (!m) return { skip:'proof-no-match', out: r.out.slice(0,160) }

  const metricBefore = (m[1] ?? m[0] ?? r.out).trim().slice(0,120)
  const numBefore = parseFloat(metricBefore)
  if (direction === 'down' && Number.isFinite(numBefore) && numBefore < 1) return { skip:'empty-proof', metric_before: metricBefore }
  // est. delta from LLM when given, else assume 1 (proof of issue is the bar)
  const estDelta = Math.max(Math.abs(parseFloat(raw.estimated_delta_pct)||0), 1)
  const effort = String(raw.effort||'M').toUpperCase()
  const risk = String(raw.risk||'low').toLowerCase()
  const roi = { S:6, M:4, L:1, X:2 }[effort] || 2 + (risk==='low'?3:risk==='med'?1:0)
  return {
    id, title, dimension: dim, direction,
    hypothesis: String(raw.hypothesis||'').trim().slice(0,240),
    metric: String(raw.metric||dim).trim().slice(0,80),
    proof_command: proof,
    proof_match_regex: regexRaw,
    metric_before: metricBefore,
    estimated_delta_pct: estDelta,
    effort, risk,
    roi,
    fix_hint: String(raw.fix_hint||'').trim().slice(0,240),
    source: 'llm',
    fp,
  }
}

// ---------------------------------------------------------------------------
// Prompt
// ---------------------------------------------------------------------------
function buildPrompt(digest, deterministicCands, ledgerRuns) {
  const recent = ledgerRuns.slice(-4)
  const schema = `{
  "ideas": [
    {
      "id": "kebab-slug",
      "title": "short human title",
      "dimension": "one of: bundle, correctness, quality, ux/a11y, dx, reliability, perf/throughput, perf/latency, security/deps, disk-deploy",
      "metric": "exact measurable quantity (name it)",
      "metric_direction": "up" | "down",
      "hypothesis": "what is currently suboptimal and why, grounded in the digest",
      "proof_command": "one bash command, runnable from the workspace root, that prints a number/proof of the issue; must not mutate files",
      "proof_match_regex": "regex applied to that command's stdout; put the number you care about in capture group 1",
      "estimated_delta_pct": 1.5,
      "effort": "S" | "M" | "L",
      "risk": "low" | "med",
      "fix_hint": "concrete implementation sketch (<=200 chars)"
    }
  ]
}`
  const prompt = `You are the research arm of a continuous-improvement system for a Rust/axum + Svelte 5 Indian-Railways app (railway-rs). Your job: propose NEW improvement aspects the deterministic scan missed.

RULES (non-negotiable):
- Propose only ideas whose "proof_command" verifiably demonstrates the problem on this machine and whose fix can be smaller than ~100 lines with ZERO behavioral regression.
- Do NOT propose adding dependencies to fix a lint. Do NOT touch the test harness (tests/common). Do NOT weaken quality gates.
- Every metric must be honest: if the app already measures it, reuse that; never invent a payoff you cannot show with the proof command.
- Prefer ideas anchored in the digest: recent commits, big files, counters, observability numbers, bundle sizes.
- Do NOT reuse dimensions/picks from the "already done" list or recent ledger.
- estimated_delta_pct: your honest guess of the % gain, default 1.

DIGEST:
${digest}

ALREADY-DONE (do not re-propose these):
${JSON.stringify(deterministicCands.map(c=>c.id))}

RECENT LEDGER RUNS (do not repeat the same dimension as the last 2):
${JSON.stringify(recent.map(r=>`#${r.run} ${r.pick} (${r.dimension})`))}

Return 6-8 ideas as STRICT JSON (no prose, no markdown fences) matching:
${schema}`
  return prompt
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
const args = process.argv.slice(2)
const asJson = args.includes('--json')

let DET_CANDS = []
function fail(msg, code=2) {
  if (asJson) console.log(JSON.stringify({ status:'research-unavailable', count:0, count_deterministic: DET_CANDS.length, candidates: DET_CANDS, message:msg }, null, 2))
  else console.log(`research: ${msg}`)
  process.exit(code)
}

function getDeterministicCandidates() {
  // deterministic candidates to avoid duplication; reuse run.sh's fresh scan via env when present
  if (process.env.KAIZEN_AUDIT_JSON) {
    try { const d = JSON.parse(process.env.KAIZEN_AUDIT_JSON); return (d.candidates||[]).filter(c=>c.id) } catch {}
    return []
  }
  const detRaw = sh(`node .agents/skills/kaizen/scripts/audit.mjs --json 2>&1`, { timeout: 40000 })
  try { const d = JSON.parse(detRaw.out); return (d.candidates||[]).filter(c=>c.id) } catch { return [] }
}

async function main() {
  DET_CANDS = getDeterministicCandidates()
  const detCands = DET_CANDS
  const ledger = readJson(join(import.meta.dirname, '..', 'ledger.json'), { runs: [] })
  const recentPicks = (ledger.runs||[]).slice(-3).map(r=>r.pick)

  const digest = buildDigest()
  const providers = buildProviders()
  if (providers.length === 0) fail('no LLM provider available — set KAIZEN_LLM_BASE/KAIZEN_LLM_MODEL/KAIZEN_LLM_API_KEY (an OpenAI-compatible endpoint) to enable research')

  const prompt = buildPrompt(digest, detCands, ledger.runs||[])
  const messages = [{ role:'user', content: prompt }]

  let content = ''
  let usedProvider = null
  for (const p of providers) {
    const ping = await pingProvider(p)
    if (!ping.healthy) { console.error(`provider ${p.name} not healthy: ${ping.reason}`); continue }
    try { content = await chatProvider(p, messages, 2200); usedProvider = p.name; break }
    catch (e) { console.error(`provider ${p.name} chat failed: ${e.message}`) }
  }
  if (!usedProvider) fail('no LLM provider responded (tried env + copilot) — run again later or set KAIZEN_LLM_* env')

  // strip fences if LLM wrapped JSON
  let raw
  try { raw = JSON.parse(content.replace(/```json|```/g,'')) }
  catch {
    const m = content.match(/\{[\s\S]*\}/)
    if (!m) fail(`LLM returned non-JSON (${usedProvider})`)
    try { raw = JSON.parse(m[0]) } catch { fail(`LLM returned unparseable JSON (${usedProvider})`) }
  }
  const ideas = Array.isArray(raw) ? raw : Array.isArray(raw.ideas) ? raw.ideas : null
  if (!ideas) fail(`LLM returned no ideas array (${usedProvider})`)

  const validated = []
  const rejected = []
  for (const idea of ideas) {
    const v = validateIdea(idea, recentPicks)
    if (!v) { rejected.push({ id: idea?.id||'?', reason:'schema' }); continue }
    if (v.skip) { rejected.push({ id: idea?.id||String(v.skip), reason:v.skip }); continue }
    validated.push(v)
  }

  // --- self-heal round: let the LLM fix proofs that failed validation once ---
  const repairable = []
  for (const r of rejected) {
    const idea = ideas.find(i => (i?.id||'?') === r.id)
    if (!idea || !idea.proof_command || !idea.proof_match_regex) continue
    if (r.reason !== 'proof-failed' && r.reason !== 'proof-no-match' && r.reason !== 'empty-proof') continue
    repairable.push({ ...r, idea })
  }
  if (repairable.length && usedProvider) {
    try {
      const repair = await chatProvider(providers.find(p=>p.name===usedProvider), [
        ...messages,
        { role: 'assistant', content },
        { role:'user', content:
`Some of your ideas were REJECTED because their proof_command failed on this machine. Fix ONLY the proof_command and proof_match_regex (and optionally estimated_delta_pct). Strict JSON array of objects keyed by id:

${repairable.map(r=>JSON.stringify({ id: r.id, problem: r.reason,
  implied_by_stdout: r.out || 'no output' })).join('\n')}

Rules:
- proof_command: one bash command, runnable from the workspace root, NOT mangling files, that prints evidence the problem actually exists. Prefer: grep -rc / grep -rn / wc -l / du / git diff --stat / ls / curl on localhost:3000 / node scripts/... Small precise commands only.
- proof_match_regex: regex applied to that command's stdout; the quantity you care about in capture group 1. Use a plain number match like "^\\d+" style only if truly unanchored; prefer anchored patterns like "^(\\d+)$" or "grep ... | wc -l" which prints a bare number.
- If you cannot construct a valid proof on this machine, return an empty array for that id (omit it).
Return strict JSON array only.` },
      ], 1600)
      const fixChunk = repair.match(/\[[\s\S]*\]/)
      if (fixChunk) {
        for (const fx of JSON.parse(fixChunk[0])) {
          const orig = ideas.find(i => (i?.id||'?') === fx.id)
          if (!orig || !fx.proof_command || !fx.proof_match_regex) continue
          const fixed = { ...orig, proof_command: fx.proof_command, proof_match_regex: fx.proof_match_regex, estimated_delta_pct: fx.estimated_delta_pct || orig.estimated_delta_pct, _repaired: true }
          const v = validateIdea(fixed, recentPicks)
          if (v && !v.skip) { validated.push(v); console.error(`research: repaired idea ${fx.id}`) }
          else rejected.push({ id: fx.id, reason: v?.skip || 'proof-still-fails' })
        }
      }
    } catch (e) { console.error(`research: repair round failed: ${e.message}`) }
  }

  // rotate bank; persist new validated ideas
  if (validated.length) {
    const add = validated.map(v=>({ fp:v.fp, id:v.id, title:v.title, dimension:v.dimension, metric_before:v.metric_before, proof_command:v.proof_command, source:'llm', provider:usedProvider, ts:new Date().toISOString() }))
    const cur = (bank.ideas||[]).filter(i=>!validated.some(v=>v.fp===i.fp))
    bank.ideas = [...cur, ...add].slice(-50)
    writeJson(BANK, bank)
  }
  // also persist rejected-proof info to help agents sharpen prompts
  if (rejected.length) console.error(`research: rejected ${rejected.length} idea(s): ${JSON.stringify(rejected.slice(0,8))}`)

  // merge with deterministic candidates (research ideas get top billing by roi)
  const merged = [...detCands, ...validated]
  const seen = new Set()
  const deduped = merged.filter(c=> (seen.has(c.id) ? false : (seen.add(c.id), true)))

  if (validated.length === 0) fail(`research: 0 validated LLM ideas (${usedProvider}); deterministic pool has ${detCands.length}`)

  const out = { status:'ok', provider: usedProvider, digest_bytes: digest.length,
    count_llm: validated.length, count_deterministic: detCands.length,
    candidates: deduped,
    top: { ...(deduped.find(c=>c.source==='llm') || deduped[0]), source: 'llm' } }
  if (asJson) console.log(JSON.stringify(out, null, 2))
  else {
    console.log(`research: ${usedProvider} proposed ${ideas.length}, validated ${validated.length}, deterministic ${detCands.length}\n`)
    validated.forEach((v,i)=> console.log(`${i+1}. [${v.dimension}] ${v.id} — ${v.metric} ${v.direction} (est ${v.estimated_delta_pct}%) — ${v.title}\n   proof: ${v.proof_command}\n   → ${v.metric_before}\n   fix:   ${v.fix_hint}\n`))
  }
}

main().catch(e => fail(`research error: ${e.message}`, 1))