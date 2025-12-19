<script lang="ts">
	import type { Track, TrackColor, Playlist, ContextMenuItem } from '$lib/types'
	import { TRACK_COLORS } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { missingTrackIds } from '$lib/stores'

	type Props = {
		open: boolean
		x: number
		y: number
		selectedTracks: Track[]
		playlists: Playlist[]
		currentPlaylistId: string | null
		onClose: () => void
		onRevealInExplorer: () => void
		onAddToPlaylist: (playlistId: string) => void
		onRemoveFromPlaylist: () => void
		onRemoveFromLibrary: () => void
		onRelocate?: (track: Track) => void
		onSetColor?: (color: TrackColor | null) => void
	}

	let {
		open,
		x,
		y,
		selectedTracks,
		playlists,
		currentPlaylistId,
		onClose,
		onRevealInExplorer,
		onAddToPlaylist,
		onRemoveFromPlaylist,
		onRemoveFromLibrary,
		onRelocate,
		onSetColor,
	}: Props = $props()

	// Platform-specific label for "View in Finder/Explorer"
	const revealLabel = $derived(() => {
		const ua = navigator.userAgent
		if (ua.includes('Mac')) return 'View in Finder'
		if (ua.includes('Windows')) return 'View in Explorer'
		return 'View in File Manager'
	})

	// Check if any selected track is missing
	const hasMissingTrack = $derived(() => {
		return selectedTracks.length === 1 && $missingTrackIds.has(selectedTracks[0].id)
	})

	// Build menu items
	const menuItems = $derived<ContextMenuItem[]>(() => {
		const items: ContextMenuItem[] = []

		// "Relocate..." - only for single missing track
		if (hasMissingTrack() && onRelocate) {
			items.push({
				id: 'relocate',
				label: 'Relocate...',
				icon: 'folder',
				action: () => onRelocate(selectedTracks[0]),
			})
			items.push({
				id: 'relocate-divider',
				label: '',
				divider: true,
			})
		}

		// "View in Finder/Explorer" - only for single track selection
		if (selectedTracks.length === 1) {
			items.push({
				id: 'reveal-in-explorer',
				label: revealLabel(),
				icon: 'folder-open',
				action: onRevealInExplorer,
			})
			items.push({
				id: 'reveal-divider',
				label: '',
				divider: true,
			})
		}

		// Add to Playlist submenu
		const playlistItems = playlists.filter((p) => !p.is_folder)
		if (playlistItems.length > 0) {
			items.push({
				id: 'add-to-playlist',
				label: 'Add to Playlist',
				icon: 'list-plus',
				submenu: playlistItems.map((playlist) => ({
					id: `playlist-${playlist.id}`,
					label: playlist.name,
					action: () => onAddToPlaylist(playlist.id),
				})),
			})
		} else {
			items.push({
				id: 'add-to-playlist',
				label: 'Add to Playlist',
				icon: 'list-plus',
				disabled: true,
			})
		}

		// Set Color submenu
		if (onSetColor) {
			const colorItems: ContextMenuItem[] = TRACK_COLORS.map((color) => ({
				id: `color-${color.id}`,
				label: color.label,
				action: () => onSetColor(color.id),
			}))
			colorItems.push({
				id: 'color-divider',
				label: '',
				divider: true,
			})
			colorItems.push({
				id: 'remove-color',
				label: 'Remove Color',
				action: () => onSetColor(null),
			})
			items.push({
				id: 'set-color',
				label: 'Set Color',
				icon: 'palette',
				submenu: colorItems,
			})
		}

		// Build Remove submenu items
		const removeItems: ContextMenuItem[] = []

		// "Remove from Playlist" - only when viewing a playlist
		if (currentPlaylistId) {
			removeItems.push({
				id: 'remove-from-playlist',
				label: 'Remove from Playlist',
				icon: 'list-minus',
				action: onRemoveFromPlaylist,
			})
		}

		// "Remove from Library" - always visible
		removeItems.push({
			id: 'remove-from-library',
			label: 'Remove from Library',
			icon: 'trash',
			action: onRemoveFromLibrary,
		})

		// Add Remove submenu
		items.push({
			id: 'remove',
			label: 'Remove',
			icon: 'minus-circle',
			submenu: removeItems,
		})

		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems()} {onClose} />
