<script module>
	import { tv } from "tailwind-variants";

	export const tabsListVariants = tv({
		base: "group/tabs-list inline-flex w-fit items-end gap-1 text-muted-foreground max-lg:w-full max-lg:max-w-full max-lg:overflow-x-auto max-lg:overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
		variants: {
			variant: {
				/* Windows-style folder strip: flat row sitting on a full-width
				   hairline; the active trigger (card surface, top corners, side
				   borders) reads as the open folder attached to the panel below. */
				default: "cn-tabs-list-variant-default rounded-t-md border-b border-border",
				line: "cn-tabs-list-variant-line gap-1 border-b border-transparent",
			},
		},
		defaultVariants: {
			variant: "default",
		},
	});
</script>

<script>
	import { Tabs as TabsPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		variant = "default",
		class: className,
		...restProps
	} = $props();
</script>

<TabsPrimitive.List
	bind:ref
	data-slot="tabs-list"
	data-variant={variant}
	class={cn(tabsListVariants({ variant }), className)}
	{...restProps}
/>
