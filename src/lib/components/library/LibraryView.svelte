<script lang="ts">
	import type { Track, SortConfig } from '$lib/types'
	import TrackList from './TrackList.svelte'
	import Icon from '$lib/components/common/Icon.svelte'

	type Props = {
		tracks: Track[]
		trackCount: number
		selectedIds: Set<string>
		playingTrackId?: string | null
		sortConfig: SortConfig
		isDragOver?: boolean
		categoryColors?: Map<string, string | null>
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		onSortChange?: (config: SortConfig) => void
		onContextMenu?: (e: MouseEvent, track: Track) => void
	}

	let {
		tracks,
		trackCount,
		selectedIds,
		playingTrackId = null,
		sortConfig,
		isDragOver = false,
		categoryColors,
		onSelectionChange,
		onTrackPlay,
		onSortChange,
		onContextMenu,
	}: Props = $props()
</script>

<div class="flex h-full flex-col overflow-hidden bg-surface-0">
	<!-- Header (matches Breadcrumbs styling) -->
	<div class="flex items-center gap-1 border-b border-stroke px-6 py-4">
		<div class="flex items-center gap-2 rounded px-2 py-1 text-sm font-medium text-text-primary">
			<Icon name="library" class="h-4 w-4 shrink-0" />
			<span>Library</span>
			<span class="ml-2 text-text-tertiary">
				{trackCount}
				{trackCount === 1 ? 'track' : 'tracks'}
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
			{onSelectionChange}
			{onTrackPlay}
			{onSortChange}
			{onContextMenu}
		/>
	</div>
</div>
