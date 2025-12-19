<script lang="ts">
	import type { Track, SortConfig } from '$lib/types'
	import { handleSelection } from '$lib/utils'
	import TrackListHeader from './TrackListHeader.svelte'
	import TrackRow from './TrackRow.svelte'

	type Props = {
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

	let lastClickedId: string | null = $state(null)

	function handleTrackClick(track: Track, e: MouseEvent) {
		const result = handleSelection(tracks, selectedIds, track.id, lastClickedId, {
			shiftKey: e.shiftKey,
			metaKey: e.metaKey,
			ctrlKey: e.ctrlKey,
		})

		lastClickedId = result.lastClickedId
		onSelectionChange?.(result.selectedIds)
	}

	function handleTrackDoubleClick(track: Track) {
		onTrackPlay?.(track)
	}

	function handleTrackContextMenu(track: Track, e: MouseEvent) {
		e.preventDefault()

		// If track not selected, select it
		if (!selectedIds.has(track.id)) {
			onSelectionChange?.(new Set([track.id]))
		}

		onContextMenu?.(e, track)
	}
</script>

<div class="flex h-full flex-col bg-zinc-950 {isDragOver ? 'ring-2 ring-blue-500 ring-inset' : ''}">
	<TrackListHeader {sortConfig} onSort={onSortChange} />

	<div class="relative flex-1 overflow-auto">
		{#if isDragOver}
			<div class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-blue-500/10">
				<div class="rounded-lg border-2 border-dashed border-blue-500 bg-zinc-900/90 px-8 py-6 text-center">
					<svg class="mx-auto mb-2 h-10 w-10 text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path
							stroke-linecap="round"
							stroke-linejoin="round"
							stroke-width="2"
							d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
						/>
					</svg>
					<p class="text-sm font-medium text-blue-500">Drop audio files to import</p>
				</div>
			</div>
		{/if}
		{#if tracks.length === 0}
			<div class="flex h-full flex-col items-center justify-center p-8 text-zinc-500">
				<svg class="mb-4 h-16 w-16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="1.5"
						d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
					/>
				</svg>
				<p class="mb-2 text-lg font-medium">No tracks yet</p>
				<p class="text-sm">Drag and drop audio files here to import them</p>
			</div>
		{:else}
			{#each tracks as track (track.id)}
				<TrackRow
					{track}
					selected={selectedIds.has(track.id)}
					playing={playingTrackId === track.id}
					dragTrackIds={Array.from(selectedIds)}
					{categoryColors}
					onclick={(e) => handleTrackClick(track, e)}
					ondblclick={() => handleTrackDoubleClick(track)}
					oncontextmenu={(e) => handleTrackContextMenu(track, e)}
				/>
			{/each}
		{/if}
	</div>
</div>
