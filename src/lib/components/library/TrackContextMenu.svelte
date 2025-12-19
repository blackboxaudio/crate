<script lang="ts">
	import type { Track, Playlist, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'

	type Props = {
		open: boolean
		x: number
		y: number
		selectedTracks: Track[]
		playlists: Playlist[]
		onClose: () => void
		onAddToPlaylist: (playlistId: string) => void
	}

	let { open, x, y, selectedTracks, playlists, onClose, onAddToPlaylist }: Props = $props()

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

		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems()} {onClose} />
