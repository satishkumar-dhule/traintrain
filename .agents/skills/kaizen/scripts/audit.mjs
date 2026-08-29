#!/usr/bin/env node
// Deterministic audit — no LLM, no guessing. Emits ROI-ranked candidates.
// Each run of kaizen picks the top candidate that is >=1% and not in recent ledger.
//
// Probes encode learnings from all opencode conversations (see SKILL.md table).
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs'
import { join, sep } from 'node:path'
import { execSync } from 'node:child_process'

const ROOT = join(import.meta.dirname, '..', '..', '..', '..')
const RR = join(ROOT, 'railway-rs')
const LEDGER_PATH = join(import.meta.dirname, '..', 'ledger.json')

function sh(cmd, opts = {}) {
  try { return execSync(cmd, { encoding: 'utf8', cwd: ROOT, stdio: ['pipe','pipe','pipe'], ...opts }).trim() } catch (e) { return (e.stdout||'') + (e.stderr||'') }
}
function exists(p) { try { statSync(p); return true } catch { return false } }
function sizeOf(p) { try { return statSync(p).size } catch { return 0 } }
function countGrep(pattern, dir) { const out = sh(`grep -r "${pattern}" ${dir} --include="*.rs" --include="*.js" --include="*.svelte" --exclude-dir=node_modules --exclude-dir=target --exclude-dir=dist --exclude-dir=.svelte-kit 2>/dev/null | wc -l`); return parseInt(out.trim()||'0',10) }
function readJson(p, fallback) { try { return JSON.parse(readFileSync(p,'utf8')) } catch { return fallback } }
function mtimeOf(p) { try { return statSync(p).mtimeMs } catch { return 0 } }

const candidates = []
function push(c) { candidates.push(c) }

// ---------- ledger-aware dedup ----------
const ledger = readJson(LEDGER_PATH, { runs: [] })
const recentPicks = new Set((ledger.runs||[]).slice(-3).map(r=>r.pick))

function ledgerPenalty(id) { return recentPicks.has(id) ? 999 : 0 }

// ---------- PROBE: fmt-drift ----------
{
  const out = sh(`bash -c 'export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$HOME/.cargo"; cargo fmt --manifest-path railway-rs/Cargo.toml --all --check 2>&1 | head -80'`)
  const hasDiff = out.includes('Diff in') || out.includes('Diff at')
  const diffLines = (out.match(/\n/g)||[]).length
  if (hasDiff) {
    push({
      id: 'fmt-drift',
      dimension: 'quality/code-health',
      before: `cargo fmt diff ~${diffLines} lines`,
      after: '0 diff',
      delta_pct: 100,
      effort: 'S',
      impact: 100,
      proof: 'cargo fmt --all --check',
      fix: 'cargo fmt --all',
      roi: 10 - ledgerPenalty('fmt-drift'),
      evidence: out.slice(0,500)
    })
  }
}

// ---------- PROBE: svelte-import-audit (StatusBadge class of bug) ----------
{
  const out = sh(`node railway-rs/frontend/scripts/check-component-imports.mjs 2>&1`)
  if (out.includes('used but not imported') || out.includes('failed')) {
    push({
      id: 'svelte-import-audit',
      dimension: 'correctness',
      before: 'missing Svelte component imports (runtime ReferenceError)',
      after: '0 missing imports',
      delta_pct: 100,
      effort: 'S',
      impact: 100,
      proof: 'node railway-rs/frontend/scripts/check-component-imports.mjs',
      fix: 'add missing imports per audit output',
      roi: 20 - ledgerPenalty('svelte-import-audit'),
      evidence: out.slice(0,500)
    })
  }
}

// ---------- PROBE: stale-assets ----------
{
  const assetsDir = join(RR, 'static', 'assets')
  if (exists(assetsDir)) {
    const files = readdirSync(assetsDir)
    const indices = files.filter(f=> /^index-.*\.js$/.test(f))
    const embeds = files.filter(f=> /^embed-.*\.js$/.test(f))
    const totalBytes = files.reduce((s,f)=> s + sizeOf(join(assetsDir,f)), 0)
    // which index does index.html actually reference? those must be kept
    let htmlKeepJs = null, htmlKeepCss = null
    try {
      const html = readFileSync(join(RR,'static','index.html'),'utf8')
      const mJs = html.match(/assets\/(index-[A-Za-z0-9_-]+\.js)/)
      if (mJs && indices.includes(mJs[1])) htmlKeepJs = mJs[1]
      const mCss = html.match(/assets\/(index-[A-Za-z0-9_-]+\.css)/)
      if (mCss && files.includes(mCss[1])) htmlKeepCss = mCss[1]
    } catch {}
    const byMtime = [...indices].sort((a,b)=> mtimeOf(join(assetsDir,b)) - mtimeOf(join(assetsDir,a)))
    const newest = htmlKeepJs || (byMtime[0] || null)
    const embedByMtime = [...embeds].sort((a,b)=> mtimeOf(join(assetsDir,b)) - mtimeOf(join(assetsDir,a)))
    const embedKeep = embedByMtime[0] || null
    if (indices.length > 1 && newest) {
      const stale = indices.filter(f=> f !== newest)
      const staleBytes = stale.reduce((s,f)=> s+ sizeOf(join(assetsDir,f)), 0)
      // css: keep the one html refs; any other index-*.css not referenced is stale
      const cssFiles = files.filter(f=> f.startsWith('index-') && f.endsWith('.css'))
      const staleCss = htmlKeepCss ? cssFiles.filter(f=> f !== htmlKeepCss) : []
      const staleCssBytes = staleCss.reduce((s,f)=> s+ sizeOf(join(assetsDir,f)), 0)
      const allStaleBytes = staleBytes + staleCssBytes
      const pct = totalBytes ? ((allStaleBytes/totalBytes)*100).toFixed(1) : 0
      const keepDesc = `keep ${newest}${htmlKeepCss?` + ${htmlKeepCss}`:''}`
      push({
        id: 'stale-assets-prune',
        dimension: 'bundle/disk',
        before: `${indices.length} index js (${(totalBytes/1024).toFixed(0)}kB total) — ${keepDesc} (html ref${htmlKeepJs?' yes':', mtime newest'})`,
        after: `1 index js (keep ${newest})${htmlKeepCss?` + 1 css`:''}`,
        delta_pct: parseFloat(pct),
        effort: 'S',
        impact: allStaleBytes,
        proof: `ls -lt railway-rs/static/assets/index-*.* && grep assets/index- railway-rs/static/index.html`,
        fix: `rm stale index-*.* keep ${newest}${htmlKeepCss?` + ${htmlKeepCss}`:''}`,
        roi: 15 - ledgerPenalty('stale-assets-prune'),
        evidence: `keepJs=${newest} keepCss=${htmlKeepCss||'none'} stale=${stale.concat(staleCss).join(', ')}`
      })
    }
    if (embeds.length > 1 && embedKeep) {
      const staleEmbed = embeds.filter(f=> f !== embedKeep)
      push({
        id: 'stale-embed-prune',
        dimension: 'bundle/disk',
        before: `${embeds.length} embed bundles (keep ${embedKeep})`,
        after: `1 embed bundle (keep ${embedKeep})`,
        delta_pct: parseFloat(((staleEmbed.length/embeds.length)*100).toFixed(1)),
        effort: 'S',
        impact: 2700 * staleEmbed.length,
        proof: `ls -lt railway-rs/static/assets/embed-*.js`,
        fix: `rm stale embed-*.js keep ${embedKeep}`,
        roi: 8 - ledgerPenalty('stale-embed-prune'),
        evidence: `keep=${embedKeep} stale=${staleEmbed.join(', ')}`
      })
    }
  }
}

// ---------- PROBE: static hygiene (unreferenced css, double gzip) ----------
{
  const totalStaticKb = parseInt(sh(`du -sk railway-rs/static 2>/dev/null | cut -f1`)||'0',10)
  if (totalStaticKb > 4000) { // >4MB suggests bloat
    push({
      id: 'static-bloat',
      dimension: 'bundle/disk',
      before: `${totalStaticKb}kB static/`,
      after: `${Math.round(totalStaticKb*0.99)}kB (target -1%)`,
      delta_pct: 1.0,
      effort: 'M',
      impact: totalStaticKb*10,
      proof: 'du -sk railway-rs/static',
      fix: 'prune stale hashed assets + audit largest chunks via vite --debug',
      roi: 3 - ledgerPenalty('static-bloat'),
      evidence: `${totalStaticKb}kB`
    })
  }
}

// ---------- PROBE: a11y — 10px / --faint / contrast ----------
{
  const faint = countGrep('--faint', 'railway-rs')
  const px10 = countGrep('10px', 'railway-rs')
  const px11 = countGrep('11px', 'railway-rs')
  if (faint > 0) {
    push({ id: 'a11y-faint-token', dimension: 'ux/a11y', before: `${faint} --faint usages (fails 4.5:1)`, after: '0', delta_pct: 100, effort: 'S', impact: faint*10, proof: 'grep -r --faint railway-rs', fix: 'replace --faint with --muted per PLAN.md', roi: 9 - ledgerPenalty('a11y-faint-token'), evidence: `${faint}` })
  }
  if (px10 > 0) {
    push({ id: 'a11y-10px-font', dimension: 'ux/a11y', before: `${px10} 10px font declarations (<12px minimum)`, after: '0', delta_pct: 100, effort: 'S', impact: px10*10, proof: 'grep -r 10px railway-rs', fix: 'bump to 12px minimum per PLAN.md typography', roi: 8 - ledgerPenalty('a11y-10px-font'), evidence: `${px10}` })
  }
  if (px11 > 0) {
    push({ id: 'a11y-11px-font', dimension: 'ux/a11y', before: `${px11} 11px font declarations`, after: '0', delta_pct: 100, effort: 'S', impact: px11*5, proof: 'grep -r 11px railway-rs', fix: 'bump to 12px minimum', roi: 6 - ledgerPenalty('a11y-11px-font'), evidence: `${px11}` })
  }
}

// ---------- PROBE: unwrap in non-test src ----------
{
  const unwraps = parseInt(sh(`grep -rn "\\.unwrap()" railway-rs/src --include="*.rs" 2>/dev/null | grep -v "tests.rs" | grep -v "test_" | wc -l`).trim()||'0',10)
  // many .unwrap() in src is okay for tests but not in prod paths; threshold >5 suggests hardening headroom
  if (unwraps > 8) {
    push({ id: 'unwrap-hardening', dimension: 'reliability', before: `${unwraps} .unwrap() in src/ (non-test)`, after: `${unwraps-1}+ with expect/context`, delta_pct: (100/unwraps).toFixed(1), effort: 'M', impact: 10, proof: 'grep -rn .unwrap() railway-rs/src', fix: 'replace one unwrap with proper AppError/context', roi: 4 - ledgerPenalty('unwrap-hardening'), evidence: `${unwraps}` })
  }
}

// ---------- PROBE: TODO/FIXME ----------
{
  const todos = countGrep('TODO\\|FIXME', 'railway-rs/src')
  if (todos > 0) {
    push({ id: 'todo-prune', dimension: 'quality/code-health', before: `${todos} TODO/FIXME in src/`, after: `${todos-1}`, delta_pct: parseFloat((100/todos).toFixed(1)), effort: 'S', impact: 20, proof: 'grep -r TODO railway-rs/src', fix: 'resolve one TODO or convert to tracked issue', roi: 7 - ledgerPenalty('todo-prune'), evidence: `${todos}` })
  }
}

// ---------- PROBE: clippy warnings (if toolchain present) ----------
if (!process.env.KAIZEN_FAST) {
  // quick probe: 25s cap; run.sh treats clippy as its own gate for rust picks
  const out = sh(`bash -c 'export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$HOME/.cargo"; timeout 25 cargo clippy --manifest-path railway-rs/Cargo.toml --all-targets -- -D warnings 2>&1 | tail -40'`)
  if (out.includes('warning:') && !out.includes('0 warnings')) {
    const warnCount = (out.match(/warning:/g)||[]).length
    push({ id: 'clippy-warnings', dimension: 'quality/code-health', before: `${warnCount} clippy warnings`, after: '0', delta_pct: 100, effort: 'M', impact: warnCount*10, proof: 'cargo clippy -D warnings', fix: 'fix clippy lints one by one', roi: 7 - ledgerPenalty('clippy-warnings'), evidence: out.slice(0,400) })
  }
}

// ---------- PROBE: disk/quota ----------
{
  const quota = readJson(join(ROOT, '.space-janitor', 'status.json'), null)
  if (quota && quota.quota_pct > 70) {
    push({ id: 'disk-hygiene', dimension: 'bundle/disk', before: `${quota.quota_pct}% quota (${quota.workspace_used_gb}GB)`, after: `${(quota.quota_pct-1).toFixed(1)}% (target -1%)`, delta_pct: 1.5, effort: 'M', impact: 15, proof: 'cat .space-janitor/status.json', fix: 'prune target/tmp, stale assets, opencode.db vacuum if sqlite3 present', roi: 5 - ledgerPenalty('disk-hygiene'), evidence: JSON.stringify(quota).slice(0,300) })
  }
  // also check static bloat even without quota pressure
  const staleTarget = sh(`du -sh railway-rs/target 2>/dev/null | cut -f1`)
  if (staleTarget) {
    // informational only — not auto-pruning target in audit, but surfaced
  }
}

// ---------- PROBE: frontend build freshness ----------
{
  const idxHtml = join(RR, 'static', 'index.html')
  const assetsDir = join(RR, 'static', 'assets')
  if (exists(idxHtml) && exists(assetsDir)) {
    const html = readFileSync(idxHtml, 'utf8')
    const m = html.match(/assets\/index-([A-Za-z0-9_-]+)\.js/)
    if (m) {
      const expected = `index-${m[1]}.js`
      const hasExpected = exists(join(assetsDir, expected))
      if (!hasExpected) {
        push({ id: 'bundle-hash-mismatch', dimension: 'correctness', before: `index.html refs ${expected} but missing on disk (stale deploy)`, after: 'hash in sync after rebuild', delta_pct: 100, effort: 'S', impact: 100, proof: 'grep assets/index- static/index.html vs ls static/assets', fix: '(cd railway-rs/frontend && npm run build)', roi: 18 - ledgerPenalty('bundle-hash-mismatch'), evidence: expected })
      }
    }
  }
}

// ---------- PROBE: cargo audit / npm audit (if tools present) ----------
if (!process.env.KAIZEN_FAST) {
  const cargoAudit = sh(`cargo audit --version 2>&1 | head -1`)
  if (cargoAudit.includes('cargo audit')) {
    const auditOut = sh(`cargo audit 2>&1 | tail -20`)
    if (auditOut.toLowerCase().includes('vulnerability') || auditOut.includes('found')) {
      push({ id: 'cargo-audit-vuln', dimension: 'security/deps', before: 'cargo audit found advisories', after: '0 advisories', delta_pct: 100, effort: 'M', impact: 50, proof: 'cargo audit', fix: 'cargo update -p <crate> or patch', roi: 9 - ledgerPenalty('cargo-audit-vuln'), evidence: auditOut.slice(0,400) })
    }
  }
  if (exists(join(RR, 'frontend', 'package.json'))) {
    const npmAuditRaw = sh(`npm --prefix railway-rs/frontend audit --json 2>&1 | head -c 4000`)
    try {
      const j = JSON.parse(npmAuditRaw)
      const vulns = j.metadata?.vulnerabilities || j.vulnerabilities || {}
      const total = (vulns.critical||0)+(vulns.high||0)+(vulns.moderate||0)+(vulns.low||0)
      // also handle npm v9 shape: j.vulnerabilities is an object count
      const vulnCount = typeof total === 'number' ? total : Object.keys(vulns).length
      if (vulnCount > 0 || (j.vulnerabilities && typeof j.vulnerabilities === 'object' && Object.keys(j.vulnerabilities).length>0 && npmAuditRaw.includes('"severity"'))) {
        // double-check: auditReportVersion 2 with empty vulns has vulnerabilities={}
        const hasReal = npmAuditRaw.includes('"severity":"') || npmAuditRaw.includes('"fixAvailable"')
        if (hasReal) {
          push({ id: 'npm-audit-vuln', dimension: 'security/deps', before: `npm audit found vulns (${vulnCount || 'unknown'})`, after: '0 vulns', delta_pct: 100, effort: 'M', impact: 40, proof: 'npm --prefix railway-rs/frontend audit', fix: 'npm audit fix --prefix railway-rs/frontend', roi: 8 - ledgerPenalty('npm-audit-vuln'), evidence: npmAuditRaw.slice(0,400) })
        }
      }
    } catch {
      // if not json, fallback loose check but require severity marker
      if (npmAuditRaw.includes('"severity"') && npmAuditRaw.includes('"vulnerabilities"')) {
        push({ id: 'npm-audit-vuln', dimension: 'security/deps', before: 'npm audit found vulns', after: '0 vulns', delta_pct: 100, effort: 'M', impact: 40, proof: 'npm --prefix railway-rs/frontend audit', fix: 'npm audit fix --prefix railway-rs/frontend', roi: 8 - ledgerPenalty('npm-audit-vuln'), evidence: npmAuditRaw.slice(0,400) })
      }
    }
  }
}

// ---------- PROBE: up-to-dateness of frontend deps (informational) ----------
// ---------- PROBE: cache hit rate (needs live server) ----------
{
  const obs = sh(`curl -s localhost:3000/rail-api/observability 2>&1 | head -c 2000`)
  if (obs.includes('cache_hits') || obs.includes('cacheHits')) {
    try {
      const j = JSON.parse(obs)
      const hits = j.cache_hits ?? j.cacheHits ?? 0
      const misses = j.cache_misses ?? j.cacheMisses ?? 0
      const total = hits + misses
      const rate = total ? (hits/total)*100 : 0
      if (total > 20 && rate < 30) {
        push({ id: 'cache-hit-rate', dimension: 'reliability', before: `${rate.toFixed(1)}% hit rate (${hits}/${total})`, after: `${(rate+1).toFixed(1)}% (target +1%)`, delta_pct: 1, effort: 'M', impact: 20, proof: 'curl localhost:3000/rail-api/observability', fix: 'tune RAILWAY_CACHE_TTL or add cache key for hottest path', roi: 6 - ledgerPenalty('cache-hit-rate'), evidence: obs.slice(0,300) })
      }
    } catch {}
  }
}

// ---------- FALLBACK: always innovates (never 0 candidates) ----------
// If every probe is green the app is at a local optimum — still propose a
// safe, measurable innovation so the autonomous loop never stalls.
if (candidates.length === 0) {
  const testCountRaw = sh(`cargo test --manifest-path railway-rs/Cargo.toml -- --list 2>&1 | grep -c "^test "`)
  const testCount = parseInt(testCountRaw.trim()||'0',10) || parseInt(sh(`grep -r "#\\[test" railway-rs --include="*.rs" 2>/dev/null | wc -l`).trim()||'0',10) || 0
  push({
    id: 'innovate-tests',
    dimension: 'dx',
    before: `${testCount} tests`,
    after: `${testCount+1} tests (+1)`,
    delta_pct: testCount ? parseFloat((100/testCount).toFixed(1)) : 100,
    effort: 'M',
    impact: 10,
    proof: 'cargo test -- --list 2>&1 | wc -l',
    fix: 'add one focused unit or integration test for the least-covered module',
    roi: 1 - ledgerPenalty('innovate-tests'),
    evidence: `${testCount} tests`
  })
}

// ---------- rank ----------
candidates.sort((a,b)=> (b.roi - a.roi) || (b.impact - a.impact))

const args = process.argv.slice(2)
const asJson = args.includes('--json')
const doFix = args.includes('--fix')

if (candidates.length === 0) {
  const msg = { status: 'no-candidates', message: 'no >=1% improvement found with current probes — add a new probe or widen thresholds', candidates: [] }
  if (asJson) console.log(JSON.stringify(msg, null, 2))
  else console.log('✓ audit: no candidates — app is at local optimum for current probes (add probe to keep improving)')
  process.exit(2)
}

if (asJson) {
  console.log(JSON.stringify({ status: 'ok', count: candidates.length, top: candidates[0], candidates }, null, 2))
} else {
  console.log(`audit: ${candidates.length} candidate(s) (ROI-ranked):\n`)
  candidates.forEach((c,i)=>{
    const flag = recentPicks.has(c.id) ? ' (recent — deprioritized)' : ''
    console.log(`${i+1}. [${c.dimension}] ${c.id}${flag} — ROI ${c.roi}`)
    console.log(`   before: ${c.before}`)
    console.log(`   after:  ${c.after}  (Δ ${c.delta_pct}%)`)
    console.log(`   proof:  ${c.proof}`)
    console.log(`   fix:    ${c.fix}`)
    console.log(`   evidence: ${String(c.evidence).slice(0,120)}\n`)
  })
  console.log(`→ pick: ${candidates[0].id} — ${candidates[0].dimension} — ${candidates[0].before} → ${candidates[0].after}`)
}

if (doFix) {
  // only auto-fix the safest hygiene items; everything else needs human/agent implementation
  const autoFixable = new Set(['fmt-drift', 'stale-assets-prune', 'stale-embed-prune'])
  const top = candidates[0]
  if (!autoFixable.has(top.id)) {
    console.log(`\n--fix: top candidate ${top.id} is not auto-fixable; implement manually per SKILL.md phase 3`)
    process.exit(0)
  }
  console.log(`\n--fix: auto-fixing ${top.id} ...`)
  if (top.id === 'fmt-drift') {
    const beforeDirty = sh('git status --porcelain | grep "\\.rs$" | cut -c4- | head -20')
    if (beforeDirty.trim()) {
      console.log(`fmt-drift auto-fix skipped — ${beforeDirty.split('\n').filter(Boolean).length} dirty .rs file(s) in working tree (would churn user work)`)
      console.log('manual: run `cargo fmt --all` when your tree is clean, or fmt the specific file you touched')
      process.exit(0)
    }
    execSync(`bash -c 'export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$HOME/.cargo"; cargo fmt --manifest-path railway-rs/Cargo.toml --all'`, { stdio: 'inherit', cwd: ROOT })
    console.log('fmt applied')
  } else if (top.id === 'stale-assets-prune') {
    const assetsDir = join(RR, 'static', 'assets')
    const files = readdirSync(assetsDir)
    const indices = files.filter(f=> /^index-.*\.js$/.test(f))
    let keepJs = null, keepCss = null
    try {
      const html = readFileSync(join(RR,'static','index.html'),'utf8')
      const mJs = html.match(/assets\/(index-[A-Za-z0-9_-]+\.js)/)
      if (mJs && indices.includes(mJs[1])) keepJs = mJs[1]
      const mCss = html.match(/assets\/(index-[A-Za-z0-9_-]+\.css)/)
      if (mCss && files.includes(mCss[1])) keepCss = mCss[1]
    } catch {}
    if (!keepJs) {
      const byMtime = [...indices].sort((a,b)=> mtimeOf(join(assetsDir,b)) - mtimeOf(join(assetsDir,a)))
      keepJs = byMtime[0]
    }
    if (!keepJs) { console.error('no keep candidate found'); process.exit(1) }
    const keepSet = new Set([keepJs, keepCss].filter(Boolean))
    console.log(`keeping ${[...keepSet].join(' + ')}`)
    for (const f of files) {
      if (f.startsWith('index-') && !keepSet.has(f)) {
        const p = join(assetsDir, f)
        console.log(`rm ${p}`)
        execSync(`rm -f "${p}"`)
      }
    }
    const html = readFileSync(join(RR,'static','index.html'),'utf8')
    if (!html.includes(keepJs)) {
      console.warn(`warning: index.html does not ref ${keepJs}; run (cd railway-rs/frontend && npm run build) to resync`)
    }
    if (keepCss && !html.includes(keepCss)) {
      console.warn(`warning: index.html does not ref ${keepCss}`)
    }
  } else if (top.id === 'stale-embed-prune') {
    const assetsDir = join(RR, 'static', 'assets')
    const files = readdirSync(assetsDir).filter(f=> /^embed-.*\.js$/.test(f))
    let keep = null
    // prefer newest by mtime
    const byMtime = [...files].sort((a,b)=> mtimeOf(join(assetsDir,b)) - mtimeOf(join(assetsDir,a)))
    keep = byMtime[0]
    for (const f of files) {
      if (f !== keep) {
        const p = join(assetsDir,f)
        console.log(`rm ${p}`)
        execSync(`rm -f "${p}"`)
      }
    }
    console.log(`kept ${keep}`)
  }
}
