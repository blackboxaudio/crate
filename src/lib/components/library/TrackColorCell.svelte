<script lang="ts">
	import type { TrackColor } from '$lib/types'
	import { TRACK_COLORS } from '$lib/types'

	type Props = {
		color: TrackColor | null
		disabled?: boolean
		onselect?: (color: TrackColor | null) => void
	}

	let { color, disabled = false, onselect }: Props = $props()

	let showDropdown = $state(false)
	let cellEl: HTMLDivElement | undefined = $state()

	function handleClick(e: MouseEvent) {
		if (disabled) return
		e.stopPropagation()
		showDropdown = !showDropdown
	}

	function handleSelect(selectedColor: TrackColor | null) {
		onselect?.(selectedColor)
		showDropdown = false
	}

	function handleClickOutside(e: MouseEvent) {
		if (cellEl && !cellEl.contains(e.target as Node)) {
			showDropdown = false
		}
	}

	$effect(() => {
		if (showDropdown) {
			document.addEventListener('click', handleClickOutside)
			return () => document.removeEventListener('click', handleClickOutside)
		}
	})

	const currentColor = $derived(color ? TRACK_COLORS.find((c) => c.id === color) : null)
</script>

<div bind:this={cellEl} class="relative flex justify-center">
	<button
		type="button"
		class="h-4 w-4 rounded-full border transition-transform hover:scale-110 {!color
			? 'border-stroke-subtle bg-transparent'
			: 'border-transparent'}"
		style={currentColor ? `background-color: ${currentColor.hex};` : ''}
		onclick={handleClick}
		{disabled}
		aria-label={color ? `Color: ${color}` : 'No color'}
	></button>

	{#if showDropdown}
		<div class="absolute top-6 z-50 rounded-md border border-stroke bg-surface-1 p-2 shadow-lg" role="menu">
			<div class="grid grid-cols-4 gap-1">
				{#each TRACK_COLORS as colorOption (colorOption.id)}
					<button
						type="button"
						class="h-6 w-6 rounded-full transition-transform hover:scale-110 {color === colorOption.id
							? 'ring-2 ring-text-primary ring-offset-1 ring-offset-surface-1'
							: ''}"
						style="background-color: {colorOption.hex};"
						onclick={() => handleSelect(colorOption.id)}
						title={colorOption.label}
						role="menuitem"
					></button>
				{/each}
			</div>
			<button
				type="button"
				class="mt-2 w-full rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-2"
				onclick={() => handleSelect(null)}
				role="menuitem"
			>
				Remove color
			</button>
		</div>
	{/if}
</div>
