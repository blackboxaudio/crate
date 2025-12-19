<script lang="ts">
	import type { Track, Playlist, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'

	type Props = {
		open: boolean
		x: number
		y: number
		selectedTracks: Track[]
		playlists: Playlist[]
		currentPlaylistId: string | null
		onClose: () => void
		onAddToPlaylist: (playlistId: string) => void
		onRemoveFromPlaylist: () => void
		onRemoveFromLibrary: () => void
	}

	let {
		open,
		x,
		y,
		selectedTracks,
		playlists,
		currentPlaylistId,
		onClose,
		onAddToPlaylist,
		onRemoveFromPlaylist,
		onRemoveFromLibrary,
	}: Props = $props()

	// Build menu items
	const menuItems = $derived<ContextMenuItem[]>(() => {
		const items: ContextMenuItem[] = []

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
