<script>
  import { cn } from '$lib/utils.js';
  import { Label } from '$lib/components/ui/label/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import AutoCompleteInput from '$lib/components/AutoCompleteInput.svelte';
  import ArrowDownUpIcon from 'lucide-svelte/icons/arrow-down-up';

  let {
    from = $bindable(''),
    to = $bindable(''),
    fromLabel = 'From',
    toLabel = 'To',
    fromId = 'sp-from',
    toId = 'sp-to',
    onFromPick = () => {},
    onToPick = () => {},
    onSwap = () => {},
    disabled = false,
    class: className = '',
    inputClass = '',
  } = $props();

  function pickFrom(item) {
    if (item?.code) from = String(item.code).toUpperCase();
    onFromPick(item);
  }

  function pickTo(item) {
    if (item?.code) to = String(item.code).toUpperCase();
    onToPick(item);
  }

  function swap() {
    const a = from;
    from = to;
    to = a;
    onSwap({ from, to });
  }
</script>

<div class={cn('flex flex-wrap items-end gap-2', className)}>
  <div class="grid min-w-44 flex-1 gap-1.5">
    <Label for={fromId}>{fromLabel}</Label>
    <AutoCompleteInput id={fromId} kind="station" placeholder="Station code…" bind:value={from} onpick={pickFrom} {disabled} inputClass={inputClass} class="min-w-0" />
  </div>
  <Button type="button" variant="outline" size="icon" onclick={swap} disabled={disabled} aria-label="Swap stations" title="Swap stations">
    <ArrowDownUpIcon />
  </Button>
  <div class="grid min-w-44 flex-1 gap-1.5">
    <Label for={toId}>{toLabel}</Label>
    <AutoCompleteInput id={toId} kind="station" placeholder="Station code…" bind:value={to} onpick={pickTo} {disabled} inputClass={inputClass} class="min-w-0" />
  </div>
</div>
