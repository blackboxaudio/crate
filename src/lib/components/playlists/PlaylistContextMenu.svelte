<script lang="ts">
	import type { Playlist, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		x: number
		y: number
		playlists: Playlist[]
		folders: Playlist[]
		onClose: () => void
		onClosed?: () => void
		onCreatePlaylist?: (playlist: Playlist) => void
		onCreateSmartPlaylist?: (playlist: Playlist) => void
		onCreateFolder?: (playlist: Playlist) => void
		onEditSmartPlaylist?: (playlist: Playlist) => void
		onRename: (playlist: Playlist) => void
		onDelete: (playlist: Playlist) => void
		onBulkDelete?: (playlists: Playlist[]) => void
		onBulkMove?: (playlists: Playlist[], folderId: string | null) => void
		onMove: (playlist: Playlist, folderId: string | null) => void
		onExport: (playlist: Playlist) => void
	}

	let {
		open,
		x,
		y,
		playlists: targetPlaylists,
		folders,
		onClose,
		onClosed,
		onCreatePlaylist,
		onCreateSmartPlaylist,
		onCreateFolder,
		onEditSmartPlaylist,
		onRename,
		onDelete,
		onBulkDelete,
		onBulkMove,
		onMove,
		onExport,
	}: Props = $props()

	const isBulk = $derived(targetPlaylists.length > 1)
	const playlist = $derived(targetPlaylists.length === 1 ? targetPlaylists[0] : null)

	const menuItems = $derived.by<ContextMenuItem[]>(() => {
		// Bulk mode: show move to folder + delete
		if (isBulk) {
			const bulkItems: ContextMenuItem[] = []

			// Build "Move to Folder" submenu (exclude folders that are in the selection)
			const selectedIdSet = new Set(targetPlaylists.map((p) => p.id))
			const moveSubmenu: ContextMenuItem[] = []

			// Option to move to root
			if (targetPlaylists.some((p) => p.parent_id !== null)) {
				moveSubmenu.push({
					id: 'bulk-move-root',
					label: get(translate)('playlists.rootNoFolder'),
					action: () => onBulkMove?.(targetPlaylists, null),
				})
			}

			for (const folder of folders) {
				if (!selectedIdSet.has(folder.id)) {
					moveSubmenu.push({
						id: `bulk-move-${folder.id}`,
						label: folder.name,
						action: () => onBulkMove?.(targetPlaylists, folder.id),
					})
				}
			}

			if (moveSubmenu.length > 0) {
				bulkItems.push({
					id: 'bulk-move',
					label: get(translate)('playlists.moveToFolder'),
					icon: 'folder-arrow',
					submenu: moveSubmenu,
				})
				bulkItems.push({ id: 'bulk-divider', label: '', divider: true })
			}

			bulkItems.push({
				id: 'bulk-delete',
				label: get(translate)('common.delete'),
				icon: 'trash',
				variant: 'danger',
				action: () => onBulkDelete?.(targetPlaylists),
			})
			return bulkItems
		}

		if (!playlist) return []

		const items: ContextMenuItem[] = []

		// New Folder / New Playlist / New Smart Playlist (only for folders)
		if (playlist.is_folder) {
			if (onCreateFolder) {
				items.push({
					id: 'new-folder',
					label: get(translate)('playlists.newFolder'),
					icon: 'folder',
					action: () => onCreateFolder(playlist),
				})
			}
			if (onCreatePlaylist) {
				items.push({
					id: 'new-playlist',
					label: get(translate)('playlists.newPlaylist'),
					icon: 'music-note',
					action: () => onCreatePlaylist(playlist),
				})
			}
			if (onCreateSmartPlaylist) {
				items.push({
					id: 'new-smart-playlist',
					label: get(translate)('playlists.newSmartPlaylist'),
					icon: 'bolt',
					action: () => onCreateSmartPlaylist(playlist),
				})
			}
			if (onCreatePlaylist || onCreateFolder || onCreateSmartPlaylist) {
				items.push({ id: 'divider-create', label: '', divider: true })
			}
		}

		// Edit Smart Playlist (for smart playlists, before Rename)
		if (playlist.is_smart && onEditSmartPlaylist) {
			items.push({
				id: 'edit-smart-playlist',
				label: get(translate)('playlists.editSmartPlaylist'),
				icon: 'bolt',
				action: () => onEditSmartPlaylist(playlist),
			})
		}

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

			// Export to device (only for non-folders, library context only)
			if (playlist.context === 'library') {
				items.push({
					id: 'export',
					label: get(translate)('playlists.exportToDevice'),
					icon: 'arrow-up-from-bracket',
					action: () => onExport(playlist),
				})
			}
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

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
