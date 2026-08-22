function apply(theme) {
  const root = document.documentElement
  root.classList.remove('light', 'dark')
  if (theme === 'light' || theme === 'dark') {
    root.classList.add(theme)
  }
}

function initial() {
  const saved = localStorage.getItem('rc-theme')
  return saved === 'light' || saved === 'dark' ? saved : 'system'
}

export const theme = $state({ mode: 'system' })

export function initTheme() {
  theme.mode = initial()
  apply(theme.mode)
}

export function setTheme(mode) {
  theme.mode = mode
  localStorage.setItem('rc-theme', mode)
  apply(mode)
}

export function toggleTheme() {
  setTheme(theme.mode === 'dark' ? 'light' : theme.mode === 'light' ? 'dark' : matchMedia('(prefers-color-scheme: dark)').matches ? 'light' : 'dark')
}
