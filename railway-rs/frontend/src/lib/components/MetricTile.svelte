<script>
  import * as Card from '$lib/components/ui/card/index.js'
  let { icon, label = '', value = '—', sub = '', tone = 'slate' } = $props()
  let Icon = $derived(icon)
  const toneChip = {
    sky: 'bg-primary/10 text-primary',
    violet: 'bg-chart-3/10 text-chart-3',
    emerald: 'bg-signal-go/10 text-signal-go-ink',
    amber: 'bg-signal-hold/10 text-signal-hold-ink',
    rose: 'bg-signal-stop/10 text-signal-stop-ink',
    slate: 'bg-muted text-muted-foreground'
  }
  const COMPACT_HERO_CARD = 'gap-1 py-2 lg:gap-2 lg:py-4'
  const COMPACT_HERO_TITLE = 'text-[10px] font-medium uppercase tracking-wide text-muted-foreground lg:text-xs'
  const COMPACT_HERO_VALUE = 'data-num text-lg font-semibold tabular-nums lg:text-2xl'
</script>

<Card.Root class={COMPACT_HERO_CARD}>
  <Card.Content class="flex items-start justify-between gap-2 px-3 lg:px-4">
    <div class="grid gap-0.5 lg:gap-1">
      <Card.Title class={COMPACT_HERO_TITLE}>{label}</Card.Title>
      <Card.Description class={COMPACT_HERO_VALUE}>{value}</Card.Description>
    </div>
    <span class={`rounded-md p-1.5 lg:p-2 ${toneChip[tone] ?? toneChip.slate}`} aria-hidden="true">
      {#if Icon}<Icon class="size-3.5 lg:size-4" />{/if}
    </span>
  </Card.Content>
  {#if sub}
    <Card.Content class="px-3 lg:px-4">
      <p class="text-[10px] leading-tight text-muted-foreground lg:text-[11px]">{sub}</p>
    </Card.Content>
  {/if}
</Card.Root>
