<script>
  import { cn } from '$lib/utils.js';

  /* Signal lamp: the status dot drawn from railway signalling.
     tone: go (green) | hold (amber) | stop (red) | idle (neutral)
     pulse: gentle halo for live states; disabled under reduced motion. */
  let { tone = 'idle', pulse = false, class: className = '' } = $props();

  const tones = {
    go: 'bg-signal-go text-signal-go',
    hold: 'bg-signal-hold text-signal-hold',
    stop: 'bg-signal-stop text-signal-stop',
    idle: 'bg-muted-foreground/40 text-muted-foreground/40'
  };
</script>

<span
  aria-hidden="true"
  class={cn(
    'inline-block size-2 shrink-0 rounded-full',
    tones[tone] ?? tones.idle,
    pulse && 'lamp-pulse',
    className
  )}
></span>
