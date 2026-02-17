<script lang="ts">
	import type { DiscoveryRelease, DiscoveryStatus, Playlist, ContextMenuItem } from '$lib/types'
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
		onSetStatus: (status: DiscoveryStatus) => void
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
		onSetStatus,
		onDelete,
		onAddToPlaylist,
	}: Props = $props()

	const currentStatus = $derived(() => {
		if (selectedReleases.length === 0) return null
		const first = selectedReleases[0].status
		return selectedReleases.every((r) => r.status === first) ? first : null
	})

	const statusOptions: DiscoveryStatus[] = ['unlistened', 'listened', 'purchased', 'dismissed']

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
			items.push({ id: 'browser-divider', label: '', divider: true })
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
			items.push({ id: 'playlist-divider', label: '', divider: true })
		}

		// Set Status submenu
		items.push({
			id: 'set-status',
			label: get(translate)('discovery.setStatus'),
			icon: 'tag',
			submenu: statusOptions.map((status) => ({
				id: `status-${status}`,
				label: get(translate)(`discovery.status.${status}`),
				selected: currentStatus() === status,
				action: () => onSetStatus(status),
			})),
		})

		items.push({ id: 'status-divider', label: '', divider: true })

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
