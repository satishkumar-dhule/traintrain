<script>
	import { cn } from "$lib/utils.js";
	let {
		ref = $bindable(null),
		class: className,
		children,
		size = "default",
		...restProps
	} = $props();
</script>

<div
	bind:this={ref}
	data-slot="card"
	data-size={size}
	class={cn("gap-(--card-spacing) overflow-hidden rounded-xl border bg-card py-(--card-spacing) text-sm text-card-foreground shadow-[inset_0_1px_0_oklch(1_0_0_/_0.7)] dark:shadow-[inset_0_1px_0_oklch(1_0_0_/_0.08)] [border-color:oklch(0.85_0.02_258)] dark:border-white/10 [--card-spacing:--spacing(4)] has-data-[slot=card-footer]:pb-0 has-[>img:first-child]:pt-0 data-[size=sm]:[--card-spacing:--spacing(3)] data-[size=sm]:has-data-[slot=card-footer]:pb-0 *:[img:first-child]:rounded-t-xl *:[img:last-child]:rounded-b-xl group/card flex flex-col", className)}
	{...restProps}
>
	{@render children?.()}
</div>