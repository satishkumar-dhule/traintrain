// Bootstrap the /tmp/opencode/ui-harness environment: playwright-core,
// chromium headless shell, and the Debian shared libraries it needs.
// Idempotent; safe to rerun. No host installs, no root.
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import zlib from 'node:zlib'

const HOME = process.env.UI_HARNESS_HOME || '/tmp/opencode/ui-harness'
const BROWSERS = path.join(HOME, 'browsers')
const DEBROOT = path.join(HOME, 'debroot')
const PW_VERSION = '1.55.0'

fs.mkdirSync(BROWSERS, { recursive: true })

function sh(cmd, args, opts = {}) {
  console.log('$', cmd, args.join(' '))
  execFileSync(cmd, args, { stdio: 'inherit', ...opts })
}

// 1. playwright-core
const pwCore = path.join(HOME, 'node_modules', 'playwright-core', 'package.json')
if (!fs.existsSync(pwCore)) {
  fs.writeFileSync(path.join(HOME, 'package.json'), JSON.stringify({ private: true }))
  sh('npm', ['i', '--prefix', HOME, '--no-fund', '--no-audit', `playwright-core@${PW_VERSION}`])
}

// 2. chromium headless shell
const hasShell = fs
  .readdirSync(BROWSERS)
  .some((d) => fs.existsSync(path.join(BROWSERS, d, 'chrome-linux', 'headless_shell')))
if (!hasShell) {
  const env = { ...process.env, PLAYWRIGHT_BROWSERS_PATH: BROWSERS }
  const npxBin = process.env.NPX_BIN || 'npx'
  sh(npxBin, ['-y', `playwright@${PW_VERSION}`, 'install', 'chromium-headless-shell'], {
    env,
    timeout: 600000,
  })
}

// 3. Debian library tree (only what headless_shell is missing on this host)
if (!fs.existsSync(path.join(DEBROOT, '.done'))) {
  const PKGDIR = path.join(HOME, 'pool')
  fs.mkdirSync(PKGDIR, { recursive: true })
  const gz = Buffer.from(
    await (
      await fetch('http://deb.debian.org/debian/dists/bookworm/main/binary-amd64/Packages.gz')
    ).arrayBuffer(),
  )
  const text = zlib.gunzipSync(gz).toString('utf8')
  const pkgs = new Map()
  const provides = new Map()
  for (const chunk of text.split('\n\n')) {
    if (!chunk.trim()) continue
    const get = (k) => chunk.match(new RegExp(`^${k}: (.*)$`, 'm'))?.[1]?.trim() ?? ''
    const rec = {
      name: get('Package'),
      filename: get('Filename'),
      depends: get('Depends'),
      provides: get('Provides'),
    }
    if (!rec.name || !rec.filename || pkgs.has(rec.name)) continue
    pkgs.set(rec.name, rec)
    for (const p of rec.provides.split(',')) {
      const key = p.trim().split(' ')[0]
      if (key) provides.set(key, [...(provides.get(key) ?? []), rec.name])
    }
  }
  const firstAlt = (dep) => {
    for (const alt of dep.split('|').map((s) => s.trim().split(' ')[0]).filter(Boolean)) {
      if (pkgs.has(alt)) return alt
      if (provides.get(alt)?.length) return provides.get(alt)[0]
    }
    return null
  }
  const SEEDS = [
    'libasound2', 'libatk1.0-0', 'libatk-bridge2.0-0', 'libatspi2.0-0',
    'libdbus-1-3', 'libdrm2', 'libgbm1', 'libglib2.0-0', 'libnspr4', 'libnss3',
    'libx11-6', 'libxcb1', 'libxcomposite1', 'libxdamage1', 'libxext6',
    'libxfixes3', 'libxrandr2', 'libxkbcommon0',
    'libfontconfig1', 'libfreetype6', 'libpng16-16', 'libexpat1', 'libbrotli1',
    'fonts-liberation', 'fontconfig-config', 'libuuid1', 'libjpeg62-turbo',
  ]
  const need = new Set()
  const queue = [...SEEDS]
  while (queue.length) {
    const n = queue.pop()
    if (need.has(n)) continue
    let rec = pkgs.get(n)
    if (!rec) {
      const via = provides.get(n)?.map((v) => pkgs.get(v)).find(Boolean)
      if (!via) { console.error('MISS', n); continue }
      rec = via
    }
    need.add(rec.name)
    for (const d of rec.depends.split(',')) {
      if (!d.trim()) continue
      const r = firstAlt(d)
      if (r && !need.has(r)) queue.push(r)
    }
  }
  for (const name of need) {
    const rec = pkgs.get(name)
    const dest = path.join(PKGDIR, path.basename(rec.filename))
    if (!fs.existsSync(dest))
      fs.writeFileSync(dest, Buffer.from(await (await fetch('http://deb.debian.org/debian/' + rec.filename)).arrayBuffer()))
    execFileSync('dpkg-deb', ['-x', dest, DEBROOT])
  }
  // Strip Debian's core libc bits — host glibc must win (nix binaries need a
  // newer GLIBC than bookworm ships).
  const lib = path.join(DEBROOT, 'lib/x86_64-linux-gnu')
  const ulib = path.join(DEBROOT, 'usr/lib/x86_64-linux-gnu')
  for (const f of [
    'libc.so.6', 'libc-2.36.so', 'ld-linux-x86-64.so.2', 'libm.so.6',
    'libpthread.so.0', 'libdl.so.2', 'librt.so.1',
  ]) fs.rmSync(path.join(lib, f), { force: true })
  for (const f of ['libstdc++.so.6', 'libstdc++.so.6.0.30', 'libgcc_s.so.1'])
    fs.rmSync(path.join(ulib, f), { force: true })
  fs.rmSync(path.join(DEBROOT, 'lib64/ld-linux-x86-64.so.2'), { force: true })
  fs.writeFileSync(path.join(DEBROOT, '.done'), String(need.size))
}

console.log('\nharness ready at', HOME)
