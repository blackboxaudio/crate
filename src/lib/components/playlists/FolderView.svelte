<script lang="ts">
	import type { Playlist, BreadcrumbItem } from '$lib/types'
	import { getPlaylistChildren } from '$lib/stores/playlists'
	import FolderCard from './FolderCard.svelte'
	import Breadcrumbs from '$lib/components/common/Breadcrumbs.svelte'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		folderId: string
		playlists: Playlist[]
		onSelect: (playlist: Playlist) => void
		breadcrumbItems: BreadcrumbItem[]
		onBreadcrumbNavigate: (item: BreadcrumbItem) => void
		onBreadcrumbContextMenu: (e: MouseEvent, item: BreadcrumbItem) => void
		onEmptySpaceContextMenu?: (e: MouseEvent, folderId: string) => void
	}

	let {
		folderId,
		playlists,
		onSelect,
		breadcrumbItems,
		onBreadcrumbNavigate,
		onBreadcrumbContextMenu,
		onEmptySpaceContextMenu,
	}: Props = $props()

	function handleEmptySpaceContextMenu(e: MouseEvent) {
		if (onEmptySpaceContextMenu) {
			e.preventDefault()
			onEmptySpaceContextMenu(e, folderId)
		}
	}

	function handleGridContextMenu(e: MouseEvent) {
		// Only trigger if clicking on the grid itself, not on a card
		if (e.target === e.currentTarget && onEmptySpaceContextMenu) {
			e.preventDefault()
			onEmptySpaceContextMenu(e, folderId)
		}
	}

	// Get children of this folder
	let children = $derived(getPlaylistChildren(playlists, folderId))

	// Sort: folders first, then playlists
	let sortedChildren = $derived(
		[...children].sort((a, b) => {
			if (a.is_folder && !b.is_folder) return -1
			if (!a.is_folder && b.is_folder) return 1
			return a.name.localeCompare(b.name)
		})
	)

	// Count children for each subfolder
	function getChildCount(playlist: Playlist): number {
		if (!playlist.is_folder) return 0
		return getPlaylistChildren(playlists, playlist.id).length
	}
</script>

<div class="flex h-full flex-col overflow-hidden bg-surface-0">
	<!-- Breadcrumb Navigation -->
	<Breadcrumbs items={breadcrumbItems} onNavigate={onBreadcrumbNavigate} onContextMenu={onBreadcrumbContextMenu} />

	<!-- Content -->
	<div class="flex-1 overflow-auto p-6">
		{#if sortedChildren.length === 0}
			<div
				class="flex h-full flex-col items-center justify-center text-text-tertiary"
				oncontextmenu={handleEmptySpaceContextMenu}
				role="region"
			>
				<Icon name="folder" class="mb-3 h-12 w-12" />
				<p class="text-sm">This folder is empty</p>
			</div>
		{:else}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="grid grid-cols-2 gap-4 lg:grid-cols-3 xl:grid-cols-4" oncontextmenu={handleGridContextMenu}>
				{#each sortedChildren as child (child.id)}
					<FolderCard playlist={child} childCount={getChildCount(child)} onclick={() => onSelect(child)} />
				{/each}
			</div>
		{/if}
	</div>
</div>
