<script lang="ts">
	import type { Track, TrackColor, Playlist, ContextMenuItem } from '$shared/types'
	import { TRACK_COLORS } from '$shared/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { missingTrackIds } from '$lib/stores'
	import { buildPlaylistMenuItems } from '$shared/stores/playlists'
	import { buildTagMenuItems, commonTagIds, tagsStore } from '$shared/stores/tags'
	import { joinMenuGroups } from '$shared/utils'
	import { translate } from '$shared/i18n'
	import { get } from 'svelte/store'

	type Props = {
		open: boolean
		x: number
		y: number
		selectedTracks: Track[]
		playlists: Playlist[]
		currentPlaylistId: string | null
		isAnalyzing?: boolean
		onClose: () => void
		onClosed?: () => void
		onRevealInExplorer: () => void
		onAddToPlaylist: (playlistId: string) => void
		onRemoveFromPlaylist: () => void
		onRemoveFromLibrary: () => void
		onRelocate?: (track: Track) => void
		onSetColor?: (color: TrackColor | null) => void
		onAnalyze?: () => void
		onToggleTag?: (tagId: string, assigned: boolean) => void
	}

	let {
		open,
		x,
		y,
		selectedTracks,
		playlists,
		currentPlaylistId,
		isAnalyzing = false,
		onClose,
		onClosed,
		onRevealInExplorer,
		onAddToPlaylist,
		onRemoveFromPlaylist,
		onRemoveFromLibrary,
		onRelocate,
		onSetColor,
		onAnalyze,
		onToggleTag,
	}: Props = $props()

	// Platform-specific label for "View in Finder/Explorer"
	const revealLabel = $derived.by(() => {
		const ua = navigator.userAgent
		if (ua.includes('Mac')) return get(translate)('contextMenu.viewInFinder')
		if (ua.includes('Windows')) return get(translate)('contextMenu.viewInExplorer')
		return get(translate)('contextMenu.viewInFileManager')
	})

	const single = $derived(selectedTracks.length === 1)
	const hasMissingTrack = $derived(single && $missingTrackIds.has(selectedTracks[0].id))

	// A color only reads as current when the whole selection shares it.
	const currentColor = $derived.by(() => {
		if (selectedTracks.length === 0) return null
		const firstColor = selectedTracks[0].color
		return selectedTracks.every((t) => t.color === firstColor) ? firstColor : null
	})

	const assignedTagIds = $derived(commonTagIds(selectedTracks))
	const currentPlaylist = $derived(currentPlaylistId ? playlists.find((p) => p.id === currentPlaylistId) : null)

	// Groups follow the shared convention (.claude/docs/CONTEXT_MENUS.md):
	// act → organize → navigate & share → destructive, a divider between non-empty groups.
	const menuItems = $derived.by<ContextMenuItem[]>(() => {
		const groups: ContextMenuItem[][] = []

		const act: ContextMenuItem[] = []
		if (onAnalyze) {
			act.push({
				id: 'analyze',
				label: get(translate)('contextMenu.analyze'),
				icon: 'activity',
				disabled: isAnalyzing,
				action: onAnalyze,
			})
		}
		if (hasMissingTrack && onRelocate) {
			act.push({
				id: 'relocate',
				label: get(translate)('contextMenu.relocate'),
				icon: 'folder',
				disabled: isAnalyzing,
				action: () => onRelocate(selectedTracks[0]),
			})
		}
		groups.push(act)

		const organize: ContextMenuItem[] = []
		const playlistItems = buildPlaylistMenuItems(
			playlists.filter((p) => p.context === 'library'),
			(playlistId) => () => onAddToPlaylist(playlistId)
		)
		organize.push({
			id: 'add-to-playlist',
			label: get(translate)('contextMenu.addToPlaylist'),
			icon: 'list-plus',
			...(playlistItems.length > 0 ? { submenu: playlistItems } : { disabled: true }),
		})
		if (onToggleTag) {
			const tagItems = buildTagMenuItems($tagsStore.categories, assignedTagIds, onToggleTag)
			organize.push({
				id: 'tags',
				label: get(translate)('nav.tags'),
				icon: 'tag',
				...(tagItems.length > 0 ? { submenu: tagItems } : { disabled: true }),
			})
		}
		if (onSetColor) {
			const colorItems: ContextMenuItem[] = TRACK_COLORS.map((color) => ({
				id: `color-${color.id}`,
				label: get(translate)(`colors.${color.id}`),
				colorDot: color.hex,
				selected: currentColor === color.id,
				action: () => onSetColor(color.id),
			}))
			colorItems.push({ id: 'color-divider', label: '', divider: true })
			colorItems.push({
				id: 'remove-color',
				label: get(translate)('contextMenu.removeColor'),
				icon: 'minus-circle',
				variant: 'danger',
				action: () => onSetColor(null),
			})
			organize.push({
				id: 'set-color',
				label: get(translate)('contextMenu.setColor'),
				icon: 'palette',
				submenu: colorItems,
			})
		}
		groups.push(organize)

		const navigate: ContextMenuItem[] = []
		if (single) {
			navigate.push({
				id: 'reveal-in-explorer',
				label: revealLabel,
				icon: 'folder-open',
				action: onRevealInExplorer,
			})
		}
		groups.push(navigate)

		const destructive: ContextMenuItem[] = []
		if (currentPlaylistId && !currentPlaylist?.is_smart) {
			destructive.push({
				id: 'remove-from-playlist',
				label: get(translate)('contextMenu.removeFromPlaylist'),
				icon: 'list-minus',
				variant: 'danger',
				disabled: isAnalyzing,
				action: onRemoveFromPlaylist,
			})
		}
		destructive.push({
			id: 'remove-from-library',
			label: get(translate)('contextMenu.removeFromLibrary'),
			icon: 'trash',
			variant: 'danger',
			disabled: isAnalyzing,
			action: onRemoveFromLibrary,
		})
		groups.push(destructive)

		return joinMenuGroups(groups)
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems} {onClose} {onClosed} />
