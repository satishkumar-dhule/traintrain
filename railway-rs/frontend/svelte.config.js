import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

export default {
  preprocess: vitePreprocess(),
  alias: {
    $lib: 'src/lib'
  }
}
