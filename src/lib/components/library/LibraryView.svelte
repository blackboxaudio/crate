<script lang="ts">
	import type { Track, SortConfig } from '$lib/types'
	import TrackList from './TrackList.svelte'

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
	<!-- Header -->
	<div class="flex items-center gap-3 border-b border-stroke px-6 py-4">
		<svg class="h-5 w-5 text-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path
				stroke-linecap="round"
				stroke-linejoin="round"
				stroke-width="2"
				d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
			/>
		</svg>
		<h2 class="text-lg font-medium text-text-primary">Library</h2>
		<span class="text-sm text-text-tertiary">
			{trackCount}
			{trackCount === 1 ? 'track' : 'tracks'}
		</span>
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
