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
		TagCategory,
		TagFilterMode,
	} from '$shared/types'
	import { TrackList, SearchBar, FilterDropdown } from '$lib/components/library'
	import { DiscoveryList } from '$lib/components/discovery'
	import { FollowingButton } from '$lib/components/follow'
	import { IconButton } from '$lib/components/common'
	import Breadcrumbs from '$lib/components/common/Breadcrumbs.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$shared/i18n'
	import { sortDiscoveryReleases } from '$shared/utils/sorting'
	import {
		expandedReleaseIds,
		discoveryStore,
		facetFilters,
		likedFilter,
		newFilter,
		purchasedFilter,
		downloadedFilter,
		hasLinkedCollection,
		ownedReleaseIds,
		fullyCachedIds,
	} from '$lib/stores'
	import { applyDiscoveryFilters, emptyFacetFilters } from '$shared/utils/discoveryFilters'
	import { releaseHasTag } from '$shared/utils/tagComputation'

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
		tagCategories?: TagCategory[]
		tagColors?: Map<string, string | null>
		tagFilterMode?: TagFilterMode
		onToggleTagFilter?: (tagId: string) => void
		onClearAllTagFilters?: () => void
		onToggleTagFilterMode?: () => void
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		selectedDiscoveryTrackIds?: Set<string>
		onDiscoveryTrackSelectionChange?: (ids: Set<string>) => void
		onDiscoveryTrackPlay?: (release: DiscoveryRelease, trackIndex: number) => void
		onDiscoveryTrackLikeToggle?: (releaseId: string, trackId: string) => void
		onDiscoveryTrackContextMenu?: (
			release: DiscoveryRelease,
			trackIndex: number,
			canPlay: boolean,
			e: MouseEvent
		) => void
		onReleaseImport?: (release: DiscoveryRelease) => void
		onReleaseOpenUrl?: (release: DiscoveryRelease) => void
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
		activeFilterTags = [],
		tagCategories = [],
		tagColors = new Map(),
		tagFilterMode = 'or',
		onToggleTagFilter,
		onClearAllTagFilters,
		onToggleTagFilterMode,
		onSelectionChange,
		onTrackPlay,
		selectedDiscoveryTrackIds = new Set<string>(),
		onDiscoveryTrackSelectionChange,
		onDiscoveryTrackPlay,
		onDiscoveryTrackLikeToggle,
		onDiscoveryTrackContextMenu,
		onReleaseImport,
		onReleaseOpenUrl,
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
		// The same facet semantics as the feed (shared helper), so what's shown here matches what plays.
		let result = applyDiscoveryFilters(releases, isDiscovery ? $facetFilters : emptyFacetFilters(), {
			ownedIds: $ownedReleaseIds,
			cachedIds: $fullyCachedIds,
		})
		if (activeFilterTags && activeFilterTags.length > 0) {
			const tagIds = new Set(activeFilterTags.map((t) => t.id))
			if (tagFilterMode === 'and') {
				result = result.filter((r) => [...tagIds].every((id) => releaseHasTag(r, id)))
			} else {
				result = result.filter((r) => [...tagIds].some((id) => releaseHasTag(r, id)))
			}
		}
		if (searchValue) {
			const search = searchValue.toLowerCase()
			result = result.filter(
				(r) =>
					r.artist?.toLowerCase().includes(search) ||
					r.title?.toLowerCase().includes(search) ||
					r.label?.toLowerCase().includes(search) ||
					r.notes?.toLowerCase().includes(search) ||
					r.tracks.some((t) => t.name?.toLowerCase().includes(search))
			)
		}

		// Apply sorting via the one shared comparator (handles release_date validity, track_count, ties).
		return sortDiscoveryReleases(result, discoverySortConfig)
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
						/>
					</div>
				{/if}
				{#if isDiscovery}
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
					liked={isDiscovery
						? { value: $likedFilter, onChange: (s) => discoveryStore.setFacetFilter('liked', s) }
						: undefined}
					newReleases={isDiscovery
						? { value: $newFilter, onChange: (s) => discoveryStore.setFacetFilter('new', s) }
						: undefined}
					purchased={isDiscovery && $hasLinkedCollection
						? { value: $purchasedFilter, onChange: (s) => discoveryStore.setFacetFilter('purchased', s) }
						: undefined}
					downloaded={isDiscovery
						? { value: $downloadedFilter, onChange: (s) => discoveryStore.setFacetFilter('downloaded', s) }
						: undefined}
				/>
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
				selectedTrackIds={selectedDiscoveryTrackIds}
				onTrackSelectionChange={onDiscoveryTrackSelectionChange}
				expandedIds={$expandedReleaseIds}
				sortConfig={discoverySortConfig}
				{categoryColors}
				{categorySortOrders}
				likedOnly={isDiscovery && $likedFilter === 'include'}
				hasAnyReleases={releases.length > 0}
				{scrollOffset}
				{onSelectionChange}
				onSortChange={onDiscoverySortChange}
				onContextMenu={(e, release) => {
					onContextMenu?.(e, release as unknown as Track)
				}}
				onToggleExpand={(id) => expandedReleaseIds.toggle(id)}
				onTrackPlay={onDiscoveryTrackPlay}
				onTrackLikeToggle={onDiscoveryTrackLikeToggle}
				onTrackContextMenu={onDiscoveryTrackContextMenu}
				{onReleaseImport}
				{onReleaseOpenUrl}
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
