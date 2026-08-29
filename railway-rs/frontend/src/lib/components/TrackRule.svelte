<script>
  let { label = '' } = $props()
</script>

{#if label}
  <div class="flex items-center gap-3" aria-hidden="true">
    <div class="track-rule flex-1"></div>
    <span class="shrink-0 rounded border bg-card px-2 py-0.5 text-[10px] font-medium uppercase tracking-[0.16em] text-muted-foreground">{label}</span>
    <div class="track-rule flex-1"></div>
  </div>
{:else}
  <div class="track-rule" aria-hidden="true"></div>
{/if}
