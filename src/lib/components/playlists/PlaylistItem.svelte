<script lang="ts">
	import type { Playlist } from '$lib/types'
	import { dragStore, hoveredDropTarget, isDraggingTracks, isDraggingPlaylist } from '$lib/stores'
	import { DRAG_THRESHOLD, getDistance } from '$lib/utils/drag'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		playlist: Playlist
		playlists?: Playlist[]
		selected?: boolean
		isContextMenuActive?: boolean
		depth?: number
		expanded?: boolean
		hasChildren?: boolean
		onclick?: () => void
		onToggle?: () => void
		oncontextmenu?: (e: MouseEvent) => void
		onTracksDrop?: (trackIds: string[]) => void
		onPlaylistDrop?: (playlistId: string) => void
	}

	let {
		playlist,
		playlists = [],
		selected = false,
		isContextMenuActive = false,
		depth = 0,
		expanded = false,
		hasChildren = false,
		onclick,
		onToggle,
		oncontextmenu,
		onTracksDrop,
		onPlaylistDrop,
	}: Props = $props()

	let paddingLeft = $derived(`${depth * 12 + 8}px`)

	// Determine the drop target type for this item
	const dropTargetType = $derived(playlist.is_folder ? 'folder' : 'playlist')
	const dropTargetId = $derived(`${dropTargetType}-${playlist.id}`)

	// Determine if this item is a valid drop target based on what's being dragged
	const isValidDropTarget = $derived.by(() => {
		// Playlists accept track drops
		if ($isDraggingTracks && !playlist.is_folder) return true
		// Folders accept playlist drops
		if ($isDraggingPlaylist && playlist.is_folder) return true
		return false
	})

	// Check if this item is currently being hovered during a valid drag
	const isHovered = $derived($hoveredDropTarget === dropTargetId && isValidDropTarget)

	// Track pointer state for drag detection (for dragging playlists/folders)
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

			// Start the drag via the store
			dragStore.startPlaylistDrag(playlist.id, playlist.is_folder, e.clientX, e.clientY)
		}
	}

	function handlePointerUp() {
		pointerStartPos = null
		isDragStarted = false
	}
</script>

<div
	role="treeitem"
	tabindex="0"
	data-drop-target={dropTargetId}
	aria-selected={selected}
	aria-expanded={playlist.is_folder ? expanded : undefined}
	class="flex cursor-pointer items-center gap-2 rounded py-1.5 pr-3 transition-all select-none {selected ||
	isContextMenuActive
		? 'bg-brand-muted text-text-primary'
		: isHovered
			? 'bg-brand-muted text-text-primary ring-1 ring-brand-primary'
			: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
	style="padding-left: {paddingLeft}"
	{onclick}
	ondblclick={() => playlist.is_folder && onToggle?.()}
	{oncontextmenu}
	onpointerdown={handlePointerDown}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerUp}
	onpointercancel={handlePointerUp}
	onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
	<!-- Expand/Collapse toggle for folders -->
	{#if playlist.is_folder && hasChildren}
		<button
			type="button"
			aria-label={expanded ? 'Collapse' : 'Expand'}
			class="flex h-4 w-4 items-center justify-center text-text-tertiary hover:text-text-secondary"
			onclick={(e) => {
				e.stopPropagation()
				onToggle?.()
			}}
		>
			<Icon name="play" class="h-3 w-3 transition-transform {expanded ? 'rotate-90' : ''}" fill />
		</button>
	{:else}
		<span class="w-4"></span>
	{/if}

	<!-- Icon -->
	<span class="flex-shrink-0">
		{#if playlist.is_folder}
			<Icon name="folder" class="h-4 w-4" />
		{:else if playlist.is_smart}
			<Icon name="bolt" class="h-4 w-4" />
		{:else}
			<Icon name="music-note" class="h-4 w-4" />
		{/if}
	</span>

	<!-- Name -->
	<span class="flex-1 truncate text-sm">
		{playlist.name}
	</span>

	<!-- Track count -->
	{#if !playlist.is_folder}
		<span class="text-xs text-text-tertiary">
			{playlist.track_count}
		</span>
	{/if}
</div>
