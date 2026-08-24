<script>
  import { navigate } from '$lib/router.svelte.js'
  import { cn } from '$lib/utils.js'
  import { Button } from '$lib/components/ui/button/index.js'
  import ArrowLeft from 'lucide-svelte/icons/arrow-left'
  import Pencil from 'lucide-svelte/icons/pencil'
  import House from 'lucide-svelte/icons/house'
  import MessageCircle from 'lucide-svelte/icons/message-circle'

  let { from = '', to = '', onEdit = () => {}, class: className = '' } = $props()

  function goBack() {
    if (window.history.length > 1) window.history.back()
    else navigate('/')
  }
</script>

<div
  class={cn(
    'sticky top-0 z-40 flex h-12 items-center gap-2 border-b bg-background/95 px-3 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:hidden',
    className,
  )}
>
  <Button type="button" variant="ghost" size="icon" onclick={goBack} class="size-10 shrink-0" aria-label="Go back">
    <ArrowLeft class="size-5" />
  </Button>

  <button
    type="button"
    onclick={onEdit}
    class="flex min-w-0 flex-1 items-center justify-center gap-2 rounded-full border border-border bg-muted/50 px-4 py-1.5 text-sm font-semibold transition-colors hover:bg-muted"
  >
    <span class="truncate">{from || '—'}</span>
    <span class="text-muted-foreground">→</span>
    <span class="truncate">{to || '—'}</span>
    <Pencil class="size-3.5 shrink-0 text-muted-foreground" />
  </button>

  <Button type="button" variant="ghost" size="icon" onclick={() => navigate('/')} class="size-10 shrink-0" aria-label="Home">
    <House class="size-5" />
  </Button>
  <Button type="button" variant="ghost" size="icon" onclick={() => navigate('/assistant')} class="size-10 shrink-0" aria-label="Ask Train Bro">
    <MessageCircle class="size-5" />
  </Button>
</div>
