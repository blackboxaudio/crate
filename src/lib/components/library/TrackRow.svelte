<script lang="ts">
	import { fade } from 'svelte/transition'
	import type { Track, TrackColor } from '$lib/types'
	import { formatDurationCompact, formatBpm, formatKey, getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils'
	import { TagChip } from '$lib/components/tags'
	import Icon from '$lib/components/common/Icon.svelte'
	import { AlbumArt, AlbumArtModal, Spinner, Text, Tooltip } from '$lib/components/common'
	import { missingTrackIds, dragStore, isDraggingTag, keyNotationFormat } from '$lib/stores'
	import { translate } from '$lib/i18n'
	import { DRAG_THRESHOLD, getDistance } from '$lib/utils/drag'
	import TrackColorCell from './TrackColorCell.svelte'

	type Props = {
		track: Track
		selected?: boolean
		playing?: boolean
		analyzing?: boolean
		dragTrackIds?: string[]
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		onclick?: (e: MouseEvent) => void
		ondblclick?: (e: MouseEvent) => void
		oncontextmenu?: (e: MouseEvent) => void
		onColorChange?: (color: TrackColor | null) => void
		onCancelAnalysis?: () => void
	}

	let {
		track,
		selected = false,
		playing = false,
		analyzing = false,
		dragTrackIds = [],
		categoryColors,
		categorySortOrders,
		onclick,
		ondblclick,
		oncontextmenu,
		onColorChange,
		onCancelAnalysis,
	}: Props = $props()

	let showArtworkModal = $state(false)
	let isHoveringColorCell = $state(false)
	let isTagDragHovered = $state(false)

	// Clear hover when tag drag ends
	$effect(() => {
		if (!$isDraggingTag) isTagDragHovered = false
	})

	// Track pointer state for drag detection
	let pointerStartPos: { x: number; y: number } | null = null
	let isDragStarted = false

	function handlePointerDown(e: PointerEvent) {
		// Only handle primary button (left click)
		if (e.button !== 0) return

		// Don't start drag on interactive elements
		const target = e.target as HTMLElement
		if (target.closest('button, [role="button"]')) return

		pointerStartPos = { x: e.clientX, y: e.clientY }
		isDragStarted = false
	}

	function handlePointerMove(e: PointerEvent) {
		if (!pointerStartPos) return

		const distance = getDistance(pointerStartPos.x, pointerStartPos.y, e.clientX, e.clientY)

		// Start drag if threshold exceeded
		if (!isDragStarted && distance >= DRAG_THRESHOLD) {
			isDragStarted = true

			// Determine which tracks to drag
			const trackIds = selected && dragTrackIds.length > 0 ? dragTrackIds : [track.id]

			// Start the drag via the store
			dragStore.startTrackDrag(trackIds, e.clientX, e.clientY)
		}
	}

	function handlePointerUp() {
		pointerStartPos = null
		isDragStarted = false
	}

	function handleArtworkClick() {
		if (track.artwork_path) {
			showArtworkModal = true
		}
	}

	const isMissing = $derived($missingTrackIds.has(track.id))
</script>

<div
	role="row"
	tabindex="0"
	data-track-row
	data-track-id={track.id}
	class="relative grid cursor-pointer grid-cols-[24px_40px_1fr_1fr_80px_60px_80px_1fr] items-center gap-2 border-b border-stroke-subtle px-3 py-1 text-sm transition-colors select-none {selected
		? 'bg-brand-muted'
		: 'hover:bg-surface-2/50'} {playing ? 'text-brand-primary' : 'text-text-secondary'} {isMissing
		? 'bg-red-500/5'
		: ''} {isTagDragHovered ? 'bg-brand-primary/10 ring-1 ring-brand-primary ring-inset' : ''}"
	{onclick}
	{ondblclick}
	{oncontextmenu}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
	onpointercancel={handlePointerUp}
	onpointerenter={() => $isDraggingTag && (isTagDragHovered = true)}
	onpointerleave={() => (isTagDragHovered = false)}
	onkeydown={(e) => e.key === 'Enter' && ondblclick?.(e)}
>
	<!-- Missing file indicator -->
	{#if isMissing}
		<div class="pointer-events-none absolute inset-0 border-l-2 border-red-500/50"></div>
	{/if}
	<!-- Color -->
	<div
		role="presentation"
		class="relative flex h-full items-center justify-center"
		onmouseenter={() => (isHoveringColorCell = true)}
		onmouseleave={() => (isHoveringColorCell = false)}
	>
		{#if analyzing}
			{#if isHoveringColorCell && onCancelAnalysis}
				<Tooltip text={$translate('contextMenu.stopAnalysis')} position="right">
					<div transition:fade={{ duration: 150 }}>
						<button
							type="button"
							class="flex h-5 w-5 cursor-pointer items-center justify-center rounded transition-colors hover:bg-red-500/20"
							onclick={(e) => {
								e.stopPropagation()
								onCancelAnalysis?.()
							}}
						>
							<Icon name="x" class="h-3 w-3 text-red-500" />
						</button>
					</div>
				</Tooltip>
			{:else}
				<div class="absolute inset-0 flex items-center justify-center" transition:fade={{ duration: 150 }}>
					<Spinner class="h-3 w-3" />
				</div>
			{/if}
		{:else}
			<div transition:fade={{ duration: 150 }}>
				<TrackColorCell color={track.color} onselect={onColorChange} />
			</div>
		{/if}
	</div>

	<!-- Artwork -->
	<div class="flex justify-center">
		<AlbumArt
			artworkPath={track.artwork_path}
			size="xs"
			onclick={handleArtworkClick}
			class={track.artwork_path ? 'cursor-zoom-in' : ''}
		/>
	</div>

	<!-- Title -->
	<div class="flex items-center truncate font-medium {playing ? 'text-brand-primary' : 'text-text-primary'}">
		{#if isMissing}
			<span class="mr-1.5 flex-shrink-0" title="File not found">
				<Icon name="warning" class="h-3.5 w-3.5 text-red-500" />
			</span>
			<!--{:else if playing}-->
			<!--	<span class="mr-1 inline-block w-4 flex-shrink-0">-->
			<!--		<Icon name="play" class="h-3 w-3 animate-pulse" fill />-->
			<!--	</span>-->
		{/if}
		<span class="truncate">{getTrackDisplayName(track)}</span>
	</div>

	<!-- Artist -->
	<div class="truncate text-text-secondary">
		{getTrackDisplayArtist(track)}
	</div>

	<!-- BPM -->
	<div class="text-right text-text-secondary tabular-nums">
		{formatBpm(track.bpm)}
	</div>

	<!-- Key -->
	<div class="flex items-center justify-center text-text-secondary">
		{formatKey(track.key, $keyNotationFormat)}
	</div>

	<!-- Duration -->
	<div class="text-right text-text-secondary tabular-nums">
		{formatDurationCompact(track.duration_ms)}
	</div>

	<!-- Tags -->
	<div class="flex h-6 items-center gap-1 overflow-hidden">
		{#each track.tags
			.toSorted((a, b) => {
				const orderA = categorySortOrders?.get(a.category_id) ?? 0
				const orderB = categorySortOrders?.get(b.category_id) ?? 0
				if (orderA !== orderB) return orderA - orderB
				return a.name.localeCompare(b.name)
			})
			.slice(0, 3) as tag (tag.id)}
			<TagChip {tag} size="sm" color={categoryColors?.get(tag.category_id)} />
		{/each}
		{#if track.tags.length > 3}
			<Text variant="caption">+{track.tags.length - 3}</Text>
		{/if}
	</div>
</div>

{#if showArtworkModal}
	<AlbumArtModal
		open={showArtworkModal}
		artworkPath={track.artwork_path}
		trackTitle={getTrackDisplayName(track)}
		onClose={() => (showArtworkModal = false)}
	/>
{/if}
