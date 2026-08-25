<script>
  import { viewport } from '$lib/media.svelte.js'
  import PageHeader from '$lib/components/PageHeader.svelte'
  import Breadcrumbs from '$lib/components/Breadcrumbs.svelte'
  import RouteContextBar from '$lib/components/RouteContextBar.svelte'

  let {
    title = '',
    description = '',
    size = 'default',
    breadcrumbs = [],
    routeFrom = '',
    routeTo = '',
    onEdit = null,
    showRouteBar = false,
    headerActions = undefined,
    extra = undefined,
    class: klass = ''
  } = $props()

  const hasRoute = $derived(Boolean(routeFrom) && showRouteBar)
</script>

<div class={klass}>
  {#if viewport.narrow && hasRoute}
    <RouteContextBar from={routeFrom} to={routeTo} onEdit={onEdit ?? (() => {})} />
  {:else if title}
    <PageHeader {title} {description} {size} actions={headerActions}>
      {#snippet children()}
        {#if breadcrumbs.length}
          <Breadcrumbs items={breadcrumbs} />
        {/if}
        {@render extra?.()}
      {/snippet}
    </PageHeader>
    <div class="track-rule" aria-hidden="true"></div>
  {/if}
</div>
