<script lang="ts">
	import type { Playlist, ContextMenuItem } from '$shared/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'
	import { buildFolderMenuItems } from '$shared/stores/playlists'
	import { joinMenuGroups } from '$shared/utils'
	import { SvelteSet } from 'svelte/reactivity'

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

	// Groups follow the shared convention (.claude/docs/CONTEXT_MENUS.md):
	// create → manage → destructive, a divider between non-empty groups.
	const menuItems = $derived.by<ContextMenuItem[]>(() => {
		if (isBulk) {
			const manage: ContextMenuItem[] = []
			// Folders in the selection can't receive it.
			const selectedIdSet = new Set(targetPlaylists.map((p) => p.id))
			const moveSubmenu = buildFolderMenuItems(
				folders,
				selectedIdSet,
				(folderId) => () => onBulkMove?.(targetPlaylists, folderId),
				'bulk-move'
			)
			if (targetPlaylists.some((p) => p.parent_id !== null)) {
				moveSubmenu.unshift({
					id: 'bulk-move-root',
					label: get(translate)('playlists.rootNoFolder'),
					action: () => onBulkMove?.(targetPlaylists, null),
				})
			}
			if (moveSubmenu.length > 0) {
				manage.push({
					id: 'bulk-move',
					label: get(translate)('playlists.moveToFolder'),
					icon: 'folder-arrow',
					submenu: moveSubmenu,
				})
			}

			const destructive: ContextMenuItem[] = [
				{
					id: 'bulk-delete',
					label: get(translate)('common.delete'),
					icon: 'trash',
					variant: 'danger',
					action: () => onBulkDelete?.(targetPlaylists),
				},
			]
			return joinMenuGroups([manage, destructive])
		}

		if (!playlist) return []

		const create: ContextMenuItem[] = []
		if (playlist.is_folder) {
			if (onCreateFolder) {
				create.push({
					id: 'new-folder',
					label: get(translate)('playlists.newFolder'),
					icon: 'folder',
					action: () => onCreateFolder(playlist),
				})
			}
			if (onCreatePlaylist) {
				create.push({
					id: 'new-playlist',
					label: get(translate)('playlists.newPlaylist'),
					icon: 'music-note',
					action: () => onCreatePlaylist(playlist),
				})
			}
			if (onCreateSmartPlaylist) {
				create.push({
					id: 'new-smart-playlist',
					label: get(translate)('playlists.newSmartPlaylist'),
					icon: 'bolt',
					action: () => onCreateSmartPlaylist(playlist),
				})
			}
		}

		const manage: ContextMenuItem[] = [
			{
				id: 'rename',
				label: get(translate)('common.rename'),
				icon: 'pencil',
				action: () => onRename(playlist),
			},
		]
		if (playlist.is_smart && onEditSmartPlaylist) {
			manage.push({
				id: 'edit-smart-playlist',
				label: get(translate)('playlists.editSmartPlaylist'),
				icon: 'bolt',
				action: () => onEditSmartPlaylist(playlist),
			})
		}
		if (!playlist.is_folder) {
			// Neither the item itself nor its current parent is a move target.
			const excludeIds = new SvelteSet<string>([playlist.id])
			if (playlist.parent_id) excludeIds.add(playlist.parent_id)
			const moveSubmenu = buildFolderMenuItems(folders, excludeIds, (folderId) => () => onMove(playlist, folderId))
			if (playlist.parent_id !== null) {
				moveSubmenu.unshift({
					id: 'move-root',
					label: get(translate)('playlists.rootNoFolder'),
					action: () => onMove(playlist, null),
				})
			}
			if (moveSubmenu.length > 0) {
				manage.push({
					id: 'move',
					label: get(translate)('playlists.moveToFolder'),
					icon: 'folder-arrow',
					submenu: moveSubmenu,
				})
			}
			// Only library playlists export to a device.
			if (playlist.context === 'library') {
				manage.push({
					id: 'export',
					label: get(translate)('playlists.exportToDevice'),
					icon: 'arrow-up-from-bracket',
					action: () => onExport(playlist),
				})
			}
		}

		const destructive: ContextMenuItem[] = [
			{
				id: 'delete',
				label: playlist.is_folder
					? get(translate)('playlists.deleteFolder')
					: get(translate)('playlists.deletePlaylist'),
				icon: 'trash',
				variant: 'danger',
				action: () => onDelete(playlist),
			},
		]

		return joinMenuGroups([create, manage, destructive])
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
