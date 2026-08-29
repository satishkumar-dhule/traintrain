---
name: kaizen
description: General-purpose continuous improvement loop — each run delivers >=1% measurable gain on one dimension with zero degradation. Use for "improve app", "kaizen", "1% better", "continuous improvement", "optimize", "harden", "polish", "tech debt", or any "make it better without breaking" request. Covers perf, bundle, correctness, UX/a11y, DX, reliability, security, docs. Never degrades gated metrics.
---

# Kaizen — 1% per run, zero degradation

Each invocation makes **exactly one** small, provably-better change. The app gets monotonically better; repeated runs never undo progress and never introduce regressions.

## Invariant (non-negotiable)

```
post_gates  >= pre_gates        // all gates pass equally or better
post_metric >= pre_metric + 1%  // at least one dimension +1% (or +1 absolute when % is undefined)
no_other_dimension_degraded     // no metric moves worse beyond tolerance (0% for correctness, <0.2% noise for perf)
```

If the invariant cannot be met, the run **aborts with no commit** and reports why.

## When to use

- User says "improve by 1%", "make it better", "kaizen", "continuous improvement", "optimize", "polish", "harden", "tech debt"
- Scheduled CI/cron that runs the agent nightly
- After any feature lands and you want to bank a safe follow-up win
- Before a release to squeeze the last 1%

Do NOT use for:
- New features / vertical slices (use `vertical-slice-rust`)
- Emergency hotfixes (fix directly)
- Exploratory "what if" spikes (use `brainstorming`)

## Measurable dimensions (one per run, pick exactly one)

| # | Dimension | Metric (higher=better unless noted) | How measured | 1% threshold |
|---|-----------|--------------------------------------|--------------|--------------|
| 1 | **Perf — latency** | p50/p95 handler latency ms (lower=better) | `state.metrics` EMA or `curl -w %{time_total}` | -1% ms or -5ms absolute |
| 2 | **Perf — throughput** | req/s or build time (lower=better for build) | `cargo build` time, `metrics.req_per_sec` | +1% RPS or -1% build secs |
| 3 | **Bundle — JS/CSS** | gzip bytes (lower=better) | `ls -lh static/assets/index-*.js`, `gzip -c` | -1% bytes or -1kB |
| 4 | **Correctness** | bug count, error ratio, failing tests (lower=better) | `cargo test`, `cargo clippy`, issue list | -1 bug or -1% error ratio, 0 failures |
| 5 | **Quality — code health** | clippy warnings, fmt diffs, dead code, TODOs (lower=better) | `cargo clippy`, `cargo fmt --check`, `grep -r TODO` | -1 warning or -1 TODO |
| 6 | **UX / a11y** | Lighthouse/a11y violations (lower=better), contrast pass rate | `axe` / `lighthouse` / manual contrast check | -1 violation or +1% score |
| 7 | **DX — docs & tests** | test count, coverage, docs coverage (higher=better) | `cargo test -- --list`, `cargo llvm-cov` if present | +1 test or +1% coverage |
| 8 | **Reliability** | cache hit rate, availability SLI, SLO error budget | `metrics.snapshot().slo_*`, cache hits/misses | +1% hit rate or +0.01 SLI |
| 9 | **Security / deps** | `cargo audit` vulns, `npm audit` (lower=better) | `cargo audit`, `npm audit --json` | -1 vuln |
| 10 | **Disk / deploy** | `static/` bytes, stale assets, `target/` bloat (lower=better) | `du -sh static`, `ls static/assets` | -1% bytes or -1 stale file |

> For absolute counts (bugs, TODOs, warnings) "1%" means **+1 fix** — still satisfies "at least 1%" for discrete improvements.

## The loop (6 phases, always in order)

### 0) Baseline — snapshot before touching anything
```bash
export PATH="$HOME/.cargo/bin:$PATH"; export CARGO_HOME="$HOME/.cargo"
# rust gates
cargo fmt --all --check            # must be clean or record diff lines
cargo clippy --all-targets -- -D warnings   # must be 0 warnings baseline
cargo test --lib 2>&1 | tail -20   # baseline pass count
# frontend gates
npm --prefix railway-rs/frontend run check:imports 2>&1 | tail
node --check railway-rs/static/app.js 2>&1 | tail
ls -lh railway-rs/static/assets/index-*.js
gzip -c railway-rs/static/assets/index-*.js | wc -c
du -sh railway-rs/static
grep -r "TODO\|FIXME" railway-rs/src --include="*.rs" | wc -l
cat .agents/skills/kaizen/ledger.json  # what was done before
```
Record all numbers in the run's report. If any gate fails pre-run, fix that first (that IS the 1%).

### 1) Discover — enumerate every candidate gap
Run the audit script:
```bash
node .agents/skills/kaizen/scripts/audit.mjs --json
# with LLM research (proposes new aspects the deterministic scan missed):
bash .agents/skills/kaizen/scripts/run.sh --research
# or: node .agents/skills/kaizen/scripts/research.mjs --json
```
It scans deterministically (no LLM guessing) and emits candidates sorted by **ROI = impact / effort**. In `--research` mode an LLM proposes *new* improvement aspects; every LLM proposal is **validated by a real proof command on this machine** and de-duplicated via `ideas-bank.json` — only proposals that verifiably demonstrate the gap enter the pool (`LLM proposes, machine proves, agent implements`). A repair round fixes rejected proofs once:
- Category A: **Zero-risk hygiene** — `cargo fmt` diff, stale `static/assets/index-*.js` duplicates, duplicate `embed-*.js`, `TODO` with trivial fix, `unused` imports, missing `StatusBadge`-style import audit (learned from opencode convo fix-statusbadge-import.md)
- Category B: **Measured hot spots** — slowest slice via `metrics`/`clippy::perf`, largest bundle chunk via `vite --debug`, largest `cargo bloat` symbol, worst cache hit rate slice
- Category C: **Proactive hardening** — `cargo audit` advisory, `npm audit`, missing error handling (`unwrap()` in non-test code), missing `data_source` tag, missing tracing spans
- Category D: **UX polish** — a11y contrast (`--faint` removal pattern from PLAN.md), keyboard handler missing (`Enter`/`Esc`), `prefers-reduced-motion`, 10px font violations

Each candidate has: `id`, `dimension`, `metric_before`, `effort` (S/M/L), `impact` (bytes/ms/warnings), `proof` (command that proves the gap).

### 2) Rank — pick exactly one
Priority order (ties broken by smallest diff):
1. Zero-risk hygiene that is provably +1% (e.g. 3 stale bundles = -66% asset bloat) — always wins if present.
2. Highest `impact/effort` ratio among B/C/D.
3. Never pick a candidate that was in the ledger's last 3 runs (force diversity).
4. Never pick a candidate whose `impact` is <1% of the dimension's total (would not meet invariant).

Write the pick to the report: "Pick: <id> — <dimension> — <before> → <expected_after>".

### 3) Implement — smallest diff that moves the needle >=1%
Rules:
- **One dimension only** — do not mix perf + UX in one run.
- **Smallest diff** — prefer 1–20 lines. If a candidate needs >50 lines, split it and do only the first 1% slice.
- **No behavior change for hygiene** — `fmt`, stale asset prune, import fix, dead-code removal must be behavior-preserving.
- **No fabrication** — latencies, bundle sizes, sources are measured, never guessed. (Hard-learned from opencode conversations where forged NTES data was rejected.)
- **Trace the source** — every slice DTO must keep `data_source`; every latency must be recorded via `state.metrics.record_source_latency`.
- Reuse existing patterns: import audit from `frontend/scripts/check-component-imports.mjs`, metrics from `core/metrics.rs`, SRE checks from `system.rs`.

### 4) Verify — prove >=1% and zero degradation
Re-run ALL gates from phase 0 and compare:
```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --lib 2>&1 | tail -5          # exact same count or higher
# frontend
(cd railway-rs/frontend && npm run build) # if frontend touched → check new hash
ls -lh railway-rs/static/assets/index-*.js && gzip -c railway-rs/static/assets/index-*.js | wc -c
node --check railway-rs/static/*.js
./scripts/smoke.sh                        # if exists
```
For perf claims: `hyperfine` or `curl -w` 5 samples before/after, report mean ± stddev.

Checklist before commit:
- [ ] ≥1% gain on picked dimension (show `before → after` numbers)
- [ ] `cargo fmt --check` passes (0 diff)
- [ ] `cargo clippy -D warnings` 0 warnings
- [ ] `cargo test` same failures as baseline (ideally 0) and not fewer passes
- [ ] No new `unwrap()` in non-test `src/` (grep check)
- [ ] No increase in bundle gzip bytes unless dimension is bundle (and then it must decrease)
- [ ] `ledger.json` updated with entry

If ANY check fails → `git reset --hard`, report "aborted — degraded <dimension>" and stop. No partial commit.

### 5) Record & commit — ledger + commit
Append to `.agents/skills/kaizen/ledger.json`:
```json
{
  "ts": "2026-08-29T02:00:00Z",
  "run": 7,
  "pick": "stale-assets-prune",
  "dimension": "bundle/disk",
  "before": "2.55 MB (3 bundles)",
  "after": "0.85 MB (1 bundle)",
  "delta_pct": -66.7,
  "gates": {"fmt": "pass", "clippy": "pass", "tests": "142 pass"},
  "commit": "abc1234"
}
```

Commit message format (so history is grep-able):
```
kaizen: <dimension> +<pct>% — <short description> (run #<n>)

Before: <metric before>
After:  <metric after>
Gates: fmt pass, clippy pass, tests <n> pass, bundle <bytes>
Ledger: .agents/skills/kaizen/ledger.json#<run>
```

Push only if user asked.

### 6) Report — one-line summary
Output a single factual line:
```
kaizen run #7: bundle -66.7% (2.55MB→0.85MB) via stale-assets-prune — gates pass — ledger updated
```
If aborted, output the reason and the next candidate that would be tried.

## Learning ledger (why this skill generalizes all opencode conversations)

Every opencode conversation that produced a fix is encoded as an audit probe, so the loop automatically reaps that class of win again if it regresses:

| Opencode fix | Probe in `audit.mjs` |
|---|---|
| `StatusBadge is not defined` (fix-statusbadge-import.md) | `svelte-import-audit` — every `.svelte` vs kit exports cross-check |
| Stale `index-B5Tv5bU1.js` served after fix | `stale-assets` — more than 1 `index-*.js` in `static/assets` |
| `average-delay shows same as live-status` (avg-delay-vs-status-debug) | `cache-key-collision` + `data_source` shape check |
| Observability unbounded gauges / overflow | `unbounded-list` — scan `observability` tab for uncapped heights |
| NTES session/CSRF retry fragility | `ntes-web-client` — audit `post_form` retry + `rowsMarker` presence |
| AI persona code-fence spam | `persona-fence` — `PERSONA` contains "no fenced prose" rule |
| 10px font / `--faint` contrast failures | `a11y-contrast` — grep `10px` / `--faint` / `4.5:1` violations |
| Keyboard Enter/Esc missing | `keyboard-audit` — inputs without key handlers |
| Disk quota 64% (space-janitor) | `disk-hygiene` — `du` + stale `target/` + `opencode.db` size |
| `cargo fmt` diff in `config.rs` | `fmt-drift` — `cargo fmt --check` probe |

Add a new probe whenever a new class of bug is fixed — the loop gets smarter forever.

## Scope limits (the "without degrading anything" guarantee)

- Never widen `unsafe` surface.
- Never add a dependency to fix a lint (fix the code).
- Never change an NTES parser's regex without a fixture test proving the old HTML still passes.
- Never edit `.space-janitor/` (per AGENTS.md guard).
- Never `git push` or `gh auth setup-git` unless explicitly asked.
- Never touch `tests/common/mod.rs` harness (would silently weaken tests).
- Never fabricate metrics — if a metric cannot be measured on this host, skip that dimension.

## Running it

### One-shot (recommended)
```bash
bash .agents/skills/kaizen/scripts/run.sh
bash .agents/skills/kaizen/scripts/run.sh --research  # also run LLM discovery for new aspects
# or
node .agents/skills/kaizen/scripts/audit.mjs          # just the discovery phase
node .agents/skills/kaizen/scripts/audit.mjs --fix    # discovery + auto-fix smallest hygiene item
node .agents/skills/kaizen/scripts/research.mjs --json# LLM discovery only (needs Copilot oauth or KAIZEN_LLM_* env)
```

### As an opencode agent (if opencode is installed)
```bash
opencode run kaizen
# or via prompt: "kaizen: improve by 1%"
```

### Autonomous daemon (always innovates)
The skill ships as an autonomous agent that never stalls:

- **Daemon** lives in `.kaizen/` (like `.space-janitor/`), starts on login via `~/.bashrc` hook installed by `bash .kaizen/install.sh`. It runs `run.sh --research` every hour (`CYCLE_SECS=3600`, configurable in `.kaizen/config.env`).
- **Always innovates**: deterministic probes cover hygiene/perf/UX/security; when they are green the LLM research proposes *new* aspects (validated by a real proof command + one self-heal retry, de-duplicated in `ideas-bank.json`). If even that is empty, `audit.mjs` injects a guaranteed `innovate-tests` fallback so the pool is never zero — every cycle either commits a 1% win or surfaces the next manual innovation for the agent to implement.
- **Autonomous by default**: `run.sh` auto-enables `--research` whenever a provider is available (Copilot oauth at `.local/share/opencode/auth.json` or `KAIZEN_LLM_*` env). Use `--no-research` to opt out. The daemon always runs with `--research`.
- **Controls**: `cat .kaizen/status.json` (last run, next cycle), `cat .kaizen/logs/daemon.log`, `touch .kaizen/PAUSE` to pause, `bash .kaizen/daemon.sh --once` for a single cycle, `bash .kaizen/install.sh` to reinstall after home wipe.

### In CI (nightly cron)
```yaml
- run: bash .agents/skills/kaizen/scripts/run.sh --research --ci
  # on success, opens a PR with the 1% commit; on abort, posts the report as a comment
```

## Exit codes
- `0` — committed a >=1% improvement, gates pass
- `2` — aborted: no candidate met 1% or invariant would be violated (not a failure, just "nothing safe to do")
- `1` — infra failure (toolchain missing, build broken) — fix infra first

## Anti-patterns that break the invariant
- Batching multiple dimensions in one commit (cannot attribute the gain, harder to revert).
- "Improving" by deleting tests or weakening clippy lints — gates must stay ≥ baseline.
- Optimizing a benchmark that is not the production path (measure what users hit: `/rail-api/*` and `static/assets`).
- Claiming 1% on a fabricated before/after — always show the command that produced the numbers.

## What improved — the Improvements page

Every run appends to `.agents/skills/kaizen/ledger.json` and mirrors to `railway-rs/static/data/kaizen.json` (served at `GET /data/kaizen.json` so the deployed app always has the committed history). The app exposes it at **`/kaizen`** (nav: *App → Improvements*).

The page (`frontend/src/lib/pages/Kaizen.svelte`) is live data only:
- fetches `/data/kaizen.json` via `$lib/api.js`
- hero tiles: total improvements shipped, static bytes freed, LLM-proposed count, latest run
- run-history table (newest first): run #, what improved (pick), dimension, Δ%, before → after, source (`scan` vs `LLM`), date, commit
- guardrail card + method card explaining the invariant

If the mirror is missing (fresh clone before first run) the page shows an empty state; `run.sh` creates the mirror on the next committed run.

## Reference implementation files
- `scripts/audit.mjs` — deterministic discovery, no LLM, JSON output, ROI-ranked
- `scripts/research.mjs` — LLM discovery of new aspects; providers: `KAIZEN_LLM_*` env (OpenAI-compatible) → GitHub Copilot via opencode's oauth → fallback; every idea validated by its `proof_command` + `proof_match_regex`, one self-heal retry, de-duplicated in `ideas-bank.json`
- `scripts/run.sh` — orchestration, baseline/verify, ledger, mirror, commit; `--research` prefers the best validated LLM aspect over the deterministic pool
- `ledger.json` — append-only history, enables N runs without revisiting same fix; each entry now carries `source` (`deterministic` | `llm`)
- `ideas-bank.json` — de-duplication fingerprints for LLM ideas across runs
- `railway-rs/static/data/kaizen.json` — public mirror of `ledger.json` for the `/kaizen` page (committed)
- `railway-rs/frontend/src/lib/pages/Kaizen.svelte` — the Improvements page
- This file — the contract the agent must obey on every run
