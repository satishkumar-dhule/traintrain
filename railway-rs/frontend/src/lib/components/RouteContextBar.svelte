<script>
  import { cn } from '$lib/utils.js'

  let { from = '', to = '', onEdit = () => {}, class: className = '' } = $props()
</script>

<div
  class={cn(
    'sticky top-0 z-40 flex min-h-9 items-center justify-center border-b bg-background/95 px-3 py-1.5 backdrop-blur supports-[backdrop-filter]:bg-background/85 lg:hidden',
    className,
  )}
>
  <button
    type="button"
    onclick={onEdit}
    aria-label={from && to ? `${from} to ${to}, tap to edit` : 'Route'}
    class="flex min-w-0 max-w-full items-center gap-1 overflow-hidden rounded-full border border-border bg-muted/50 px-3 py-1 text-xs font-semibold transition-colors hover:bg-muted sm:text-sm"
  >
    <span class="min-w-0 truncate">{from || '—'}</span>
    <span class="shrink-0 text-muted-foreground">→</span>
    <span class="min-w-0 truncate text-right">{to || '—'}</span>
  </button>
</div>
