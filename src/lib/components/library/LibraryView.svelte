<script lang="ts">
	import type { Track, TrackColor, SortConfig } from '$lib/types'
	import TrackList from './TrackList.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import { translate } from '$lib/i18n'

	type Props = {
		tracks: Track[]
		trackCount: number
		selectedIds: Set<string>
		playingTrackId?: string | null
		sortConfig: SortConfig
		isDragOver?: boolean
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		onSortChange?: (config: SortConfig) => void
		onContextMenu?: (e: MouseEvent, track: Track) => void
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onTrackColorChange?: (trackIds: string[], color: TrackColor | null) => void
		onCancelAnalysis?: () => void
	}

	let {
		tracks,
		trackCount,
		selectedIds,
		playingTrackId = null,
		sortConfig,
		isDragOver = false,
		categoryColors,
		categorySortOrders,
		onSelectionChange,
		onTrackPlay,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onTrackColorChange,
		onCancelAnalysis,
	}: Props = $props()
</script>

<div class="flex h-full flex-col overflow-hidden bg-surface-0">
	<!-- Header (matches Breadcrumbs styling) -->
	<div class="flex items-center gap-1 border-b border-stroke px-6 py-4">
		<div class="flex items-center gap-2 rounded px-2 py-1 text-sm font-medium text-text-primary">
			<Icon name="library" class="h-4 w-4 shrink-0" />
			<span>{$translate('nav.library')}</span>
			<span class="ml-2 text-text-tertiary">
				{trackCount}
				{trackCount === 1 ? $translate('library.track') : $translate('library.tracks')}
			</span>
		</div>
	</div>

	<!-- Content -->
	<div class="flex-1 overflow-hidden">
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
			{onEmptySpaceContextMenu}
			{onTrackColorChange}
			{onCancelAnalysis}
		/>
	</div>
</div>
