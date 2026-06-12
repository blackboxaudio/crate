<script lang="ts">
	import type { DiscoveryRelease, Playlist, ContextMenuItem } from '$shared/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { toastStore } from '$shared/stores/toast'
	import { buildYouTubeSearchUrl } from '$shared/utils'

	type Props = {
		open: boolean
		x: number
		y: number
		selectedReleases: DiscoveryRelease[]
		playlists?: Playlist[]
		currentPlaylistId?: string | null
		onClose: () => void
		onClosed?: () => void
		onOpenInBrowser: () => void
		onRefreshMetadata?: () => void
		onImport: () => void
		onMerge?: () => void
		onDelete: () => void
		onAddToPlaylist?: (playlistId: string) => void
		onRemoveFromPlaylist?: () => void
	}

	let {
		open,
		x,
		y,
		selectedReleases,
		playlists = [],
		currentPlaylistId,
		onClose,
		onClosed,
		onOpenInBrowser,
		onRefreshMetadata,
		onImport,
		onMerge,
		onDelete,
		onAddToPlaylist,
		onRemoveFromPlaylist,
	}: Props = $props()

	// Filter to only non-folder discovery playlists
	const availablePlaylists = $derived(playlists.filter((p) => !p.is_folder && !p.is_smart))

	const menuItems = $derived.by<ContextMenuItem[]>(() => {
		const items: ContextMenuItem[] = []

		// Open in Browser / Copy URL - single release only
		if (selectedReleases.length === 1) {
			items.push({
				id: 'open-in-browser',
				label: get(translate)('discovery.openInBrowser'),
				icon: 'external-link',
				action: onOpenInBrowser,
			})
			items.push({
				id: 'copy-url',
				label: get(translate)('discovery.copyUrl'),
				icon: 'copy',
				action: () => {
					writeText(selectedReleases[0].url).then(() => {
						toastStore.info(get(translate)('discovery.copiedUrl'))
					})
				},
			})
			items.push({
				id: 'search-youtube',
				label: get(translate)('discovery.searchOnYouTube'),
				icon: 'search',
				action: () => {
					openUrl(buildYouTubeSearchUrl(selectedReleases[0].artist, selectedReleases[0].title))
				},
			})
			items.push({ id: 'browser-divider', label: '', divider: true })
		}

		// Refresh metadata - available for single and multi-select
		if (onRefreshMetadata) {
			items.push({
				id: 'refresh-metadata',
				label: get(translate)('discovery.refreshMetadata'),
				icon: 'refresh',
				action: onRefreshMetadata,
			})
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

		const currentPlaylist = currentPlaylistId ? playlists.find((p) => p.id === currentPlaylistId) : null
		if (currentPlaylistId && !currentPlaylist?.is_smart && onRemoveFromPlaylist) {
			items.push({
				id: 'remove-from-playlist',
				label: get(translate)('contextMenu.removeFromPlaylist'),
				icon: 'list-minus',
				variant: 'danger',
				action: onRemoveFromPlaylist,
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

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
