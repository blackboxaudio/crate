<script lang="ts">
	import type { DiscoveryRelease, DiscoverySortConfig, DiscoverySourceType } from '$lib/types'
	import { handleSelection } from '$lib/utils'
	import { createVirtualList } from '$lib/utils/virtualizer.svelte'
	import { translate } from '$lib/i18n'
	import DiscoveryListHeader from './DiscoveryListHeader.svelte'
	import DiscoveryRow from './DiscoveryRow.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'

	const BASE_PREVIEWABLE: Set<DiscoverySourceType> = new Set(['bandcamp', 'soundcloud', 'youtube'])

	function isReleasePreviewable(release: DiscoveryRelease): boolean {
		if (BASE_PREVIEWABLE.has(release.source_type)) return true
		return release.tracks.some((t) => t.video_id !== null)
	}

	const ROW_HEIGHT = 49
	const TRACK_ROW_HEIGHT = 29

	type Props = {
		releases: DiscoveryRelease[]
		selectedIds: Set<string>
		expandedIds?: Set<string>
		sortConfig: DiscoverySortConfig
		categoryColors?: Map<string, string | null>
		categorySortOrders?: Map<string, number>
		isDragOver?: boolean
		scrollOffset?: number
		onSelectionChange?: (ids: Set<string>) => void
		onReleaseOpen?: (release: DiscoveryRelease) => void
		onReleaseOpenUrl?: (release: DiscoveryRelease) => void
		onReleaseImport?: (release: DiscoveryRelease) => void
		onSortChange?: (config: DiscoverySortConfig) => void
		onContextMenu?: (e: MouseEvent, release: DiscoveryRelease) => void
		onEmptySpaceContextMenu?: (e: MouseEvent) => void
		onToggleExpand?: (id: string) => void
		onTrackPlay?: (release: DiscoveryRelease, trackIndex: number) => void
		onTrackLikeToggle?: (releaseId: string, trackId: string) => void
		onScrollChange?: (offset: number) => void
		likedOnly?: boolean
	}

	let {
		releases,
		selectedIds,
		expandedIds = new Set<string>(),
		sortConfig,
		categoryColors,
		categorySortOrders,
		isDragOver = false,
		scrollOffset = 0,
		onSelectionChange,
		onReleaseOpen,
		onReleaseOpenUrl,
		onReleaseImport,
		onSortChange,
		onContextMenu,
		onEmptySpaceContextMenu,
		onToggleExpand,
		onTrackPlay,
		onTrackLikeToggle,
		onScrollChange,
		likedOnly = false,
	}: Props = $props()

	let lastClickedId: string | null = $state(null)
	let scrollContainerEl: HTMLElement | undefined = $state(undefined)
	let scrollRestoredForView = $state(false)
	let scrollDebounceTimer: ReturnType<typeof setTimeout> | null = null

	// Enable CSS transition on row containers when expandedIds changes (expand/collapse).
	// Uses $effect.pre so the transition class is present BEFORE the DOM update that
	// repositions rows — otherwise rows jump to new positions without animating.
	// Cleared on scroll to prevent laggy scroll-related position updates.
	let isTransitioning = $state(false)
	let transitionTimer: ReturnType<typeof setTimeout> | null = null
	let initialized = false

	$effect.pre(() => {
		void expandedIds
		if (!initialized) {
			initialized = true
			return
		}
		isTransitioning = true
		if (transitionTimer) clearTimeout(transitionTimer)
		transitionTimer = setTimeout(() => {
			isTransitioning = false
		}, 250)
	})

	function getEstimateSize(index: number): number {
		const release = releases[index]
		if (!release) return ROW_HEIGHT
		if (!expandedIds.has(release.id) || release.tracks.length === 0) return ROW_HEIGHT
		const visibleTracks = likedOnly ? release.tracks.filter((t) => t.is_liked).length : release.tracks.length
		// Header (49px) + first track (28px, no border-t) + remaining tracks (29px each, with border-t) + container border-b (1px)
		// = 49 + 28 + (N-1)*29 + 1 = 49 + 29N
		return ROW_HEIGHT + visibleTracks * TRACK_ROW_HEIGHT
	}

	const virtualList = createVirtualList({
		count: () => releases.length,
		getScrollElement: () => scrollContainerEl ?? null,
		// Read expandedIds and likedOnly so the virtualizer's $effect.pre re-runs when they change
		estimateSize: () => {
			void expandedIds
			void likedOnly
			return getEstimateSize
		},
		overscan: 10,
		getItemKey: (index: number) => releases[index]?.id ?? index,
	})

	// Restore scroll position after virtualizer mounts
	$effect(() => {
		if (scrollContainerEl && scrollOffset > 0 && !scrollRestoredForView) {
			scrollRestoredForView = true
			requestAnimationFrame(() => {
				virtualList.scrollToOffset(scrollOffset)
			})
		}
	})

	function handleScroll() {
		// Cancel expand/collapse transition during scroll to prevent laggy repositioning
		if (isTransitioning) {
			isTransitioning = false
			if (transitionTimer) clearTimeout(transitionTimer)
		}
		if (!scrollContainerEl || !onScrollChange) return
		if (scrollDebounceTimer) clearTimeout(scrollDebounceTimer)
		scrollDebounceTimer = setTimeout(() => {
			if (scrollContainerEl) {
				onScrollChange(scrollContainerEl.scrollTop)
			}
		}, 100)
	}

	function handleReleaseClick(release: DiscoveryRelease, e: MouseEvent) {
		const result = handleSelection(releases, selectedIds, release.id, lastClickedId, {
			shiftKey: e.shiftKey,
			metaKey: e.metaKey,
			ctrlKey: e.ctrlKey,
		})

		lastClickedId = result.lastClickedId
		onSelectionChange?.(result.selectedIds)
	}

	function handleReleaseDoubleClick(release: DiscoveryRelease) {
		onToggleExpand?.(release.id)
	}

	function handleReleaseContextMenu(release: DiscoveryRelease, e: MouseEvent) {
		e.preventDefault()

		if (!selectedIds.has(release.id)) {
			onSelectionChange?.(new Set([release.id]))
		}

		onContextMenu?.(e, release)
	}

	function handleContainerClick(e: MouseEvent) {
		const target = e.target as HTMLElement
		if (target.closest('[data-release-row]')) return
		onSelectionChange?.(new Set())
	}

	function handleContainerContextMenu(e: MouseEvent) {
		const target = e.target as HTMLElement
		if (target.closest('[data-release-row]')) return

		if (onEmptySpaceContextMenu) {
			e.preventDefault()
			onEmptySpaceContextMenu(e)
		}
	}
</script>

<div class="flex h-full flex-col bg-surface-0">
	<DiscoveryListHeader {sortConfig} onSort={onSortChange} />

	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		bind:this={scrollContainerEl}
		class="relative flex-1 overflow-auto"
		data-drop-target="releaselist-main"
		onclick={handleContainerClick}
		oncontextmenu={handleContainerContextMenu}
		onscroll={handleScroll}
	>
		{#if releases.length === 0}
			<div
				class="flex h-full flex-col items-center justify-center p-8 text-text-tertiary {isDragOver
					? 'border-brand-primary/50 bg-brand-primary/5 rounded-md border-2 border-dashed'
					: ''}"
			>
				<Icon name="globe" class="mb-4 h-16 w-16" />
				{#if isDragOver}
					<Text variant="header-1" weight="medium" class="mb-2">{$translate('discovery.dropHint')}</Text>
				{:else}
					<Text variant="header-1" weight="medium" class="mb-2">{$translate('discovery.noReleasesYet')}</Text>
					<Text color="tertiary" class="max-w-sm text-center">
						{$translate('discovery.addReleaseHint', { values: { shortcut: '⌘D' } })}
					</Text>
				{/if}
			</div>
		{:else}
			<div style="height: {virtualList.totalSize}px; position: relative; pointer-events: none;">
				{#each virtualList.virtualItems as virtualItem (virtualItem.key)}
					{@const release = releases[virtualItem.index]}
					<div
						class={isTransitioning ? 'transition-[transform,height] duration-200 ease-out' : ''}
						style="position: absolute; top: 0; left: 0; width: 100%; height: {virtualItem.size}px; overflow: hidden; transform: translateY({virtualItem.start}px); pointer-events: auto;"
					>
						<DiscoveryRow
							{release}
							selected={selectedIds.has(release.id)}
							expanded={expandedIds.has(release.id)}
							isPreviewable={isReleasePreviewable(release)}
							dragReleaseIds={Array.from(selectedIds)}
							{categoryColors}
							{categorySortOrders}
							{likedOnly}
							onclick={(e) => handleReleaseClick(release, e)}
							ondblclick={() => handleReleaseDoubleClick(release)}
							oncontextmenu={(e) => handleReleaseContextMenu(release, e)}
							onimport={() => onReleaseImport?.(release)}
							onopenurl={() => onReleaseOpenUrl?.(release)}
							onToggleExpand={() => onToggleExpand?.(release.id)}
							onTrackPlay={(idx) => onTrackPlay?.(release, idx)}
							onTrackLikeToggle={(trackId) => onTrackLikeToggle?.(release.id, trackId)}
						/>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
