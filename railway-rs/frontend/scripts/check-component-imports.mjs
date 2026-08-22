/* Fails the build when a shared component is used in markup but not imported
   (Svelte 5 compiles unknown capitalized tags to bare identifiers, so this
   only explodes at runtime — e.g. "ReferenceError: X is not defined").
   Dependency-free; runs as `prebuild`. */
import { readdirSync, readFileSync } from 'node:fs'
import { join, sep } from 'node:path'

const LIB = join(process.cwd(), 'src', 'lib')

/* Every component exported from $lib/components/badges/index.js. Kept in sync
   manually; add new exports here so usage-without-import stays a build error. */
const KIT = [
  'StatusBadge',
  'TrainNumberBadge',
  'StationCodeBadge',
  'DelayBadge',
  'RunsOnBadges',
  'DataSourceBadge',
  'CountBadge',
  'PnrStatusBadge',
  'AvailabilityStatusBadge',
  'HaltStatusBadge',
  'LogLevelBadge',
  'ExceptionKindBadge'
]

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name)
    if (e.isDirectory()) {
      // The kit itself defines these components; skip it.
      if (p.split(sep).includes('badges')) return []
      return walk(p)
    }
    return p.endsWith('.svelte') ? [p] : []
  })
}

const problems = []
for (const file of walk(LIB)) {
  const src = readFileSync(file, 'utf8')
  const used = KIT.filter((c) => new RegExp(`<${c}[\\s/>]`).test(src))
  if (!used.length) continue

  const block = src.match(
    /import\s*\{([^}]*)\}\s*from\s*['"]\$lib\/components\/badges\/index\.js['"]/
  )
  const imported = new Set(
    block ? block[1].split(',').map((s) => s.trim()).filter(Boolean) : []
  )
  for (const c of used) {
    if (!imported.has(c)) problems.push(`${file}: <${c}> used but not imported`)
  }
}

if (problems.length) {
  console.error('\ncomponent-import audit failed:')
  for (const p of problems) console.error('  ' + p)
  console.error('')
  process.exit(1)
}
console.log('component-import audit OK')
