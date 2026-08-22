const MAX_ITEMS = 6

export function loadRecent(key, validate) {
  try {
    const raw = JSON.parse(localStorage.getItem(key) ?? '[]')
    if (!Array.isArray(raw)) return []
    const items = typeof validate === 'function' ? raw.filter(validate) : raw
    return items.slice(0, MAX_ITEMS)
  } catch {
    return []
  }
}

export function rememberRecent(key, entry, validate) {
  const rest = loadRecent(key, validate)
    .filter((r) => r?.id !== entry.id)
    .slice(0, MAX_ITEMS - 1)
  const next = [{ ...entry }, ...rest]
  try {
    localStorage.setItem(key, JSON.stringify(next))
  } catch {}
  return next
}

export function clearStored(key) {
  try {
    localStorage.removeItem(key)
  } catch {}
}
