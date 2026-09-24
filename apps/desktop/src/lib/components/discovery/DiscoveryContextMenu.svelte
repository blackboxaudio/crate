<script lang="ts">
	import type { DiscoveryRelease, Playlist, ContextMenuItem } from '$shared/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { toastStore } from '$shared/stores/toast'
	import { buildYouTubeSearchUrl, joinMenuGroups } from '$shared/utils'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { buildPlaylistMenuItems } from '$shared/stores/playlists'
	import { buildTagMenuItems, commonTagIds, tagsStore } from '$shared/stores/tags'
	import { firstPlayablePreviewIndex } from '$shared/stores/playbackQueue'
	import { playReleasesNext, addReleasesToQueue } from '$lib/controllers'

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
		onExport: () => void
		onDelete: () => void
		onAddToPlaylist?: (playlistId: string) => void
		onRemoveFromPlaylist?: () => void
		onToggleTag?: (tagId: string, assigned: boolean) => void
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
		onExport,
		onDelete,
		onAddToPlaylist,
		onRemoveFromPlaylist,
		onToggleTag,
	}: Props = $props()

	const single = $derived(selectedReleases.length === 1)
	const release = $derived(single ? selectedReleases[0] : null)
	const assignedTagIds = $derived(commonTagIds(selectedReleases))
	const currentPlaylist = $derived(currentPlaylistId ? playlists.find((p) => p.id === currentPlaylistId) : null)

	const openInLabel = $derived.by(() => {
		const platform = release ? getReleasePlatformName(release.source_type) : null
		return platform
			? get(translate)('discovery.openInApp', { values: { app: platform } })
			: get(translate)('discovery.openInBrowser')
	})

	// Whether any selected release has a track the queue could play (mirrors the mobile release menu).
	const canQueue = $derived(selectedReleases.some((r) => firstPlayablePreviewIndex(r) !== -1))

	// Groups follow the shared convention (.claude/docs/CONTEXT_MENUS.md):
	// act → organize → manage → navigate & share → destructive, a divider between non-empty groups.
	const menuItems = $derived.by<ContextMenuItem[]>(() => {
		// Whole-release queue actions enqueue every track of each selected release, in order.
		const act: ContextMenuItem[] = [
			{
				id: 'play-next',
				label: get(translate)('queue.playNext'),
				icon: 'play-next',
				disabled: !canQueue,
				action: () => playReleasesNext(selectedReleases),
			},
			{
				id: 'add-to-queue',
				label: get(translate)('queue.addToQueue'),
				icon: 'queue-plus',
				disabled: !canQueue,
				action: () => addReleasesToQueue(selectedReleases),
			},
		]

		const organize: ContextMenuItem[] = []
		if (single) {
			organize.push({
				id: 'import-to-library',
				label: get(translate)('discovery.importToLibrary'),
				icon: 'plus',
				action: onImport,
			})
		}
		if (onAddToPlaylist) {
			const playlistItems = buildPlaylistMenuItems(playlists, (playlistId) => () => onAddToPlaylist(playlistId))
			organize.push({
				id: 'add-to-playlist',
				label: get(translate)('contextMenu.addToPlaylist'),
				icon: 'playlist',
				...(playlistItems.length > 0 ? { submenu: playlistItems } : { disabled: true }),
			})
		}
		if (onToggleTag) {
			const tagItems = buildTagMenuItems($tagsStore.categories, assignedTagIds, onToggleTag)
			organize.push({
				id: 'tags',
				label: get(translate)('nav.tags'),
				icon: 'tag',
				...(tagItems.length > 0 ? { submenu: tagItems } : { disabled: true }),
			})
		}

		const manage: ContextMenuItem[] = []
		if (onRefreshMetadata) {
			manage.push({
				id: 'refresh-metadata',
				label: get(translate)('discovery.refreshMetadata'),
				icon: 'refresh',
				action: onRefreshMetadata,
			})
		}
		if (selectedReleases.length >= 2 && onMerge) {
			manage.push({
				id: 'merge-releases',
				label: get(translate)('discovery.mergeReleases'),
				icon: 'copy',
				action: onMerge,
			})
		}
		manage.push({
			id: 'export-json',
			label: get(translate)('discovery.exportAsJson'),
			icon: 'download',
			action: onExport,
		})

		const navigate: ContextMenuItem[] = []
		if (release) {
			navigate.push(
				{ id: 'open-in-browser', label: openInLabel, icon: 'external-link', action: onOpenInBrowser },
				{
					id: 'search-youtube',
					label: get(translate)('discovery.searchOnYouTube'),
					icon: 'search',
					action: () => {
						openUrl(buildYouTubeSearchUrl(release.artist, release.title))
					},
				},
				{
					id: 'copy-url',
					label: get(translate)('discovery.copyUrl'),
					icon: 'copy',
					action: () => {
						writeText(release.url).then(() => {
							toastStore.info(get(translate)('discovery.copiedUrl'))
						})
					},
				}
			)
		}

		const destructive: ContextMenuItem[] = []
		if (currentPlaylistId && !currentPlaylist?.is_smart && onRemoveFromPlaylist) {
			destructive.push({
				id: 'remove-from-playlist',
				label: get(translate)('contextMenu.removeFromPlaylist'),
				icon: 'list-minus',
				variant: 'danger',
				action: onRemoveFromPlaylist,
			})
		}
		destructive.push({
			id: 'delete',
			label: single ? get(translate)('discovery.deleteRelease') : get(translate)('discovery.deleteReleases'),
			icon: 'trash',
			variant: 'danger',
			action: onDelete,
		})

		return joinMenuGroups([act, organize, manage, navigate, destructive])
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
