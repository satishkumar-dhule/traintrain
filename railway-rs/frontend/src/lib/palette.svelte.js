export const palette = $state({ open: false })

export function openPalette() {
  palette.open = true
}

export function closePalette() {
  palette.open = false
}

export function togglePalette() {
  palette.open = !palette.open
}
