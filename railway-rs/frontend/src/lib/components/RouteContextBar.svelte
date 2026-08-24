<script>
  import { navigate } from '$lib/router.svelte.js'
  import { cn } from '$lib/utils.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import ArrowLeft from 'lucide-svelte/icons/arrow-left'
  import Pencil from 'lucide-svelte/icons/pencil'

  let { from = '', to = '', onEdit = () => {}, class: className = '' } = $props()

  function goBack() {
    if (window.history.length > 1) window.history.back()
    else navigate('/')
  }
</script>

<div
  class={cn(
    'sticky top-0 z-40 flex min-h-11 items-center gap-1.5 border-b bg-background/95 px-2 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:hidden',
    className,
  )}
>
  <Button type="button" variant="ghost" size="icon" onclick={goBack} class="size-9 shrink-0" aria-label="Go back">
    <ArrowLeft class="size-4.5" />
  </Button>

  <button
    type="button"
    onclick={onEdit}
    class="flex min-w-0 flex-1 items-center gap-1.5 overflow-hidden rounded-full border border-border bg-muted/50 px-3 py-1 text-xs font-semibold transition-colors hover:bg-muted sm:text-sm"
  >
    <span class="min-w-0 flex-1 truncate">{from || '—'}</span>
    <span class="shrink-0 text-muted-foreground">→</span>
    <span class="min-w-0 flex-1 truncate text-right">{to || '—'}</span>
    <Pencil class="size-3 shrink-0 text-muted-foreground" />
  </button>

  <Button type="button" variant="ghost" size="icon" onclick={() => navigate('/')} class="size-9 shrink-0" aria-label="Home">
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-4"><path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>
  </Button>
</div>
