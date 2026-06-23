<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { lightTap } from '$lib/utils/haptics'

	type Props = {
		releases: DiscoveryRelease[]
		onReorder: (releaseIds: string[]) => void
	}
	let { releases, onReorder }: Props = $props()

	let items = $derived<DiscoveryRelease[]>([...releases])
	let dragIndex = $state<number | null>(null)
	let overIndex = $state<number | null>(null)
	let pointerId = $state<number | null>(null)
	let startY = 0
	let currentY = 0
	let rowHeight = 0
	let containerEl: HTMLDivElement | undefined = $state()

	let longPressTimer = 0

	const LONG_PRESS_MS = 300

	function onPointerDown(e: PointerEvent, index: number) {
		if (pointerId !== null) return
		pointerId = e.pointerId
		startY = e.clientY
		currentY = e.clientY

		const row = e.currentTarget as HTMLElement
		rowHeight = row.offsetHeight

		longPressTimer = window.setTimeout(() => {
			longPressTimer = 0
			void lightTap()
			dragIndex = index
			overIndex = index
			;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
		}, LONG_PRESS_MS)

		window.addEventListener('pointermove', onPointerMove)
		window.addEventListener('pointerup', onPointerUp)
		window.addEventListener('pointercancel', onPointerUp)
	}

	function onPointerMove(e: PointerEvent) {
		if (e.pointerId !== pointerId) return
		currentY = e.clientY

		if (dragIndex === null) {
			if (Math.abs(currentY - startY) > 10) {
				clearTimeout(longPressTimer)
				longPressTimer = 0
				cleanup()
			}
			return
		}

		const dy = currentY - startY
		const rawTarget = dragIndex + Math.round(dy / rowHeight)
		overIndex = Math.max(0, Math.min(items.length - 1, rawTarget))
	}

	function onPointerUp(e: PointerEvent) {
		if (e.pointerId !== pointerId) return

		if (longPressTimer) {
			clearTimeout(longPressTimer)
			longPressTimer = 0
		}

		if (dragIndex !== null && overIndex !== null && dragIndex !== overIndex) {
			const reordered = [...items]
			const [moved] = reordered.splice(dragIndex, 1)
			reordered.splice(overIndex, 0, moved)
			items = reordered
			onReorder(reordered.map((r) => r.id))
		}

		cleanup()
	}

	function cleanup() {
		dragIndex = null
		overIndex = null
		pointerId = null
		window.removeEventListener('pointermove', onPointerMove)
		window.removeEventListener('pointerup', onPointerUp)
		window.removeEventListener('pointercancel', onPointerUp)
	}

	function getTransform(index: number): string {
		if (dragIndex === null || overIndex === null) return ''
		if (index === dragIndex) {
			return `translateY(${(overIndex - dragIndex) * rowHeight}px)`
		}
		if (dragIndex < overIndex && index > dragIndex && index <= overIndex) {
			return `translateY(-${rowHeight}px)`
		}
		if (dragIndex > overIndex && index < dragIndex && index >= overIndex) {
			return `translateY(${rowHeight}px)`
		}
		return ''
	}
</script>

<div bind:this={containerEl} class="flex flex-col">
	{#each items as release, index (release.id)}
		{@const isDragging = dragIndex === index}
		<div
			class="flex touch-none items-center gap-3 px-4 py-3 {isDragging
				? 'relative z-10 scale-[1.02] bg-surface-2 shadow-lg'
				: 'bg-surface-0'} {dragIndex !== null && !isDragging ? 'transition-transform duration-150 ease-out' : ''}"
			style={dragIndex !== null ? `transform: ${getTransform(index)}` : ''}
			onpointerdown={(e) => onPointerDown(e, index)}
		>
			<svg class="h-5 w-5 flex-shrink-0 text-text-tertiary" viewBox="0 0 24 24" fill="currentColor">
				<rect x="4" y="5" width="16" height="2" rx="1" />
				<rect x="4" y="11" width="16" height="2" rx="1" />
				<rect x="4" y="17" width="16" height="2" rx="1" />
			</svg>

			{#if release.artwork_url}
				<img src={release.artwork_url} alt="" class="h-10 w-10 flex-shrink-0 rounded object-cover" />
			{:else}
				<div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary">
					<svg viewBox="0 0 24 24" class="h-4 w-4" fill="currentColor">
						<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
					</svg>
				</div>
			{/if}

			<div class="flex min-w-0 flex-1 flex-col leading-tight">
				<span class="truncate text-sm font-medium text-text-primary">
					{release.title ?? $translate('common.untitled')}
				</span>
				<span class="truncate text-xs text-text-secondary">
					{release.artist ?? $translate('common.unknownArtist')}
				</span>
			</div>
		</div>
	{/each}
</div>
