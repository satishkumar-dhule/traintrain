// server.mjs - guarantee the app under test is up, reusing an already-running
// instance when possible. Never kills a server it did not start.
import { spawn } from 'node:child_process'
import fs from 'node:fs'
import { ROOT, BASE_URL } from './env.mjs'

const started = []

async function healthy(timeoutMs = 1500) {
  const ctl = new AbortController()
  const t = setTimeout(() => ctl.abort(), timeoutMs)
  try {
    const r = await fetch(`${BASE_URL}/healthz`, { signal: ctl.signal })
    if (!r.ok) return false
    const j = await r.json()
    return j.status === 'ok' && j.service === 'railway-rs'
  } catch {
    return false
  } finally {
    clearTimeout(t)
  }
}

function pickBinary() {
  for (const rel of ['target/release/railway-rs', 'target/debug/railway-rs']) {
    const p = `${ROOT}/${rel}`
    if (fs.existsSync(p)) return p
  }
  return null
}

/* Resolve to a running app: reuse if healthy, else start the local binary on
   the port embedded in BASE_URL and wait (up to ~25s) for healthz. */
export async function ensureApp() {
  if (await healthy()) return { startedBySuite: false }

  const bin = pickBinary()
  if (!bin) {
    throw new Error(
      `no app at ${BASE_URL} and no railway-rs binary under ${ROOT}/target — run "cargo build" or start the server first`,
    )
  }
  const port = new URL(BASE_URL).port || '80'
  const log = fs.openSync('/tmp/railway-ui-test-server.log', 'a')
  const child = spawn(bin, [], {
    cwd: ROOT,
    detached: true,
    stdio: ['ignore', log, log],
    env: { ...process.env, RAILWAY_PORT: port },
  })
  child.unref()
  started.push(child.pid)

  const deadline = Date.now() + 25000
  while (Date.now() < deadline) {
    if (await healthy(1000)) return { startedBySuite: true }
    await new Promise((r) => setTimeout(r, 400))
  }
  throw new Error(`app did not become healthy on ${BASE_URL} within 25s (log: /tmp/railway-ui-test-server.log)`)
}

/* Only ever stops processes this module spawned. */
export async function shutdown() {
  for (const pid of started.splice(0)) {
    try {
      process.kill(-pid, 'SIGTERM') // negative pid: we started it detached
    } catch {
      /* already gone */
    }
  }
}
