<script module>
  export const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
  export const DAY_SHORT = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  export const DAY_NAMES = [
    'Monday',
    'Tuesday',
    'Wednesday',
    'Thursday',
    'Friday',
    'Saturday',
    'Sunday'
  ];

  /* Normalize any running-days shape into a bool[7] (Mon..Sun):
     - bool[7] / 0-1 array
     - array of names ("Mon", "Monday", …)
     - string like "M-TW-FS-" or "MTTFSS"
     - object with keys like monday…sunday or trainRunsOnMon…trainRunsOnSun
     Returns null when nothing usable is supplied. */
  export function dayFlags(value) {
    if (value == null) return null;
    if (Array.isArray(value)) {
      if (value.length && value.every((v) => typeof v === 'boolean' || v == null)) {
        return [0, 1, 2, 3, 4, 5, 6].map((i) => Boolean(value[i]));
      }
      const set = new Set(
        value
          .filter(Boolean)
          .map((v) => String(v).trim().toLowerCase().slice(0, 3))
      );
      return DAY_SHORT.map((d) => set.has(d.toLowerCase()));
    }
    if (typeof value === 'object') {
      return [0, 1, 2, 3, 4, 5, 6].map((i) => {
        const short = DAY_SHORT[i];
        return (
          value[DAY_NAMES[i].toLowerCase()] === true ||
          value[`trainRunsOn${short}`] === true ||
          value[`runsOn${short}`] === true
        );
      });
    }
    const s = String(value).trim();
    if (!s) return null;
    const clean = s.replace(/[\s,|/]+/g, '');
    if (/^[MTWFSmtwfs-]{7}$/.test(clean)) {
      return [...clean].map((c) => c !== '-');
    }
    const flags = dayFlags(clean.split(''));
    return flags;
  }

  /* "M-W-F" summary of a bool[7], or '' when none. */
  export function daysSummary(flags, sep = '') {
    if (!flags) return '';
    return flags.map((on, i) => (on ? DAY_LETTERS[i] : '-')).join(sep);
  }
</script>

<script>
  import { Badge } from '$lib/components/ui/badge/index.js';
  import { cn } from '$lib/utils.js';

  let {
    days,
    flags = null, // pre-parsed bool[7]; wins over `days` when supplied
    format = 'letter', // 'letter' | 'short'
    class: className = '',
    ...rest
  } = $props();

  const resolved = $derived(flags ?? dayFlags(days));
  const labels = $derived(format === 'short' ? DAY_SHORT : DAY_LETTERS);
  const activeNames = $derived(
    resolved ? DAY_NAMES.filter((_, i) => resolved[i]) : []
  );
</script>

{#if resolved}
  <span
    class={cn('inline-flex flex-wrap items-center gap-1', className)}
    role="img"
    aria-label={activeNames.length
      ? `Runs on ${activeNames.join(', ')}`
      : 'No running days'}
    {...rest}
  >
    {#each labels as label, i (i)}
      <Badge
        variant={resolved[i] ? 'secondary' : 'outline'}
        class={cn(
          'h-5 min-w-5 justify-center px-1.5 font-medium text-[10px]',
          !resolved[i] && 'text-muted-foreground opacity-40'
        )}
        title={`${DAY_NAMES[i]}: ${resolved[i] ? 'runs' : 'no run'}`}
        aria-hidden="true"
      >
        {label}
      </Badge>
    {/each}
  </span>
{:else}
  <span class="text-muted-foreground">—</span>
{/if}
