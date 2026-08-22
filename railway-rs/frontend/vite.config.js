import path from 'node:path'
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  base: '/',
  plugins: [svelte(), tailwindcss()],
  resolve: {
    alias: {
      $lib: path.resolve('./src/lib')
    }
  },
  build: {
    outDir: '../static',
    emptyOutDir: false
  },
  server: {
    proxy: {
      '/rail-api': 'http://localhost:3000'
    }
  }
})
