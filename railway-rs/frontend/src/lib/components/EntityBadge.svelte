<script>
  import StatusBadge from './badges/status-badge.svelte';
  import { prefetch } from '$lib/api.js';
  let { type='train', code='', number='', name='', link=true, size='sm', class: className='', ...rest }=$props();
  const val=$derived(type==='station'? String(code??'').trim().toUpperCase(): String(number??code??'').trim());
  const href=$derived(!link||!val? '': type==='station'? `/station/${encodeURIComponent(val)}`: `/train/${encodeURIComponent(val)}`);
  const label=$derived(val||'—');
  const prefetchPath=$derived(!val? '': type==='station'? `/rail-api/stations/${encodeURIComponent(val)}`: `/rail-api/schedule?train=${encodeURIComponent(val)}`);
  const tracking=$derived(type==='station'?'tracking-[0.14em] uppercase':'tracking-tight');
  let warmedHref=''; function warm(){ if(!prefetchPath||warmedHref===prefetchPath) return; warmedHref=prefetchPath; prefetch(prefetchPath); }
</script>
<StatusBadge tone="neutral" {href} {size} title={name?`${label} · ${name}`:label} class="data-num {tracking} {className}" aria-label="{type==='station'?'Station':'Train'} {label}" onpointerenter={warm} ontouchstart={warm} {...rest}>{label}</StatusBadge>
