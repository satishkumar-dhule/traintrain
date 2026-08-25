<script>
  import * as Tabs from '$lib/components/ui/tabs/index.js'

  let {
    cols = 2,
    children = undefined,
    class: klass = ''
  } = $props()

  const gridCols = $derived(cols === 5 ? 'max-lg:grid-cols-5' : cols === 3 ? 'max-lg:grid-cols-3' : 'max-lg:grid-cols-2')
</script>

<Tabs.List
  class={`w-full justify-start overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden max-lg:grid ${gridCols} max-lg:gap-1 max-lg:overflow-visible max-lg:overflow-x-visible max-lg:h-auto max-lg:p-1 max-lg:bg-muted max-lg:rounded-lg max-lg:border ${klass}`}
>
  {@render children?.()}
</Tabs.List>
