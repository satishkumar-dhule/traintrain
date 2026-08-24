<script>
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	} = $props();
</script>

<TabsPrimitive.Trigger
	bind:ref
	data-slot="tabs-trigger"
	class={cn(
		/* Folder-tab trigger (Windows-style): natural width, top corners only,
		   transparent side/top borders that turn into a raised card-colored
		   tab when active. bits-ui emits data-state="active" on the selected
		   tab — style against that attribute, not `data-active`. */
		"relative inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-t-md border border-b-0 border-transparent px-3 py-1.5 text-sm font-medium text-muted-foreground transition-all hover:bg-accent/60 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='size-'])]:max-lg:size-[1.125rem] [&_svg]:pointer-events-none [&_svg]:shrink-0",
		"max-lg:min-h-11 max-lg:gap-2 max-lg:px-4 max-lg:py-2.5 max-lg:text-[0.9375rem]",
		/* Active folder: card surface + visible frame, dropped 1px over the
		   strip's baseline (desktop only; the scrolling strip stays flush on
		   touch) so its open side merges with the content below. */
		"lg:-mb-px",
		"data-[state=active]:z-10 data-[state=active]:border-border data-[state=active]:bg-card data-[state=active]:text-foreground data-[state=active]:shadow-xs",
		/* Vertical orientation fallback. */
		"group-data-[orientation=vertical]/tabs:w-full group-data-[orientation=vertical]/tabs:justify-start group-data-[orientation=vertical]/tabs:rounded-md",
		/* Underline ("line") variant keeps its flat look; indicator via ::after. */
		"group-data-[variant=line]/tabs-list:rounded-none group-data-[variant=line]/tabs-list:border-0 group-data-[variant=line]/tabs-list:bg-transparent group-data-[variant=line]/tabs-list:shadow-none group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent group-data-[variant=line]/tabs-list:data-[state=active]:shadow-none",
		"after:absolute after:bg-foreground after:opacity-0 after:transition-opacity max-lg:after:hidden group-data-[orientation=horizontal]/tabs:after:inset-x-0 group-data-[orientation=horizontal]/tabs:after:bottom-[-5px] group-data-[orientation=horizontal]/tabs:after:h-0.5 group-data-[orientation=vertical]/tabs:after:inset-y-0 group-data-[orientation=vertical]/tabs:after:-right-1 group-data-[orientation=vertical]/tabs:after:w-0.5 group-data-[variant=line]/tabs-list:data-[state=active]:after:opacity-100",
		className
	)}
	{...restProps}
/>
