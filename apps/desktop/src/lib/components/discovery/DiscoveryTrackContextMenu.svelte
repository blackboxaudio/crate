<script lang="ts">
	import type { DiscoveryRelease, DiscoveryTrack, ContextMenuItem, Playlist } from '$shared/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { toastStore } from '$shared/stores/toast'
	import { buildTagMenuItems, commonTagIds, tagsStore } from '$shared/stores/tags'
	import { buildPlaylistMenuItems } from '$shared/stores/playlists'
	import { buildYouTubeSearchUrl, joinMenuGroups } from '$shared/utils'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'

	type Props = {
		open: boolean
		x: number
		y: number
		release: DiscoveryRelease
		track: DiscoveryTrack
		/** The tracks the menu acts on: the clicked one, or the whole selection it belongs to. */
		tracks?: DiscoveryTrack[]
		canPlay: boolean
		playlists?: Playlist[]
		currentPlaylistId?: string | null
		onClose: () => void
		onClosed?: () => void
		onLikeToggle: () => void
		onPlayPreview: () => void
		onAddToPlaylist?: (playlistId: string) => void
		onRemoveFromPlaylist?: () => void
		onToggleTag?: (tagId: string, assigned: boolean) => void
	}

	let {
		open,
		x,
		y,
		release,
		track,
		tracks = [track],
		canPlay,
		playlists = [],
		currentPlaylistId = null,
		onClose,
		onClosed,
		onLikeToggle,
		onPlayPreview,
		onAddToPlaylist,
		onRemoveFromPlaylist,
		onToggleTag,
	}: Props = $props()

	const currentPlaylist = $derived(currentPlaylistId ? playlists.find((p) => p.id === currentPlaylistId) : null)
	const single = $derived(tracks.length === 1)
	const assignedTagIds = $derived(commonTagIds(tracks))
	// Prefer the track's own page when the source provides one (Bandcamp/SoundCloud).
	const trackUrl = $derived(track.url ?? release.url)

	const openInLabel = $derived.by(() => {
		const platform = getReleasePlatformName(release.source_type)
		return platform
			? get(translate)('discovery.openInApp', { values: { app: platform } })
			: get(translate)('discovery.openInBrowser')
	})

	// Groups follow the shared convention (.claude/docs/CONTEXT_MENUS.md):
	// act → organize → navigate & share → destructive, a divider between non-empty groups.
	// Act and navigate are single-track only; a multi-track menu starts at the organizing items.
	const menuItems = $derived.by<ContextMenuItem[]>(() => {
		const act: ContextMenuItem[] = single
			? [
					{
						id: 'play-preview',
						label: get(translate)('discovery.playPreview'),
						icon: 'play',
						iconFill: true,
						disabled: !canPlay,
						action: onPlayPreview,
					},
					{
						id: 'like-toggle',
						label: track.is_liked ? get(translate)('discovery.unlike') : get(translate)('discovery.like'),
						icon: 'heart',
						iconFill: track.is_liked,
						action: onLikeToggle,
					},
				]
			: []

		const organize: ContextMenuItem[] = []
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

		const navigate: ContextMenuItem[] = single
			? [
					{
						id: 'open-in-browser',
						label: openInLabel,
						icon: 'external-link',
						action: () => {
							openUrl(trackUrl)
						},
					},
					{
						id: 'search-youtube',
						label: get(translate)('discovery.searchOnYouTube'),
						icon: 'search',
						action: () => {
							openUrl(buildYouTubeSearchUrl(release.artist, track.name))
						},
					},
					{
						id: 'copy-url',
						label: get(translate)('discovery.copyUrl'),
						icon: 'copy',
						action: () => {
							writeText(trackUrl).then(() => {
								toastStore.info(get(translate)('discovery.copiedUrl'))
							})
						},
					},
				]
			: []

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

		return joinMenuGroups([act, organize, navigate, destructive])
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
