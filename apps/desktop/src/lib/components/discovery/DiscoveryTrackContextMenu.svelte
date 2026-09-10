<script lang="ts">
	import type { DiscoveryRelease, DiscoveryTrack, ContextMenuItem, Playlist } from '$shared/types'
	import { DEFAULT_TAG_COLOR } from '$shared/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { toastStore } from '$shared/stores/toast'
	import { tagsStore } from '$shared/stores/tags'
	import { buildYouTubeSearchUrl } from '$shared/utils'

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

	// Same targets as the release menu: regular discovery playlists only (smart ones are rule-generated).
	const availablePlaylists = $derived(playlists.filter((p) => !p.is_folder && !p.is_smart))
	const currentPlaylist = $derived(currentPlaylistId ? playlists.find((p) => p.id === currentPlaylistId) : null)
	const trackTagIds = $derived(new Set((track.tags ?? []).map((t) => t.id)))
	const multi = $derived(tracks.length > 1)

	const menuItems = $derived.by<ContextMenuItem[]>(() => {
		// Like and Play are single-track actions; a multi-track menu starts at the organizing items.
		const items: ContextMenuItem[] = multi
			? []
			: [
					{
						id: 'like-toggle',
						label: track.is_liked ? get(translate)('discovery.unlike') : get(translate)('discovery.like'),
						icon: 'heart',
						iconFill: track.is_liked,
						action: onLikeToggle,
					},
					{
						id: 'play-preview',
						label: get(translate)('discovery.playPreview'),
						icon: 'play',
						iconFill: true,
						disabled: !canPlay,
						action: onPlayPreview,
					},
					{ id: 'play-divider', label: '', divider: true },
				]
		const organizeStart = items.length

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

		if (currentPlaylistId && !currentPlaylist?.is_smart && onRemoveFromPlaylist) {
			items.push({
				id: 'remove-from-playlist',
				label: get(translate)('contextMenu.removeFromPlaylist'),
				icon: 'list-minus',
				variant: 'danger',
				action: onRemoveFromPlaylist,
			})
		}

		// Track-level tags: one flat submenu, categories separated by dividers, assigned tags checked.
		const categories = $tagsStore.categories.filter((c) => c.tags.length > 0)
		if (onToggleTag && categories.length > 0) {
			const submenu: ContextMenuItem[] = []
			categories.forEach((category, i) => {
				if (i > 0) submenu.push({ id: `tag-divider-${category.id}`, label: '', divider: true })
				for (const tag of category.tags) {
					const assigned = trackTagIds.has(tag.id)
					submenu.push({
						id: `tag-${tag.id}`,
						label: tag.name,
						colorDot: tag.color ?? category.color ?? DEFAULT_TAG_COLOR,
						selected: assigned,
						action: () => onToggleTag!(tag.id, assigned),
					})
				}
			})
			items.push({ id: 'tags', label: get(translate)('nav.tags'), icon: 'tag', submenu })
		}

		if (items.length > organizeStart) items.push({ id: 'organize-divider', label: '', divider: true })

		// Link actions are single-track too.
		if (multi) return items

		items.push(
			{
				id: 'search-youtube',
				label: get(translate)('discovery.searchOnYouTube'),
				icon: 'search',
				action: () => {
					openUrl(buildYouTubeSearchUrl(release.artist, track.name))
				},
			},
			{
				id: 'open-release-in-browser',
				label: get(translate)('discovery.openReleaseInBrowser'),
				icon: 'external-link',
				action: () => {
					// Prefer the track's own page when the source provides one (Bandcamp/SoundCloud).
					openUrl(track.url ?? release.url)
				},
			},
			{
				id: 'copy-release-url',
				label: get(translate)('discovery.copyReleaseUrl'),
				icon: 'copy',
				action: () => {
					writeText(track.url ?? release.url).then(() => {
						toastStore.info(get(translate)('discovery.copiedUrl'))
					})
				},
			}
		)
		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
