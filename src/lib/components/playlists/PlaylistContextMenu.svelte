<script lang="ts">
	import type { Playlist, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		x: number
		y: number
		playlist: Playlist | null
		folders: Playlist[]
		onClose: () => void
		onClosed?: () => void
		onRename: (playlist: Playlist) => void
		onDelete: (playlist: Playlist) => void
		onMove: (playlist: Playlist, folderId: string | null) => void
		onExport: (playlist: Playlist) => void
	}

	let { open, x, y, playlist, folders, onClose, onClosed, onRename, onDelete, onMove, onExport }: Props = $props()

	const menuItems = $derived<ContextMenuItem[]>(() => {
		if (!playlist) return []

		const items: ContextMenuItem[] = []

		// Rename
		items.push({
			id: 'rename',
			label: get(translate)('common.rename'),
			icon: 'pencil',
			action: () => onRename(playlist),
		})

		// Move to Folder (only for non-folders)
		if (!playlist.is_folder) {
			const moveSubmenu: ContextMenuItem[] = []

			// Option to move to root (no parent)
			if (playlist.parent_id !== null) {
				moveSubmenu.push({
					id: 'move-root',
					label: get(translate)('playlists.rootNoFolder'),
					action: () => onMove(playlist, null),
				})
			}

			// Add folders as options (exclude current parent)
			for (const folder of folders) {
				if (folder.id !== playlist.parent_id && folder.id !== playlist.id) {
					moveSubmenu.push({
						id: `move-${folder.id}`,
						label: folder.name,
						action: () => onMove(playlist, folder.id),
					})
				}
			}

			if (moveSubmenu.length > 0) {
				items.push({
					id: 'move',
					label: get(translate)('playlists.moveToFolder'),
					icon: 'folder-arrow',
					submenu: moveSubmenu,
				})
			}

			// Export to device (only for non-folders)
			items.push({
				id: 'export',
				label: get(translate)('playlists.exportToDevice'),
				icon: 'arrow-up-from-bracket',
				action: () => onExport(playlist),
			})
		}

		items.push({ id: 'divider-1', label: '', divider: true })

		// Delete
		items.push({
			id: 'delete',
			label: playlist.is_folder ? get(translate)('playlists.deleteFolder') : get(translate)('playlists.deletePlaylist'),
			icon: 'trash',
			variant: 'danger',
			action: () => onDelete(playlist),
		})

		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems()} {onClose} {onClosed} />
