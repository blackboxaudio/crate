<script lang="ts">
	import type { Track, TrackColor, SortConfig } from '$lib/types'
	import { handleSelection } from '$lib/utils'
	import TrackListHeader from './TrackListHeader.svelte'
	import TrackRow from './TrackRow.svelte'
	import Icon from '$lib/components/common/Icon.svelte'

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
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onTrackColorChange?: (trackIds: string[], color: TrackColor | null) => void
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
		onEmptySpaceContextMenu,
		onTrackColorChange,
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

	function handleContainerClick(e: MouseEvent) {
		if (e.target === e.currentTarget) {
			onSelectionChange?.(new Set())
		}
	}

	function handleContainerContextMenu(e: MouseEvent) {
		const target = e.target as HTMLElement
		// Don't trigger if right-clicking on a track row
		if (target.closest('[data-track-row]')) return

		if (onEmptySpaceContextMenu) {
			e.preventDefault()
			onEmptySpaceContextMenu(e)
		}
	}

	function handleColorChange(track: Track, color: TrackColor | null) {
		// If track is selected, apply to all selected tracks; otherwise just this track
		const ids = selectedIds.has(track.id) ? Array.from(selectedIds) : [track.id]
		onTrackColorChange?.(ids, color)
	}
</script>

<div class="flex h-full flex-col bg-surface-0 {isDragOver ? 'ring-2 ring-brand-primary ring-inset' : ''}">
	<TrackListHeader {sortConfig} onSort={onSortChange} />

	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="relative flex-1 overflow-auto" onclick={handleContainerClick} oncontextmenu={handleContainerContextMenu}>
		{#if isDragOver}
			<div class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-brand-muted">
				<div class="rounded-lg border-2 border-dashed border-brand-primary bg-surface-1/90 px-8 py-6 text-center">
					<Icon name="upload" class="mx-auto mb-2 h-10 w-10 text-brand-primary" />
					<p class="text-sm font-medium text-brand-primary">Drop audio files to import</p>
				</div>
			</div>
		{/if}
		{#if tracks.length === 0}
			<div class="flex h-full flex-col items-center justify-center p-8 text-text-tertiary">
				<Icon name="music-note" class="mb-4 h-16 w-16" />
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
					onColorChange={(color) => handleColorChange(track, color)}
				/>
			{/each}
		{/if}
	</div>
</div>
