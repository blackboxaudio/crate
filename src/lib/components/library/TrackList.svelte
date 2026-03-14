<script lang="ts">
	import type { Track, TrackColor, SortConfig } from '$lib/types'
	import { tick } from 'svelte'
	import { handleSelection } from '$lib/utils'
	import { createVirtualList } from '$lib/utils/virtualizer.svelte'
	import { analyzingTrackIds } from '$lib/stores'
	import { translate } from '$lib/i18n'
	import TrackListHeader from './TrackListHeader.svelte'
	import TrackRow from './TrackRow.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'

	type Props = {
		tracks: Track[]
		selectedIds: Set<string>
		playingTrackId?: string | null
		sortConfig: SortConfig
		isDragOver?: boolean
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		scrollOffset?: number
		onSelectionChange?: (ids: Set<string>) => void
		onTrackPlay?: (track: Track) => void
		onSortChange?: (config: SortConfig) => void
		onContextMenu?: (e: MouseEvent, track: Track) => void
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onTrackColorChange?: (trackIds: string[], color: TrackColor | null) => void
		onCancelAnalysis?: (trackId: string) => void
		onScrollChange?: (offset: number) => void
	}

	let {
		tracks,
		selectedIds,
		playingTrackId = null,
		sortConfig,
		isDragOver = false,
		categoryColors,
		categorySortOrders,
		scrollOffset = 0,
		onSelectionChange,
		onTrackPlay,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onTrackColorChange,
		onCancelAnalysis,
		onScrollChange,
	}: Props = $props()

	let lastClickedId: string | null = $state(null)
	let scrollContainerEl: HTMLElement | undefined = $state(undefined)
	let scrollRestoredForView = $state(false)
	let scrollDebounceTimer: ReturnType<typeof setTimeout> | null = null

	const virtualList = createVirtualList({
		count: () => tracks.length,
		getScrollElement: () => scrollContainerEl ?? null,
		estimateSize: () => () => 33,
		overscan: 15,
		getItemKey: (index: number) => tracks[index]?.id ?? index,
	})

	// Restore scroll position once after virtualizer mounts.
	// Uses tick() + direct scrollTop to restore before paint (no one-frame flash).
	$effect(() => {
		if (scrollContainerEl && !scrollRestoredForView) {
			scrollRestoredForView = true
			if (scrollOffset > 0) {
				tick().then(() => {
					scrollContainerEl!.scrollTop = scrollOffset
				})
			}
		}
	})

	function handleScroll() {
		if (!scrollContainerEl || !onScrollChange) return
		if (scrollDebounceTimer) clearTimeout(scrollDebounceTimer)
		scrollDebounceTimer = setTimeout(() => {
			if (scrollContainerEl) {
				onScrollChange(scrollContainerEl.scrollTop)
			}
		}, 100)
	}

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
		const target = e.target as HTMLElement
		if (target.closest('[data-track-row]')) return
		onSelectionChange?.(new Set())
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
	<div
		bind:this={scrollContainerEl}
		class="relative flex-1 overflow-auto"
		data-drop-target="tracklist-main"
		onclick={handleContainerClick}
		oncontextmenu={handleContainerContextMenu}
		onscroll={handleScroll}
	>
		{#if isDragOver}
			<div class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center bg-brand-muted">
				<div class="rounded-lg border-2 border-dashed border-brand-primary bg-surface-1/90 px-8 py-6 text-center">
					<Icon name="upload" class="mx-auto mb-2 h-10 w-10 text-brand-primary" />
					<Text variant="body-2" color="brand">Drop audio files to import</Text>
				</div>
			</div>
		{/if}
		{#if tracks.length === 0}
			<div class="flex h-full flex-col items-center justify-center p-8 text-text-tertiary">
				<Icon name="music-note" class="mb-4 h-16 w-16" />
				<Text variant="header-1" weight="medium" class="mb-2">{$translate('library.noTracksYet')}</Text>
				<Text color="tertiary">{$translate('library.dragDropHint')}</Text>
			</div>
		{:else}
			<div style="height: {virtualList.totalSize}px; position: relative; pointer-events: none;">
				{#each virtualList.virtualItems as virtualItem (virtualItem.key)}
					{@const track = tracks[virtualItem.index]}
					<div
						style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({virtualItem.start}px); pointer-events: auto;"
					>
						<TrackRow
							{track}
							selected={selectedIds.has(track.id)}
							playing={playingTrackId === track.id}
							analyzing={$analyzingTrackIds.has(track.id)}
							dragTrackIds={Array.from(selectedIds)}
							{categoryColors}
							{categorySortOrders}
							onclick={(e) => handleTrackClick(track, e)}
							ondblclick={() => handleTrackDoubleClick(track)}
							oncontextmenu={(e) => handleTrackContextMenu(track, e)}
							onColorChange={(color) => handleColorChange(track, color)}
							onCancelAnalysis={() => onCancelAnalysis?.(track.id)}
						/>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
