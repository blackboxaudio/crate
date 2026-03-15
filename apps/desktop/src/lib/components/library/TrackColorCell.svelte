<script lang="ts">
	import type { TrackColor } from '$shared/types'
	import { TRACK_COLORS } from '$shared/types'

	type Props = {
		color: TrackColor | null
		disabled?: boolean
		onselect?: (color: TrackColor | null) => void
	}

	let { color, disabled = false, onselect }: Props = $props()

	const currentColor = $derived(color ? TRACK_COLORS.find((c) => c.id === color) : null)
</script>

<div class="relative flex justify-center">
	<button
		type="button"
		class="h-2.5 w-2.5 cursor-pointer rounded-full border transition-all duration-100 {!color
			? 'border-stroke-subtle bg-transparent'
			: 'border-transparent'}"
		style={currentColor ? `background-color: ${currentColor.hex};` : ''}
		onclick={() => {}}
		{disabled}
		aria-label={color ? `Color: ${color}` : 'No color'}
	></button>
</div>
