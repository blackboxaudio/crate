<script lang="ts">
	import type { Track, Playlist, TagCategory, ContextMenuItem } from '$lib/types'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import { SvelteMap } from 'svelte/reactivity'

	type Props = {
		open: boolean
		x: number
		y: number
		selectedTracks: Track[]
		playlists: Playlist[]
		tagCategories: TagCategory[]
		onClose: () => void
		onAddToPlaylist: (playlistId: string) => void
		onAssignTag: (tagId: string) => void
		onRemoveTag: (tagId: string) => void
	}

	let {
		open,
		x,
		y,
		selectedTracks,
		playlists,
		tagCategories,
		onClose,
		onAddToPlaylist,
		onAssignTag,
		onRemoveTag,
	}: Props = $props()

	// Get tags that are on all selected tracks (for removal)
	const tagsOnSelectedTracks = $derived(() => {
		if (selectedTracks.length === 0) return []

		// Get all unique tags across selected tracks
		const tagMap = new SvelteMap<string, { id: string; name: string; count: number }>()

		for (const track of selectedTracks) {
			for (const tag of track.tags || []) {
				const existing = tagMap.get(tag.id)
				if (existing) {
					existing.count++
				} else {
					tagMap.set(tag.id, { id: tag.id, name: tag.name, count: 1 })
				}
			}
		}

		// Return tags that appear on at least one track
		return Array.from(tagMap.values())
	})

	// Build menu items
	const menuItems = $derived<ContextMenuItem[]>(() => {
		const items: ContextMenuItem[] = []

		// Add to Playlist submenu
		const playlistItems = playlists.filter((p) => !p.is_folder)
		if (playlistItems.length > 0) {
			items.push({
				id: 'add-to-playlist',
				label: 'Add to Playlist',
				submenu: playlistItems.map((playlist) => ({
					id: `playlist-${playlist.id}`,
					label: playlist.name,
					action: () => onAddToPlaylist(playlist.id),
				})),
			})
		} else {
			items.push({
				id: 'add-to-playlist',
				label: 'Add to Playlist',
				disabled: true,
			})
		}

		items.push({ id: 'divider-1', label: '', divider: true })

		// Add Tag submenu - group by category
		const tagSubmenu: ContextMenuItem[] = []
		for (const category of tagCategories) {
			if (category.tags.length > 0) {
				tagSubmenu.push({
					id: `category-${category.id}`,
					label: category.name,
					submenu: category.tags.map((tag) => ({
						id: `tag-${tag.id}`,
						label: tag.name,
						action: () => onAssignTag(tag.id),
					})),
				})
			}
		}

		if (tagSubmenu.length > 0) {
			items.push({
				id: 'add-tag',
				label: 'Add Tag',
				submenu: tagSubmenu,
			})
		} else {
			items.push({
				id: 'add-tag',
				label: 'Add Tag',
				disabled: true,
			})
		}

		// Remove Tag submenu - only show tags on selected tracks
		const tagsToRemove = tagsOnSelectedTracks()
		if (tagsToRemove.length > 0) {
			items.push({
				id: 'remove-tag',
				label: 'Remove Tag',
				submenu: tagsToRemove.map((tag) => ({
					id: `remove-tag-${tag.id}`,
					label: tag.name,
					action: () => onRemoveTag(tag.id),
				})),
			})
		}

		return items
	})
</script>

<ContextMenu {open} {x} {y} items={menuItems()} {onClose} />
