<script lang="ts">
	// Skeleton placeholders for the release detail's track list while its metadata (and tracks) are being
	// fetched. Each row mirrors a real track row's shape — a number slot, a flexible name bar, and a short
	// duration bar at the same min-h-[44px]/gap/padding — so the real rows drop in without shifting the
	// layout. Rows pulse in unison and go static under prefers-reduced-motion. Decorative: the surrounding
	// track-list container owns the accessible "loading" label, so these are aria-hidden.
	type Props = {
		/** How many placeholder rows to render. */
		count?: number
	}
	let { count = 8 }: Props = $props()

	// Slightly varied name-bar widths so the placeholder reads as a track list rather than a uniform grid.
	const WIDTHS = ['w-3/5', 'w-4/5', 'w-1/2', 'w-2/3', 'w-3/4', 'w-2/5']
	const rows = $derived(Array.from({ length: count }, (_, i) => WIDTHS[i % WIDTHS.length]))
</script>

<div class="flex flex-col" aria-hidden="true">
	{#each rows as width, i (i)}
		<div class="flex min-h-[44px] w-full animate-pulse items-center gap-3 py-2 pr-10 pl-2 motion-reduce:animate-none">
			<span class="w-5 flex-shrink-0">
				<span class="mx-auto block h-3.5 w-3 rounded bg-surface-2"></span>
			</span>
			<span class="h-3.5 min-w-0 flex-1 rounded bg-surface-2 {width}"></span>
			<span class="h-3 w-8 flex-shrink-0 rounded bg-surface-2"></span>
		</div>
	{/each}
</div>
