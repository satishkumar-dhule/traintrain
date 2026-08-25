<script>
  import { cn } from '$lib/utils.js'
  import LoaderCircleIcon from 'lucide-svelte/icons/loader-circle'
  import { availabilityStatusKind } from '$lib/components/badges/availability-status-badge.svelte'

  let {
    classCode = '',
    fare = null,
    status = '',
    refreshing = false,
    onRefresh = () => {},
    class: className = '',
  } = $props()

  function fmtStatus(s) {
    const t = String(s ?? '').trim()
    return t || '—'
  }

  // super-optimized: static tone map reused, shared kind fan-out
  const TONE_MAP = {
    available: 'border-signal-go/30 bg-signal-go/10 text-signal-go-ink dark:border-signal-go/40 dark:bg-signal-go/15',
    rac: 'border-primary/25 bg-primary/10 text-primary dark:border-primary/35 dark:bg-primary/15',
    waitlist: 'border-signal-hold/40 bg-signal-hold/15 text-signal-hold-ink dark:border-signal-hold/45 dark:bg-signal-hold/20',
    regret: 'border-signal-stop/30 bg-signal-stop/10 text-signal-stop-ink dark:border-signal-stop/40 dark:bg-signal-stop/15',
    closed: 'border-signal-stop/30 bg-signal-stop/10 text-signal-stop-ink dark:border-signal-stop/40 dark:bg-signal-stop/15',
    idle: 'border-border bg-muted/50 text-muted-foreground',
    other: 'border-border bg-muted/50 text-muted-foreground',
  }

  const rawStatus = $derived(String(status ?? '').trim())
  const statusKind = $derived.by(() => {
    if (!rawStatus || rawStatus.toUpperCase() === 'REFRESH') return 'idle'
    const k = availabilityStatusKind(rawStatus)
    if (k === 'closed') return 'regret'
    return k ?? 'other'
  })

  const tone = $derived(TONE_MAP[statusKind] ?? TONE_MAP.other)
</script>

<div class={cn('overflow-hidden rounded-lg border px-3 py-2 text-xs', tone, className)}>
  <div class="flex items-baseline justify-between gap-2">
    <span class="data-num text-sm font-semibold">{fmtStatus(classCode)}</span>
    {#if fare != null}
      <span class="data-num text-sm tabular-nums">₹{fare.toLocaleString('en-IN')}</span>
    {:else}
      <span class="text-muted-foreground">₹</span>
    {/if}
  </div>
  {#if statusKind === 'idle'}
    <button
      type="button"
      onclick={onRefresh}
      disabled={refreshing}
      class="mt-1 flex items-center gap-1 text-[11px] font-medium text-primary transition-colors hover:underline disabled:opacity-50"
    >
      {#if refreshing}
        <LoaderCircleIcon class="size-3 animate-spin" />
      {:else}
        <span class="text-primary">↻</span>
      {/if}
      Refresh
    </button>
  {:else}
    <div class="mt-1 min-w-0 truncate font-medium" title={fmtStatus(status)}>
      {fmtStatus(status)}
    </div>
  {/if}
</div>
