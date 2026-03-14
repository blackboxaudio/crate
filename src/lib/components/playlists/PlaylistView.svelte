<script lang="ts">
	import type {
		Playlist,
		Track,
		TrackColor,
		SortConfig,
		BreadcrumbItem,
		DiscoveryRelease,
		DiscoverySortConfig,
		Tag,
		TagFilterMode,
	} from '$lib/types'
	import { TrackList, SearchBar } from '$lib/components/library'
	import { DiscoveryList } from '$lib/components/discovery'
	import { IconButton } from '$lib/components/common'
	import Breadcrumbs from '$lib/components/common/Breadcrumbs.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$lib/i18n'
	import { expandedReleaseIds } from '$lib/stores'

	type Props = {
		playlist: Playlist
		tracks: Track[]
		selectedIds: Set<string>
		playingTrackId?: string | null
		sortConfig: SortConfig
		isDragOver?: boolean
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		breadcrumbItems: BreadcrumbItem[]
		isDiscovery?: boolean
		releases?: DiscoveryRelease[]
		editorVisible?: boolean
		hasSelection?: boolean
		searchValue?: string
		onSearchChange?: (query: string) => void
		activeFilterTags?: Tag[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onRemoveTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
		likedOnly?: boolean
		onToggleLikedFilter?: () => void
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		onDiscoveryTrackPlay?: (release: DiscoveryRelease, trackIndex: number) => void
		onDiscoveryTrackLikeToggle?: (releaseId: string, trackId: string) => void
		onSortChange?: (config: SortConfig) => void
		discoverySortConfig?: DiscoverySortConfig
		onDiscoverySortChange?: (config: DiscoverySortConfig) => void
		onContextMenu?: (e: MouseEvent, track: Track) => void
		onEmptySpaceContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		onBreadcrumbNavigate: (item: BreadcrumbItem) => void
		onBreadcrumbContextMenu: (e: MouseEvent, item: BreadcrumbItem) => void
		onTrackColorChange?: (trackIds: string[], color: TrackColor | null) => void
		onCancelAnalysis?: (trackId: string) => void
		onToggleEditor?: () => void
		scrollOffset?: number
		onScrollChange?: (offset: number) => void
	}

	let {
		playlist,
		tracks,
		selectedIds,
		playingTrackId = null,
		sortConfig,
		isDragOver = false,
		categoryColors,
		categorySortOrders,
		breadcrumbItems,
		isDiscovery = false,
		releases = [],
		editorVisible = false,
		hasSelection = false,
		searchValue = '',
		onSearchChange,
		activeFilterTags,
		tagColors,
		tagFilterMode,
		onRemoveTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
		likedOnly = false,
		onToggleLikedFilter,
		onSelectionChange,
		onTrackPlay,
		onDiscoveryTrackPlay,
		onDiscoveryTrackLikeToggle,
		onSortChange,
		discoverySortConfig = { field: 'artist', direction: 'asc' },
		onDiscoverySortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onBreadcrumbNavigate,
		onBreadcrumbContextMenu,
		onTrackColorChange,
		onCancelAnalysis,
		onToggleEditor,
		scrollOffset = 0,
		onScrollChange,
	}: Props = $props()

	function handleEmptySpaceContextMenu(e: MouseEvent) {
		onEmptySpaceContextMenu?.(e, playlist)
	}

	const filteredReleases = $derived.by(() => {
		let result = likedOnly ? releases.filter((r) => r.tracks.some((t) => t.is_liked)) : [...releases]
		if (activeFilterTags && activeFilterTags.length > 0) {
			const tagIds = new Set(activeFilterTags.map((t) => t.id))
			if (tagFilterMode === 'and') {
				result = result.filter((r) => [...tagIds].every((id) => r.tags.some((t) => t.id === id)))
			} else {
				result = result.filter((r) => r.tags.some((t) => tagIds.has(t.id)))
			}
		}
		if (searchValue) {
			const search = searchValue.toLowerCase()
			result = result.filter(
				(r) =>
					r.artist?.toLowerCase().includes(search) ||
					r.title?.toLowerCase().includes(search) ||
					r.label?.toLowerCase().includes(search) ||
					r.notes?.toLowerCase().includes(search)
			)
		}

		// Apply sorting
		const { field, direction } = discoverySortConfig
		const dir = direction === 'asc' ? 1 : -1
		result.sort((a, b) => {
			let cmp = 0
			if (field === 'release_date') {
				const aDate = a.release_date ? new Date(a.release_date).getTime() : NaN
				const bDate = b.release_date ? new Date(b.release_date).getTime() : NaN
				const aValid = !isNaN(aDate)
				const bValid = !isNaN(bDate)
				if (!aValid && !bValid) cmp = 0
				else if (!aValid) return 1
				else if (!bValid) return -1
				else if (aDate < bDate) cmp = -1 * dir
				else if (aDate > bDate) cmp = 1 * dir
			} else {
				const aVal = a[field] ?? ''
				const bVal = b[field] ?? ''
				if (aVal < bVal) cmp = -1 * dir
				else if (aVal > bVal) cmp = 1 * dir
			}
			if (cmp !== 0) return cmp
			return a.id < b.id ? -1 : a.id > b.id ? 1 : 0
		})

		return result
	})

	const hasExpandableReleases = $derived(filteredReleases.some((r) => r.tracks.length > 0))

	function handleExpandAll() {
		expandedReleaseIds.expandAll(filteredReleases.filter((r) => r.tracks.length > 0).map((r) => r.id))
	}

	function handleCollapseAll() {
		expandedReleaseIds.collapseAll()
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
							placeholder={isDiscovery ? $translate('discovery.searchPlaceholder') : undefined}
							likedOnly={isDiscovery ? likedOnly : undefined}
							onToggleLikedFilter={isDiscovery ? onToggleLikedFilter : undefined}
							{activeFilterTags}
							{tagColors}
							{tagFilterMode}
							{onRemoveTagFilter}
							{onClearAllTagFilters}
							{onToggleTagFilterMode}
						/>
					</div>
				{/if}
				{#if isDiscovery}
					<Tooltip text={$translate('discovery.expandAll')} position="bottom" delay={250}>
						<IconButton icon="unfold-vertical" size="sm" disabled={!hasExpandableReleases} onclick={handleExpandAll} />
					</Tooltip>
					<Tooltip text={$translate('discovery.collapseAll')} position="bottom" delay={250}>
						<IconButton icon="fold-vertical" size="sm" disabled={!hasExpandableReleases} onclick={handleCollapseAll} />
					</Tooltip>
				{/if}
				<Tooltip
					text={editorVisible ? $translate('editor.hideEditor') : $translate('editor.showEditor')}
					position="bottom"
					delay={250}
				>
					<IconButton
						icon="panel-right"
						size="sm"
						active={editorVisible && hasSelection}
						disabled={!hasSelection}
						onclick={onToggleEditor}
					/>
				</Tooltip>
			</div>
		{/snippet}
	</Breadcrumbs>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		{#if isDiscovery}
			<DiscoveryList
				releases={filteredReleases}
				{selectedIds}
				expandedIds={$expandedReleaseIds}
				sortConfig={discoverySortConfig}
				{categoryColors}
				{categorySortOrders}
				{likedOnly}
				{scrollOffset}
				{onSelectionChange}
				onSortChange={onDiscoverySortChange}
				onContextMenu={(e, release) => {
					onContextMenu?.(e, release as unknown as Track)
				}}
				onToggleExpand={(id) => expandedReleaseIds.toggle(id)}
				onTrackPlay={onDiscoveryTrackPlay}
				onTrackLikeToggle={onDiscoveryTrackLikeToggle}
				{onScrollChange}
			/>
		{:else}
			<TrackList
				{tracks}
				{selectedIds}
				{playingTrackId}
				{sortConfig}
				{isDragOver}
				{categoryColors}
				{categorySortOrders}
				{scrollOffset}
				{onSelectionChange}
				{onTrackPlay}
				{onSortChange}
				{onContextMenu}
				onEmptySpaceContextMenu={handleEmptySpaceContextMenu}
				{onTrackColorChange}
				{onCancelAnalysis}
				{onScrollChange}
			/>
		{/if}
	</div>
</div>
