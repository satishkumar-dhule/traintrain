<script>
  import { Skeleton } from '$lib/components/ui/skeleton/index.js'
  import * as Alert from '$lib/components/ui/alert/index.js'
  import EmptyState from '$lib/components/EmptyState.svelte'

  let {
    phase = 'idle', // 'loading'|'refreshing'|'error'|'ok'|'idle'
    error = null,
    empty = false,
    skeletonCount = 4,
    emptyIcon = null,
    emptyTitle = 'No data',
    emptyHint = '',
    children = undefined,
    // optional: show skeletons as Card-like? simple list
    class: klass = ''
  } = $props()

  const isLoading = $derived(phase === 'loading' || phase === 'refreshing')
  const isError = $derived(phase === 'error')
  const isEmpty = $derived(empty && phase !== 'loading' && phase !== 'refreshing' && phase !== 'error')
</script>

<div class={klass}>
  {#if isLoading}
    <div class="grid gap-2" aria-busy="true">
      {#each Array(skeletonCount) as _, i (i)}
        <Skeleton class="h-10 w-full" />
      {/each}
    </div>
  {:else if isError}
    <Alert.Root variant="destructive" role="alert">
      <Alert.Title>Could not load</Alert.Title>
      <Alert.Description>{error || 'Something went wrong'}</Alert.Description>
    </Alert.Root>
  {:else if isEmpty}
    {#if emptyIcon}
      <EmptyState icon={emptyIcon} title={emptyTitle} hint={emptyHint} />
    {:else}
      <div class="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
        {emptyHint || emptyTitle}
      </div>
    {/if}
  {:else if children}
    {@render children()}
  {/if}
</div>
