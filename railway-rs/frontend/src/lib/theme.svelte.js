function applyTheme(theme) {
  const root = document.documentElement
  root.classList.remove('light', 'dark')
  root.classList.add(theme)
}

function initialTheme() {
  try {
    const saved = localStorage.getItem('rc-theme')
    if (saved === 'light' || saved === 'dark') return saved
  } catch {
    /* storage unavailable */
  }
  return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

export const theme = $state({ mode: 'dark' })

export function initTheme() {
  theme.mode = initialTheme()
  applyTheme(theme.mode)
}

export function setTheme(mode) {
  if (mode !== 'light' && mode !== 'dark') return
  theme.mode = mode
  try {
    localStorage.setItem('rc-theme', mode)
  } catch {
    /* storage unavailable */
  }
  applyTheme(mode)
}

export function toggleTheme() {
  setTheme(theme.mode === 'dark' ? 'light' : 'dark')
}
