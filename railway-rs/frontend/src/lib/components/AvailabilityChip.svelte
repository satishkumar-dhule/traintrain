<script>
  import { cn } from '$lib/utils.js'
  import LoaderCircleIcon from 'lucide-svelte/icons/loader-circle'

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

  const statusKind = $derived.by(() => {
    const s = String(status ?? '').trim().toUpperCase()
    if (/^AVAILABLE/.test(s)) return 'available'
    if (/^RAC/.test(s)) return 'rac'
    if (/^(WL|WAITLIST|GNWL|RLWL|PQWL|TQWL)/.test(s)) return 'waitlist'
    if (/^REGRET/.test(s)) return 'regret'
    if (s === 'REFRESH' || s === '') return 'idle'
    return 'other'
  })

  const tone = $derived(
    {
      available: 'border-emerald-600/30 bg-emerald-500/10 text-emerald-700 dark:border-emerald-500/35 dark:bg-emerald-400/10 dark:text-emerald-400',
      rac: 'border-amber-600/30 bg-amber-500/10 text-amber-700 dark:border-amber-500/35 dark:bg-amber-400/10 dark:text-amber-400',
      waitlist: 'border-red-600/30 bg-red-500/10 text-red-700 dark:border-red-500/35 dark:bg-red-400/10 dark:text-red-400',
      regret: 'border-red-600/30 bg-red-500/10 text-red-700 dark:border-red-500/35 dark:bg-red-400/10 dark:text-red-400',
      idle: 'border-border bg-muted/50 text-muted-foreground',
      other: 'border-border bg-muted/50 text-muted-foreground',
    }[statusKind],
  )
</script>

<div class={cn('overflow-hidden rounded-lg border px-3 py-2 text-xs', tone, className)}>
  <div class="flex items-baseline justify-between gap-2">
    <span class="font-mono text-sm font-semibold">{fmtStatus(classCode)}</span>
    {#if fare != null}
      <span class="font-mono text-sm tabular-nums">₹{fare.toLocaleString('en-IN')}</span>
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
