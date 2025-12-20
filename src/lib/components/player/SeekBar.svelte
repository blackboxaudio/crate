<script lang="ts">
	import { formatDuration } from '$lib/utils'
	import { Text } from '$lib/components/common'

	type Props = {
		position: number
		duration: number
		disabled?: boolean
		onSeek?: (position: number) => void
	}

	let { position, duration, disabled = false, onSeek }: Props = $props()

	let isDragging = $state(false)
	let dragPosition = $state(0)

	let displayPosition = $derived(isDragging ? dragPosition : position)
	let progress = $derived(duration > 0 ? (displayPosition / duration) * 100 : 0)

	function handleMouseDown(e: MouseEvent) {
		if (disabled || duration === 0) return
		isDragging = true
		updatePositionFromEvent(e)
		window.addEventListener('mousemove', handleMouseMove)
		window.addEventListener('mouseup', handleMouseUp)
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isDragging) return
		updatePositionFromEvent(e)
	}

	function handleMouseUp() {
		if (isDragging) {
			onSeek?.(dragPosition)
			isDragging = false
		}
		window.removeEventListener('mousemove', handleMouseMove)
		window.removeEventListener('mouseup', handleMouseUp)
	}

	function updatePositionFromEvent(e: MouseEvent) {
		const bar = (e.target as HTMLElement).closest('.seek-bar')
		if (!bar) return
		const rect = bar.getBoundingClientRect()
		const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width))
		const percent = x / rect.width
		dragPosition = Math.floor(percent * duration)
	}
</script>

<div class="flex w-full items-center gap-2">
	<Text variant="caption" color="secondary" tabular class="w-10 text-right">
		{formatDuration(displayPosition)}
	</Text>

	<div
		role="slider"
		tabindex="0"
		aria-label="Seek"
		aria-valuemin={0}
		aria-valuemax={duration}
		aria-valuenow={displayPosition}
		class="seek-bar group relative h-1.5 flex-1 cursor-pointer rounded-full bg-surface-2"
		class:opacity-50={disabled}
		class:cursor-not-allowed={disabled}
		onmousedown={handleMouseDown}
	>
		<!-- Progress -->
		<div class="absolute inset-y-0 left-0 rounded-full bg-brand-primary" style="width: {progress}%"></div>

		<!-- Thumb -->
		<div
			class="absolute top-1/2 h-3 w-3 -translate-y-1/2 rounded-full bg-white opacity-0 transition-opacity group-hover:opacity-100"
			style="left: calc({progress}% - 6px)"
		></div>
	</div>

	<Text variant="caption" color="secondary" tabular class="w-10">
		{formatDuration(duration)}
	</Text>
</div>
