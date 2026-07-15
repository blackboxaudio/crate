<script lang="ts">
	import type { DiscoveryRelease } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { lightTap } from '$lib/utils/haptics'
	import ReleaseCardContent from '$lib/components/discovery/ReleaseCardContent.svelte'

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

	// Drag starts immediately on the trailing handle (no long-press): a dedicated handle removes the
	// scroll-vs-drag ambiguity, and the rows themselves keep normal touch behavior so the list stays
	// scrollable while in reorder mode.
	function onPointerDown(e: PointerEvent, index: number) {
		if (pointerId !== null) return
		pointerId = e.pointerId
		startY = e.clientY
		currentY = e.clientY

		const handle = e.currentTarget as HTMLElement
		// The handle is a direct child of the row element.
		rowHeight = handle.parentElement?.offsetHeight ?? 72

		void lightTap()
		dragIndex = index
		overIndex = index
		handle.setPointerCapture(e.pointerId)

		window.addEventListener('pointermove', onPointerMove)
		window.addEventListener('pointerup', onPointerUp)
		window.addEventListener('pointercancel', onPointerUp)
	}

	function onPointerMove(e: PointerEvent) {
		if (e.pointerId !== pointerId || dragIndex === null) return
		currentY = e.clientY
		const dy = currentY - startY
		const rawTarget = dragIndex + Math.round(dy / rowHeight)
		overIndex = Math.max(0, Math.min(items.length - 1, rawTarget))
	}

	function onPointerUp(e: PointerEvent) {
		if (e.pointerId !== pointerId) return

		if (dragIndex !== null && overIndex !== null && dragIndex !== overIndex) {
			// Drop tick, completing the pickup tick in onPointerDown — only when the order actually changed.
			void lightTap()
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

<div class="flex flex-col">
	{#each items as release, index (release.id)}
		{@const isDragging = dragIndex === index}
		<!-- h-[72px] mirrors ReleaseFeedList's fixed rowHeight, and ReleaseCardContent is the same interior
		     the normal rows render — so toggling reorder mode doesn't reflow the list. The drag handle sits
		     in the trailing-accessory position (where the normal row's chevron lives). -->
		<div
			class="flex h-[72px] items-center gap-3 px-4 {isDragging
				? 'relative z-10 scale-[1.02] bg-surface-2 shadow-lg'
				: 'bg-surface-0'} {dragIndex !== null && !isDragging ? 'transition-transform duration-150 ease-out' : ''}"
			style={dragIndex !== null ? `transform: ${getTransform(index)}` : ''}
		>
			<ReleaseCardContent {release} />

			<button
				type="button"
				class="flex h-11 w-11 flex-shrink-0 touch-none items-center justify-center rounded-md text-text-tertiary"
				aria-label={$translate('queue.reorder')}
				onpointerdown={(e) => onPointerDown(e, index)}
			>
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
					<rect x="4" y="5" width="16" height="2" rx="1" />
					<rect x="4" y="11" width="16" height="2" rx="1" />
					<rect x="4" y="17" width="16" height="2" rx="1" />
				</svg>
			</button>
		</div>
	{/each}
</div>
