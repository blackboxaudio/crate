<script lang="ts">
	import type {
		Playlist,
		Track,
		TrackColor,
		SortConfig,
		BreadcrumbItem,
		DiscoveryRelease,
		DiscoverySortConfig,
	} from '$lib/types'
	import { TrackList } from '$lib/components/library'
	import { DiscoveryList } from '$lib/components/discovery'
	import Breadcrumbs from '$lib/components/common/Breadcrumbs.svelte'

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
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		onSortChange?: (config: SortConfig) => void
		onContextMenu?: (e: MouseEvent, track: Track) => void
		onEmptySpaceContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		onBreadcrumbNavigate: (item: BreadcrumbItem) => void
		onBreadcrumbContextMenu: (e: MouseEvent, item: BreadcrumbItem) => void
		onTrackColorChange?: (trackIds: string[], color: TrackColor | null) => void
		onCancelAnalysis?: (trackId: string) => void
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
		onSelectionChange,
		onTrackPlay,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onBreadcrumbNavigate,
		onBreadcrumbContextMenu,
		onTrackColorChange,
		onCancelAnalysis,
	}: Props = $props()

	function handleEmptySpaceContextMenu(e: MouseEvent) {
		onEmptySpaceContextMenu?.(e, playlist)
	}

	// Provide a default sort config for discovery lists
	const discoverySortConfig: DiscoverySortConfig = { field: 'date_added', direction: 'desc' }
</script>

<div class="flex h-full flex-col overflow-hidden bg-surface-0">
	<!-- Breadcrumb Navigation -->
	<Breadcrumbs items={breadcrumbItems} onNavigate={onBreadcrumbNavigate} onContextMenu={onBreadcrumbContextMenu} />

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
		{#if isDiscovery}
			<DiscoveryList
				{releases}
				{selectedIds}
				sortConfig={discoverySortConfig}
				{categoryColors}
				{categorySortOrders}
				{onSelectionChange}
				onContextMenu={(e, release) => {
					// Adapt release context menu to the track-style callback
					onContextMenu?.(e, release as unknown as Track)
				}}
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
				{onSelectionChange}
				{onTrackPlay}
				{onSortChange}
				{onContextMenu}
				onEmptySpaceContextMenu={handleEmptySpaceContextMenu}
				{onTrackColorChange}
				{onCancelAnalysis}
			/>
		{/if}
	</div>
</div>
