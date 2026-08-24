<script>
  import * as Breadcrumb from '$lib/components/ui/breadcrumb/index.js';
  import { navigate } from '$lib/router.svelte.js';
  import { cn } from '$lib/utils.js';

  let {
    items = [],
    class: className = '',
  } = $props();
</script>

<nav aria-label="Breadcrumb" class={className}>
  <Breadcrumb.List>
    {#each items as item, i (i)}
      <Breadcrumb.Item>
        {#if i === items.length - 1 || !item.href}
          <Breadcrumb.Page>{item.label}</Breadcrumb.Page>
        {:else}
          <Breadcrumb.Link href={item.href} onclick={(e) => { e.preventDefault(); navigate(item.href); }}>{item.label}</Breadcrumb.Link>
        {/if}
      </Breadcrumb.Item>
      {#if i < items.length - 1}<Breadcrumb.Separator />{/if}
    {/each}
  </Breadcrumb.List>
</nav>
