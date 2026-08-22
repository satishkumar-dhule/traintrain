const THEME_MODES = ['system', 'light', 'dark']
const CONTRAST_MODES = ['off', 'high', 'invert']

function applyTheme(theme) {
  const root = document.documentElement
  root.classList.remove('light', 'dark')
  if (theme === 'light' || theme === 'dark') {
    root.classList.add(theme)
  }
}

function initialTheme() {
  const saved = localStorage.getItem('rc-theme')
  return saved === 'light' || saved === 'dark' ? saved : 'system'
}

function applyContrast(mode) {
  if (mode === 'high' || mode === 'invert') {
    document.documentElement.setAttribute('data-contrast', mode)
  } else {
    document.documentElement.removeAttribute('data-contrast')
  }
}

function initialContrast() {
  const saved = localStorage.getItem('rc-contrast')
  return CONTRAST_MODES.includes(saved) ? saved : 'off'
}

export const theme = $state({ mode: 'system' })
export const contrast = $state({ mode: 'off' })

export function initTheme() {
  theme.mode = initialTheme()
  contrast.mode = initialContrast()
  applyTheme(theme.mode)
  applyContrast(contrast.mode)
}

export function setTheme(mode) {
  theme.mode = mode
  localStorage.setItem('rc-theme', mode)
  applyTheme(mode)
}

export function toggleTheme() {
  setTheme(theme.mode === 'dark' ? 'light' : theme.mode === 'light' ? 'dark' : matchMedia('(prefers-color-scheme: dark)').matches ? 'light' : 'dark')
}

export function cycleTheme() {
  setTheme(THEME_MODES[(THEME_MODES.indexOf(theme.mode) + 1) % THEME_MODES.length])
}

export function setContrast(mode) {
  contrast.mode = CONTRAST_MODES.includes(mode) ? mode : 'off'
  localStorage.setItem('rc-contrast', contrast.mode)
  applyContrast(contrast.mode)
}

export function toggleContrast(mode) {
  setContrast(contrast.mode === mode ? 'off' : mode)
}

export function cycleContrast() {
  setContrast(CONTRAST_MODES[(CONTRAST_MODES.indexOf(contrast.mode) + 1) % CONTRAST_MODES.length])
}
