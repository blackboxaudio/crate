<script lang="ts">
	import type { Playlist, BreadcrumbItem, Tag, TagCategory, TagFilterMode } from '$shared/types'
	import { getPlaylistChildren } from '$shared/stores/playlists'
	import FolderCard from './FolderCard.svelte'
	import { SearchBar, FilterDropdown } from '$lib/components/library'
	import { FollowingButton } from '$lib/components/follow'
	import Breadcrumbs from '$lib/components/common/Breadcrumbs.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { translate } from '$shared/i18n'

	type Props = {
		folderId: string
		playlists: Playlist[]
		onSelect: (playlist: Playlist) => void
		breadcrumbItems: BreadcrumbItem[]
		onBreadcrumbNavigate: (item: BreadcrumbItem) => void
		onBreadcrumbContextMenu: (e: MouseEvent, item: BreadcrumbItem) => void
		onEmptySpaceContextMenu?: (e: MouseEvent, folderId: string) => void
		onCardContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		searchValue?: string
		onSearchChange?: (query: string) => void
		activeFilterTags?: Tag[]
		tagCategories?: TagCategory[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onToggleTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
		isDiscoveryContext?: boolean
		likedOnly?: boolean
		onToggleLikedFilter?: () => void
	}

	let {
		folderId,
		playlists,
		onSelect,
		breadcrumbItems,
		onBreadcrumbNavigate,
		onBreadcrumbContextMenu,
		onEmptySpaceContextMenu,
		onCardContextMenu,
		searchValue = '',
		onSearchChange,
		activeFilterTags = [],
		tagCategories = [],
		tagColors = new Map(),
		tagFilterMode = 'or',
		onToggleTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
		isDiscoveryContext = false,
		likedOnly = false,
		onToggleLikedFilter,
	}: Props = $props()

	function handleContentContextMenu(e: MouseEvent) {
		// Don't trigger if clicking on a FolderCard (button element)
		const target = e.target as HTMLElement
		if (target.closest('button')) return

		if (onEmptySpaceContextMenu) {
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
	<Breadcrumbs items={breadcrumbItems} onNavigate={onBreadcrumbNavigate} onContextMenu={onBreadcrumbContextMenu}>
		{#snippet actions()}
			<div class="flex items-center gap-2">
				{#if onSearchChange}
					<div class="w-64">
						<SearchBar
							{onSearchChange}
							initialValue={searchValue}
							placeholder={isDiscoveryContext ? $translate('discovery.searchPlaceholder') : undefined}
						/>
					</div>
				{/if}
				{#if isDiscoveryContext}
					<FollowingButton />
				{/if}
				<FilterDropdown
					{activeFilterTags}
					{tagCategories}
					{tagColors}
					{tagFilterMode}
					onToggleTagFilter={(tagId) => onToggleTagFilter?.(tagId)}
					onClearAll={() => onClearAllTagFilters?.()}
					onToggleTagFilterMode={() => onToggleTagFilterMode?.()}
					showLikedFilter={isDiscoveryContext}
					likedOnly={isDiscoveryContext ? likedOnly : false}
					onToggleLikedFilter={isDiscoveryContext ? onToggleLikedFilter : undefined}
				/>
			</div>
		{/snippet}
	</Breadcrumbs>

	<!-- Content -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="flex-1 overflow-auto p-6" oncontextmenu={handleContentContextMenu}>
		{#if sortedChildren.length === 0}
			<div class="flex h-full flex-col items-center justify-center text-text-tertiary" role="region">
				<Icon name="folder" class="mb-3 h-12 w-12" />
				<Text color="tertiary">{$translate('playlists.folderEmpty')}</Text>
			</div>
		{:else}
			<div class="grid grid-cols-2 gap-4 lg:grid-cols-3 xl:grid-cols-4">
				{#each sortedChildren as child (child.id)}
					<FolderCard
						playlist={child}
						childCount={getChildCount(child)}
						onclick={() => onSelect(child)}
						oncontextmenu={(e) => {
							e.preventDefault()
							onCardContextMenu?.(e, child)
						}}
					/>
				{/each}
			</div>
		{/if}
	</div>
</div>
