<script lang="ts">
	import type { DiscoveryRelease, DiscoveryTrack, ContextMenuItem } from '$shared/types'
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
		release: DiscoveryRelease
		track: DiscoveryTrack
		canPlay: boolean
		onClose: () => void
		onClosed?: () => void
		onLikeToggle: () => void
		onPlayPreview: () => void
	}

	let { open, x, y, release, track, canPlay, onClose, onClosed, onLikeToggle, onPlayPreview }: Props = $props()

	const menuItems = $derived.by<ContextMenuItem[]>(() => [
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
				openUrl(release.url)
			},
		},
		{
			id: 'copy-release-url',
			label: get(translate)('discovery.copyReleaseUrl'),
			icon: 'copy',
			action: () => {
				writeText(release.url).then(() => {
					toastStore.info(get(translate)('discovery.copiedUrl'))
				})
			},
		},
	])
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
