<script>
  import { api } from '$lib/api.js';
  import { navigate } from '$lib/router.svelte.js';
  import { StatusBadge } from '$lib/components/badges/index.js';
  import { cn } from '$lib/utils.js';

  let { class: className = '' } = $props();

  let status = $state({ phase: 'loading', data: null });

  async function load() {
    const res = await api('/rail-api/source-status');
    if (res.ok) status = { phase: 'success', data: res.data };
    else status = { phase: 'error', data: null };
  }

  $effect(() => {
    load();
  });

  const health = $derived.by(() => {
    const d = status.data;
    if (!d) return { tone: 'neutral', label: 'Sources…' };
    const sources = Array.isArray(d.sources) ? d.sources : [];
    const reachable = sources.filter((s) => s.reachable).length;
    if (!d.live_enabled) return { tone: 'danger', label: 'Live off' };
    if (reachable === 0) return { tone: 'danger', label: 'Offline' };
    if (reachable < sources.length) return { tone: 'warning', label: 'Partial' };
    return { tone: 'success', label: 'Live data' };
  });
</script>

<button type="button" onclick={() => navigate('/system')} class={cn('inline-flex items-center gap-1.5 rounded-full border border-border bg-card px-2.5 py-1 text-xs font-medium transition-colors hover:bg-muted', className)} title="Data source status — click for details">
  {#if status.phase === 'loading'}
    <span class="size-2 rounded-full bg-muted-foreground/40"></span><span class="text-muted-foreground">Sources…</span>
  {:else}
    <StatusBadge tone={health.tone} dot>{health.label}</StatusBadge>
  {/if}
</button>
