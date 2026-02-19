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
	import { IconButton } from '$lib/components/common'
	import Breadcrumbs from '$lib/components/common/Breadcrumbs.svelte'
	import Tooltip from '$lib/components/common/Tooltip.svelte'
	import { translate } from '$lib/i18n'
	import { SvelteSet } from 'svelte/reactivity'

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
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		onSortChange?: (config: SortConfig) => void
		onContextMenu?: (e: MouseEvent, track: Track) => void
		onEmptySpaceContextMenu?: (e: MouseEvent, playlist: Playlist) => void
		onBreadcrumbNavigate: (item: BreadcrumbItem) => void
		onBreadcrumbContextMenu: (e: MouseEvent, item: BreadcrumbItem) => void
		onTrackColorChange?: (trackIds: string[], color: TrackColor | null) => void
		onCancelAnalysis?: (trackId: string) => void
		onToggleEditor?: () => void
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
		onSelectionChange,
		onTrackPlay,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onBreadcrumbNavigate,
		onBreadcrumbContextMenu,
		onTrackColorChange,
		onCancelAnalysis,
		onToggleEditor,
	}: Props = $props()

	function handleEmptySpaceContextMenu(e: MouseEvent) {
		onEmptySpaceContextMenu?.(e, playlist)
	}

	// Discovery expand/collapse state
	let expandedIds = $state(new Set<string>())

	function toggleExpand(id: string) {
		expandedIds = new SvelteSet(expandedIds)
		if (expandedIds.has(id)) {
			expandedIds.delete(id)
		} else {
			expandedIds.add(id)
		}
	}

	const hasExpandableReleases = $derived(releases.some((r) => r.tracks.length > 0))

	function expandAll() {
		expandedIds = new SvelteSet(releases.filter((r) => r.tracks.length > 0).map((r) => r.id))
	}

	function collapseAll() {
		expandedIds = new SvelteSet()
	}

	// Provide a default sort config for discovery lists
	const discoverySortConfig: DiscoverySortConfig = { field: 'date_added', direction: 'desc' }
</script>

<div class="flex h-full flex-col overflow-hidden bg-surface-0">
	<!-- Breadcrumb Navigation -->
	<Breadcrumbs items={breadcrumbItems} onNavigate={onBreadcrumbNavigate} onContextMenu={onBreadcrumbContextMenu}>
		{#snippet actions()}
			<div class="flex items-center gap-1">
				{#if isDiscovery && hasExpandableReleases}
					<Tooltip text={$translate('discovery.expandAll')} position="bottom" delay={250}>
						<IconButton icon="unfold-vertical" size="sm" onclick={expandAll} />
					</Tooltip>
					<Tooltip text={$translate('discovery.collapseAll')} position="bottom" delay={250}>
						<IconButton icon="fold-vertical" size="sm" onclick={collapseAll} />
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
				{releases}
				{selectedIds}
				{expandedIds}
				sortConfig={discoverySortConfig}
				{categoryColors}
				{categorySortOrders}
				{onSelectionChange}
				onContextMenu={(e, release) => {
					onContextMenu?.(e, release as unknown as Track)
				}}
				onToggleExpand={toggleExpand}
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
