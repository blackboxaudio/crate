<script lang="ts">
	import type { TrackColor } from '$lib/types'
	import { TRACK_COLORS } from '$lib/types'

	type Props = {
		color: TrackColor | null
		disabled?: boolean
	}

	let { color, disabled = false }: Props = $props()

	const currentColor = $derived(color ? TRACK_COLORS.find((c) => c.id === color) : null)
</script>

<div class="relative flex justify-center">
	<button
		type="button"
		class="h-4 w-4 cursor-pointer rounded-full border transition-all duration-100 {!color
			? 'border-stroke-subtle bg-transparent'
			: 'border-transparent'}"
		style={currentColor ? `background-color: ${currentColor.hex};` : ''}
		onclick={() => {}}
		{disabled}
		aria-label={color ? `Color: ${color}` : 'No color'}
	></button>
</div>
