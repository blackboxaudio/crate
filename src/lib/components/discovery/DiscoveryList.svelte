<script lang="ts">
	import type { DiscoveryRelease, DiscoverySortConfig, DiscoverySourceType } from '$lib/types'
	import { tick } from 'svelte'
	import { handleSelection } from '$lib/utils'
	import { createVirtualList } from '$lib/utils/virtualizer.svelte'
	import { translate } from '$lib/i18n'
	import DiscoveryListHeader from './DiscoveryListHeader.svelte'
	import DiscoveryRow from './DiscoveryRow.svelte'
	import Icon from '$lib/components/common/Icon.svelte'
	import Text from '$lib/components/common/Text.svelte'
	import { SvelteMap } from 'svelte/reactivity'

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

	// Animate expand/collapse using calculated deltas.
	// Instead of FLIP (which requires capturing old positions from DOM or cache),
	// we calculate the height delta from the toggled item's track count and
	// directly animate each visible element. This works reliably across component
	// remounts because it only depends on the post-tick DOM + known delta.
	// Defined BEFORE createVirtualList so this $effect.pre runs first.
	let prevExpandedIds: Set<string> | null = null

	$effect.pre(() => {
		const currentIds = expandedIds
		if (prevExpandedIds === null) {
			prevExpandedIds = currentIds
			return
		}
		if (currentIds !== prevExpandedIds) {
			// Find the single toggled item (skip animation for bulk operations)
			let toggledId: string | null = null
			let isExpanding = false
			for (const id of currentIds) {
				if (!prevExpandedIds.has(id)) {
					if (toggledId !== null) {
						toggledId = null
						break
					}
					toggledId = id
					isExpanding = true
				}
			}
			if (toggledId === null) {
				for (const id of prevExpandedIds) {
					if (!currentIds.has(id)) {
						if (toggledId !== null) {
							toggledId = null
							break
						}
						toggledId = id
						isExpanding = false
					}
				}
			}
			prevExpandedIds = currentIds

			if (!toggledId) return

			const toggledIndex = releases.findIndex((r) => r.id === toggledId)
			if (toggledIndex === -1) return

			const release = releases[toggledIndex]
			const visibleTracks = likedOnly ? release.tracks.filter((t) => t.is_liked).length : release.tracks.length
			const delta = visibleTracks * TRACK_ROW_HEIGHT * (isExpanding ? 1 : -1)

			if (Math.abs(delta) < 0.5) return

			// Build index lookup for matching DOM elements to release positions
			const indexById = new SvelteMap<string, number>()
			for (let i = 0; i < releases.length; i++) {
				indexById.set(releases[i].id, i)
			}

			tick().then(() => {
				if (!scrollContainerEl) return

				const wrappers = scrollContainerEl.querySelectorAll<HTMLElement>('[data-vkey]')
				for (const el of wrappers) {
					const key = el.getAttribute('data-vkey')
					if (!key) continue

					if (key === toggledId) {
						// Toggled item: animate height change
						const newSize = parseFloat(el.style.height) || 0
						el.animate([{ height: `${newSize - delta}px` }, { height: `${newSize}px` }], {
							duration: 200,
							easing: 'ease-out',
						})
					} else {
						// Items below the toggled item: animate position shift
						const itemIndex = indexById.get(key)
						if (itemIndex !== undefined && itemIndex > toggledIndex) {
							const match = el.style.transform.match(/translateY\((.+?)px\)/)
							const newStart = match ? parseFloat(match[1]) : 0
							el.animate(
								[{ transform: `translateY(${newStart - delta}px)` }, { transform: `translateY(${newStart}px)` }],
								{ duration: 200, easing: 'ease-out' }
							)
						}
					}
				}
			})
		}
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
						data-vkey={virtualItem.key}
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
