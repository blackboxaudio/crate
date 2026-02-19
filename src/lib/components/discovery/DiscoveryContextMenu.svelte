<script lang="ts">
	import type { DiscoveryRelease, Playlist, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$lib/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		x: number
		y: number
		selectedReleases: DiscoveryRelease[]
		playlists?: Playlist[]
		onClose: () => void
		onClosed?: () => void
		onOpenInBrowser: () => void
		onRefreshMetadata?: () => void
		onImport: () => void
		onMerge?: () => void
		onDelete: () => void
		onAddToPlaylist?: (playlistId: string) => void
	}

	let {
		open,
		x,
		y,
		selectedReleases,
		playlists = [],
		onClose,
		onClosed,
		onOpenInBrowser,
		onRefreshMetadata,
		onImport,
		onMerge,
		onDelete,
		onAddToPlaylist,
	}: Props = $props()

	// Filter to only non-folder discovery playlists
	const availablePlaylists = $derived(playlists.filter((p) => !p.is_folder))

	const menuItems = $derived<ContextMenuItem[]>(() => {
		const items: ContextMenuItem[] = []

		// Open in Browser - single release only
		if (selectedReleases.length === 1) {
			items.push({
				id: 'open-in-browser',
				label: get(translate)('discovery.openInBrowser'),
				icon: 'globe',
				action: onOpenInBrowser,
			})
			if (onRefreshMetadata) {
				items.push({
					id: 'refresh-metadata',
					label: get(translate)('discovery.refreshMetadata'),
					icon: 'refresh',
					action: onRefreshMetadata,
				})
			}
			items.push({ id: 'browser-divider', label: '', divider: true })
		}

		// Import to Library
		if (selectedReleases.length === 1) {
			items.push({
				id: 'import-to-library',
				label: get(translate)('discovery.importToLibrary'),
				icon: 'plus',
				action: onImport,
			})
		}

		// Add to Playlist submenu
		if (onAddToPlaylist && availablePlaylists.length > 0) {
			items.push({
				id: 'add-to-playlist',
				label: get(translate)('contextMenu.addToPlaylist'),
				icon: 'playlist',
				submenu: availablePlaylists.map((p) => ({
					id: `playlist-${p.id}`,
					label: p.name,
					action: () => onAddToPlaylist!(p.id),
				})),
			})
		}

		// Merge Releases - when 2+ releases selected
		if (selectedReleases.length >= 2 && onMerge) {
			items.push({
				id: 'merge-releases',
				label: get(translate)('discovery.mergeReleases'),
				icon: 'copy',
				action: onMerge,
			})
		}

		items.push({ id: 'actions-divider', label: '', divider: true })

		// Delete
		items.push({
			id: 'delete',
			label:
				selectedReleases.length === 1
					? get(translate)('discovery.deleteRelease')
					: get(translate)('discovery.deleteReleases'),
			icon: 'trash',
			variant: 'danger',
			action: onDelete,
		})

		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems()} {onClose} {onClosed} />
