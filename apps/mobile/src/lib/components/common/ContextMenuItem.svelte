<script lang="ts">
	import type { Snippet } from 'svelte'

	// A single row inside a `ContextMenu` platter: a leading glyph followed by its label, destructive actions
	// tinted red, and an optional hairline grouping it from the rows above. Pair with `ContextMenu`.
	type Props = {
		/** Leading glyph, rendered to the left of the label. */
		icon?: Snippet
		/** Tint the row red — for destructive actions (Delete, Remove). */
		destructive?: boolean
		/** Draw a hairline divider above this row to start a new visual group. */
		separatorBefore?: boolean
		onclick?: () => void
		children: Snippet
	}

	let { icon, destructive = false, separatorBefore = false, onclick, children }: Props = $props()
</script>

{#if separatorBefore}
	<div class="my-1 border-t border-stroke-subtle"></div>
{/if}

<button
	type="button"
	role="menuitem"
	class="flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm active:bg-surface-2 {destructive
		? 'text-danger'
		: 'text-text-primary'}"
	{onclick}
>
	{#if icon}
		<span
			class="flex h-5 w-5 flex-shrink-0 items-center justify-center {destructive
				? 'text-danger'
				: 'text-text-secondary'}"
		>
			{@render icon()}
		</span>
	{/if}
	<span class="min-w-0 flex-1 truncate">{@render children()}</span>
</button>
