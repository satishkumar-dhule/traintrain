<script>
  import { navigate } from '$lib/router.svelte.js'
  import { visitTrail } from '$lib/visit-trail.svelte.js'
  import * as Breadcrumb from '$lib/components/ui/breadcrumb/index.js'

  let { title = 'Visit trail' } = $props()

  let stripEl = $state(null)
  let listEl = $state(null)
  let hidden = $state(0)
  let containerW = $state(0)
  let revealBlocked = false

  const SLACK = 48

  function step() {
    if (!stripEl || !listEl || !visitTrail.entries.length) return
    const over = listEl.scrollWidth - listEl.clientWidth
    if (over > 0 && hidden < visitTrail.entries.length - 1) {
      hidden++
      revealBlocked = false
      requestAnimationFrame(step)
    } else if (!revealBlocked && hidden > 0 && over <= -SLACK) {
      hidden--
      requestAnimationFrame(() => {
        if (!listEl) return
        if (listEl.scrollWidth - listEl.clientWidth > 0) {
          hidden++
          revealBlocked = true
        } else {
          step()
        }
      })
    }
  }

  $effect(() => {
    void visitTrail.entries.length
    void containerW
    if (!stripEl) return
    revealBlocked = false
    requestAnimationFrame(step)
    document.fonts?.ready?.then(() => step())
  })

  $effect(() => {
    if (!stripEl) return
    const ro = new ResizeObserver(() => (containerW = stripEl.clientWidth))
    ro.observe(stripEl)
    containerW = stripEl.clientWidth
    return () => ro.disconnect()
  })

  function go(e, path) {
    e.preventDefault()
    if (path !== window.location.pathname) navigate(path)
  }
</script>

{#if visitTrail.entries.length > 1}
  <div
    bind:this={stripEl}
    class="z-20 border-b bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80 lg:sticky lg:top-0"
  >
    <nav aria-label={title} data-hidden={hidden} class="mx-auto w-full max-w-5xl min-w-0 px-4 md:px-8">
      <Breadcrumb.List bind:ref={listEl} class="min-w-0 flex-nowrap overflow-hidden py-1.5">
        {#if hidden > 0}
          <Breadcrumb.Ellipsis class="shrink-0 max-lg:min-h-11" />
        {/if}
        {#each visitTrail.entries.slice(hidden) as entry, i (entry.ts + entry.path)}
          <Breadcrumb.Item data-crumb class="shrink-0 flex items-center">
            {#if i === visitTrail.entries.length - hidden - 1}
              <Breadcrumb.Page
                class="max-lg:flex max-lg:min-h-11 max-lg:items-center min-w-0 font-medium text-foreground"
              >
                <span class="max-w-[9rem] truncate font-mono sm:max-w-[14rem]">{entry.label}</span>
              </Breadcrumb.Page>
            {:else}
              <Breadcrumb.Link
                href={entry.path}
                onclick={(e) => go(e, entry.path)}
                class="max-lg:flex max-lg:min-h-11 max-lg:items-center min-w-0 rounded-md px-1 hover:bg-accent"
              >
                <span class="max-w-[9rem] truncate font-mono sm:max-w-[14rem]">{entry.label}</span>
              </Breadcrumb.Link>
            {/if}
          </Breadcrumb.Item>
          {#if i < visitTrail.entries.length - hidden - 1}
            <Breadcrumb.Separator data-sep />
          {/if}
        {/each}
      </Breadcrumb.List>
    </nav>
  </div>
{/if}
