<script lang="ts">
	import type { Snippet } from 'svelte'

	// Thin list container: an optional uppercase section header, a column of rows (caller renders
	// MobileListItem children), and an optional empty state. Not virtualized — mobile feeds are short
	// for now; a virtualized variant can come with the full discovery feed.
	type Props = {
		title?: string
		children: Snippet
		empty?: Snippet
		isEmpty?: boolean
	}

	let { title, children, empty, isEmpty = false }: Props = $props()
</script>

<section class="flex flex-col">
	{#if title}
		<h2 class="px-4 pt-4 pb-1 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
			{title}
		</h2>
	{/if}

	{#if isEmpty && empty}
		<div class="px-4 py-6 text-sm text-text-secondary">{@render empty()}</div>
	{:else}
		<div class="flex flex-col">
			{@render children()}
		</div>
	{/if}
</section>
