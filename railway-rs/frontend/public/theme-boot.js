/* Pre-paint theme bootstrap: apply the saved dark/bright class before the
   stylesheet paints to avoid a flash of the wrong scheme. Kept as an external
   same-origin file so it satisfies the app's CSP (`script-src 'self'`). */
try {
  var t = localStorage.getItem('rc-theme')
  if (t !== 'light' && t !== 'dark')
    t = matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  document.documentElement.classList.add(t)
} catch (e) {}
