<script lang="ts">
	import type { Snippet } from 'svelte'

	// Touch-optimized list row with a guaranteed 44pt minimum height (Apple HIG touch target). Renders
	// as a native <button> when interactive (keyboard-accessible, correct semantics) and a plain <div>
	// otherwise. Optional leading (artwork/icon) and trailing (count/chevron) slots flank the content.
	type Props = {
		onclick?: (e: MouseEvent) => void
		selected?: boolean
		disabled?: boolean
		leading?: Snippet
		trailing?: Snippet
		children: Snippet
		ariaLabel?: string
		/** Bindable handle to the row's root element — lets a list scroll a specific row into view. */
		ref?: HTMLElement | null
	}

	let {
		onclick,
		selected = false,
		disabled = false,
		leading,
		trailing,
		children,
		ariaLabel,
		ref = $bindable(null),
	}: Props = $props()

	const base = 'flex min-h-[44px] w-full items-center gap-3 px-4 py-2 text-left transition-colors select-none'
</script>

{#snippet inner()}
	{#if leading}
		<span class="flex-shrink-0">{@render leading()}</span>
	{/if}
	<span class="min-w-0 flex-1">{@render children()}</span>
	{#if trailing}
		<span class="flex-shrink-0 text-text-tertiary">{@render trailing()}</span>
	{/if}
{/snippet}

{#if onclick}
	<button
		type="button"
		bind:this={ref}
		{disabled}
		aria-label={ariaLabel}
		aria-pressed={selected}
		{onclick}
		class="{base} cursor-pointer disabled:opacity-40 {selected
			? 'bg-brand-muted text-text-primary'
			: 'text-text-primary active:bg-surface-2'}"
	>
		{@render inner()}
	</button>
{:else}
	<div
		bind:this={ref}
		aria-label={ariaLabel}
		class="{base} {selected ? 'bg-brand-muted text-text-primary' : 'text-text-primary'}"
	>
		{@render inner()}
	</div>
{/if}
