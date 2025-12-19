<script lang="ts">
	import type { Playlist, Track, SortConfig } from '$lib/types'
	import { TrackList } from '$lib/components/library'

	type Props = {
		playlist: Playlist
		tracks: Track[]
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
		playlist,
		tracks,
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
				d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
			/>
		</svg>
		<h2 class="text-lg font-medium text-text-primary">{playlist.name}</h2>
		<span class="text-sm text-text-tertiary">
			{tracks.length}
			{tracks.length === 1 ? 'track' : 'tracks'}
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
