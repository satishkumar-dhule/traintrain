/* Fails the build when a Svelte component is used in markup but not imported
   (Svelte 5 compiles unknown capitalized tags to bare identifiers, so this
   only explodes at runtime — e.g. "ReferenceError: JourneysTable is not
   defined" which once blanked /plan). Dependency-free; runs as `prebuild`.

   General audit, not just the badges kit:
   - collects every identifier imported into the file (default, named,
     namespace `* as X`) plus locally-bound component variables
     (`const Icon = ...`) and svelte's special elements
   - collects every capitalized tag used in markup (after the last
     `</script>`; `<svelte:el` excluded)
   - a used tag with no matching binding (and not the file's own component,
     recursion) is a build error. */
import { readdirSync, readFileSync } from 'node:fs'
import { join, sep } from 'node:path'

const LIB = join(process.cwd(), 'src', 'lib')

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = join(dir, e.name)
    if (e.isDirectory()) return walk(p)
    return p.endsWith('.svelte') ? [p] : []
  })
}

function boundNames(src) {
  const names = new Set(['svelte', 'self'])
  for (const m of src.matchAll(/import\s+([\s\S]*?)\s+from\s*['"][^'"]+['"]/g)) {
    const clause = m[1]
    for (const ns of clause.matchAll(/\*\s+as\s+(\w+)/g)) names.add(ns[1])
    const def = clause.match(/(?:^|,)\s*(?!{)(\w+)/)
    if (def && !/^\s*$/.test(def[1])) names.add(def[1])
    const named = clause.match(/{([^}]*)}/)
    if (named) {
      for (const n of named[1].split(',')) {
        const id = n.split(/\s+as\s+/).pop()?.trim()
        if (id && /^\w+$/.test(id)) names.add(id)
      }
    }
  }
  /* Local component bindings: const Icon = ..., let Foo = $derived(...) */
  for (const m of src.matchAll(/(?:const|let)\s+([A-Z]\w*)\s*=/g)) names.add(m[1])
  /* Snippet parameters: {#snippet header(Icon, title)} — params are bindings. */
  for (const m of src.matchAll(/\{#snippet\s+\w+\s*\(([^)]*)\)/g)) {
    for (const p of m[1].split(',')) {
      const id = p.trim()
      if (/^[A-Z]\w*$/.test(id)) names.add(id)
    }
  }
  return names
}

const problems = []
for (const file of walk(LIB)) {
  const src = readFileSync(file, 'utf8')
  const own = file.split(sep).pop().replace(/\.svelte$/, '')
  const names = boundNames(src)

  /* Markup = after the last </script> (module + instance scripts hold no tags). */
  const markup = src.slice(src.lastIndexOf('</script>'))
  for (const m of markup.matchAll(/<([A-Z]\w*)(?:[.\s/>])/g)) {
    const tag = m[1]
    if (tag === 'Self' || tag === own) continue
    if (!names.has(tag)) problems.push(`${file}: <${tag}> used but not imported`)
  }
}

if (problems.length) {
  console.error('\ncomponent-import audit failed:')
  for (const p of problems) console.error('  ' + p)
  console.error('')
  process.exit(1)
}
console.log('component-import audit OK')
