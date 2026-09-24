<script lang="ts">
	// Conventional skeleton-loader rows for a list that's still loading. Each placeholder mirrors
	// MobileListItem's shape — a square artwork block plus two stacked text lines at the same row height
	// (h-12 artwork + py-2 ≈ 64px) — so the real rows drop in without shifting the layout. Rows pulse in
	// unison and go static under prefers-reduced-motion. Purely decorative: the surrounding container owns
	// the accessible "loading" label, so the rows are aria-hidden.
	type Props = {
		/** How many placeholder rows to render (default fills a typical mobile viewport). */
		count?: number
	}
	let { count = 10 }: Props = $props()

	// Slightly varied bar widths per row so the placeholder reads as content rather than a uniform grid.
	const WIDTHS = [
		['w-2/5', 'w-3/5'],
		['w-1/2', 'w-2/3'],
		['w-1/3', 'w-1/2'],
		['w-2/5', 'w-3/4'],
	]
	const rows = $derived(Array.from({ length: count }, (_, i) => WIDTHS[i % WIDTHS.length]))
</script>

<div class="flex flex-col" aria-hidden="true">
	{#each rows as [primary, secondary], i (i)}
		<div class="flex min-h-[44px] w-full animate-pulse items-center gap-3 px-4 py-2 motion-reduce:animate-none">
			<div class="h-12 w-12 flex-shrink-0 rounded bg-surface-2"></div>
			<div class="flex min-w-0 flex-1 flex-col gap-2">
				<div class="h-3.5 rounded bg-surface-2 {primary}"></div>
				<div class="h-3 rounded bg-surface-2 {secondary}"></div>
			</div>
		</div>
	{/each}
</div>
