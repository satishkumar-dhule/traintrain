<script>
  let { icon = null, title = '', hint = '', children = null } = $props()
</script>

<div
  class="flex flex-col items-center gap-2 rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground"
>
  {#if icon}
    {@const Icon = icon}
    <Icon class="size-5 shrink-0" aria-hidden="true" />
  {/if}
  {#if title}<span class="font-medium text-foreground">{title}</span>{/if}
  {#if hint}<span>{hint}</span>{/if}
  {@render children?.()}
</div>
