<script lang="ts">
	import type { Tag, TagSelectionState } from '$lib/types'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		tag: Tag
		size?: 'sm' | 'md'
		color?: string | null
		removable?: boolean
		state?: TagSelectionState
		selectionCount?: number
		selectionTotal?: number
		onclick?: () => void
		onremove?: () => void
		oncontextmenu?: (e: MouseEvent) => void
	}

	let {
		tag,
		size = 'md',
		color,
		removable = false,
		state,
		selectionCount,
		selectionTotal,
		onclick,
		onremove,
		oncontextmenu,
	}: Props = $props()

	const sizeStyles = {
		sm: 'px-1.5 py-0.5 text-xs',
		md: 'px-2 py-1 text-xs',
	}

	// Use provided color, then tag color, then default
	let bgColor = $derived(color || tag.color || '#6366f1')

	// Compute state-based styles
	let stateStyles = $derived(() => {
		if (!state) return ''
		switch (state) {
			case 'active':
				return ''
			case 'inactive':
				return 'opacity-40 grayscale'
			case 'mixed':
				return ''
		}
	})

	let showMixedBadge = $derived(state === 'mixed' && selectionCount !== undefined && selectionTotal !== undefined)
</script>

<span
	role={onclick ? 'button' : 'presentation'}
	tabindex="-1"
	class="inline-flex items-center gap-1 rounded font-medium {sizeStyles[size]} {stateStyles()} {onclick
		? 'cursor-pointer hover:opacity-80'
		: ''}"
	style="background-color: {bgColor}20; color: {bgColor}; border: 1px solid {bgColor}40;"
	data-tag
	{onclick}
	{oncontextmenu}
	onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
	{tag.name}
	{#if showMixedBadge}
		<span class="ml-0.5 text-[10px] opacity-60">{selectionCount}/{selectionTotal}</span>
	{/if}
	{#if removable && onremove}
		<button
			type="button"
			aria-label="Remove tag"
			class="ml-0.5 hover:cursor-pointer hover:opacity-70"
			onclick={(e) => {
				e.stopPropagation()
				onremove()
			}}
		>
			<Icon name="x" class="h-3 w-3" />
		</button>
	{/if}
</span>
