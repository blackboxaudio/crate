<script lang="ts">
	import type { Playlist } from '$lib/types'
	import { toastStore } from '$lib/stores'

	type Props = {
		playlist: Playlist
		playlists?: Playlist[]
		selected?: boolean
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

	// State for track drags (onto playlists)
	let isDragOver = $state(false)
	let dragEnterCounter = $state(0)

	// State for playlist drags (onto folders)
	let isPlaylistDragOver = $state(false)
	let playlistDragEnterCounter = $state(0)

	// Check if dataTransfer contains our custom track mime type
	function hasTrackData(dataTransfer: DataTransfer | null): boolean {
		if (!dataTransfer?.types) return false
		return Array.from(dataTransfer.types).includes('application/x-crate-tracks')
	}

	// Check if dataTransfer contains our custom playlist mime type
	function hasPlaylistData(dataTransfer: DataTransfer | null): boolean {
		if (!dataTransfer?.types) return false
		return Array.from(dataTransfer.types).includes('application/x-crate-playlist')
	}

	// Check if targetId is a descendant of potentialAncestorId (prevents circular drops)
	function isDescendantOf(potentialAncestorId: string, targetId: string): boolean {
		let currentId: string | null = targetId
		while (currentId) {
			if (currentId === potentialAncestorId) return true
			const current = playlists.find((p) => p.id === currentId)
			currentId = current?.parent_id ?? null
		}
		return false
	}

	// Handle drag start - make this playlist/folder draggable
	function handleDragStart(e: DragEvent) {
		if (!e.dataTransfer) return
		e.dataTransfer.effectAllowed = 'move'
		e.dataTransfer.setData(
			'application/x-crate-playlist',
			JSON.stringify({
				id: playlist.id,
				is_folder: playlist.is_folder,
			})
		)
		e.dataTransfer.setData('text/plain', playlist.name)
	}

	function handleDragOver(e: DragEvent) {
		// Accept track drops on playlists (not folders)
		if (!playlist.is_folder && hasTrackData(e.dataTransfer)) {
			e.preventDefault()
			e.stopPropagation()
			e.dataTransfer!.dropEffect = 'copy'
			return
		}

		// Accept playlist/folder drops on folders only
		if (playlist.is_folder && hasPlaylistData(e.dataTransfer)) {
			e.preventDefault()
			e.stopPropagation()
			e.dataTransfer!.dropEffect = 'move'
		}
	}

	function handleDragEnter(e: DragEvent) {
		// Track drops on playlists
		if (!playlist.is_folder && hasTrackData(e.dataTransfer)) {
			e.preventDefault()
			e.stopPropagation()
			dragEnterCounter++
			isDragOver = true
			return
		}

		// Playlist drops on folders
		if (playlist.is_folder && hasPlaylistData(e.dataTransfer)) {
			e.preventDefault()
			e.stopPropagation()
			playlistDragEnterCounter++
			isPlaylistDragOver = true
		}
	}

	function handleDragLeave(e: DragEvent) {
		// Track drops on playlists
		if (!playlist.is_folder && hasTrackData(e.dataTransfer)) {
			dragEnterCounter--
			if (dragEnterCounter <= 0) {
				dragEnterCounter = 0
				isDragOver = false
			}
			return
		}

		// Playlist drops on folders
		if (playlist.is_folder && hasPlaylistData(e.dataTransfer)) {
			playlistDragEnterCounter--
			if (playlistDragEnterCounter <= 0) {
				playlistDragEnterCounter = 0
				isPlaylistDragOver = false
			}
		}
	}

	function handleDrop(e: DragEvent) {
		e.preventDefault()
		e.stopPropagation()

		// Reset all drag state
		dragEnterCounter = 0
		isDragOver = false
		playlistDragEnterCounter = 0
		isPlaylistDragOver = false

		// Handle track drops on playlists
		if (!playlist.is_folder) {
			const trackData = e.dataTransfer?.getData('application/x-crate-tracks')
			if (trackData) {
				try {
					const trackIds = JSON.parse(trackData) as string[]
					onTracksDrop?.(trackIds)
				} catch {
					// Invalid data
				}
			}
			return
		}

		// Handle playlist/folder drops on folders
		const playlistData = e.dataTransfer?.getData('application/x-crate-playlist')
		if (playlistData) {
			try {
				const { id, is_folder } = JSON.parse(playlistData) as { id: string; is_folder: boolean }

				// Prevent dropping on self
				if (id === playlist.id) {
					toastStore.error('Cannot drop a folder into itself')
					return
				}

				// Prevent dropping folder into its own descendants
				if (is_folder && isDescendantOf(id, playlist.id)) {
					toastStore.error('Cannot drop a folder into its own subfolder')
					return
				}

				onPlaylistDrop?.(id)
			} catch {
				// Invalid data
			}
		}
	}
</script>

<div
	role="treeitem"
	tabindex="0"
	draggable="true"
	aria-selected={selected}
	aria-expanded={playlist.is_folder ? expanded : undefined}
	class="flex cursor-pointer items-center gap-2 rounded py-1.5 pr-2 transition-colors {selected
		? 'bg-brand-muted text-text-primary'
		: isDragOver || isPlaylistDragOver
			? 'bg-brand-muted text-text-primary ring-1 ring-brand-primary'
			: 'text-text-secondary hover:bg-surface-2 hover:text-text-primary'}"
	style="padding-left: {paddingLeft}"
	{onclick}
	{oncontextmenu}
	ondragstart={handleDragStart}
	ondragover={handleDragOver}
	ondragenter={handleDragEnter}
	ondragleave={handleDragLeave}
	ondrop={handleDrop}
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
			<svg class="h-3 w-3 transition-transform {expanded ? 'rotate-90' : ''}" fill="currentColor" viewBox="0 0 24 24">
				<path d="M8 5v14l11-7z" />
			</svg>
		</button>
	{:else}
		<span class="w-4"></span>
	{/if}

	<!-- Icon -->
	<span class="flex-shrink-0">
		{#if playlist.is_folder}
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
				/>
			</svg>
		{:else if playlist.is_smart}
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
			</svg>
		{:else}
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					stroke-width="2"
					d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
				/>
			</svg>
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
