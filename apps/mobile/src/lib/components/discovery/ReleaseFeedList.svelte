<script lang="ts" generics="T extends { id: string }">
	import { tick } from 'svelte'
	import type { Snippet } from 'svelte'
	import { createVirtualList } from '$shared/utils/virtualizer.svelte'
	import PullToRefresh from '$lib/components/common/PullToRefresh.svelte'

	// Shared VIRTUALIZED release list used by BOTH the Discovery feed and the playlist detail. Only the
	// rows in view mount, so a large synced collection (thousands of releases) stays responsive — the
	// playlist detail used to render every row, which froze the UI on big playlists.
	//
	// Generic over the item (`T extends { id: string }`): the item is normally a DiscoveryRelease, but
	// the feed's grid mode passes CHUNKED rows (three releases per virtual row) through the exact same
	// machinery — PullToRefresh, scroll restore, the exported scroll methods — with a taller rowHeight.
	// NOTE: `rowHeight` is captured by the virtualizer's estimate closure at creation and is NOT
	// reactive — hosts that change it (the feed's list↔grid toggle) must remount (`{#key mode}`).
	//
	// The virtualizer must own the scroll element (it builds a ResizeObserver on it), so this component
	// owns the scroll container and the parent reaches in only through the exported `scrollToIndex`
	// (locate) and the `onScroll` callback (persistence). Callers supply the row via a snippet so each
	// owns its own ReleaseCard props/context, plus optional `leading` (scrolls above the list) and `empty`
	// (shown when there are no rows) snippets. `topInset` reserves resting top padding for a glass toolbar
	// the host overlays on the list (so rows scroll behind it); it's applied via PullToRefresh, which owns
	// the scroll element's padding-top — so it takes effect only when `onRefresh` is provided (the feed's case).
	type Props = {
		releases: T[]
		row: Snippet<[{ release: T; index: number }]>
		rowHeight?: number
		topInset?: number
		leading?: Snippet
		empty?: Snippet
		/** One-shot scroll offset to restore once the virtualizer has measured (feed only). */
		initialScrollTop?: number
		/** Skip the restore (the feed sets this when arriving via "locate", which drives its own scroll). */
		skipScrollRestore?: boolean
		/** rAF-coalesced; fires the live scrollTop so the feed can persist it + close swipe rows. */
		onScroll?: (scrollTop: number) => void
		/** Lock scrolling (the playlist passes the Drawer's `animating` flag during the slide). */
		scrollLocked?: boolean
		/** When set, a pull-down at the top of the list runs this (the feed uses it to check follows). */
		onRefresh?: () => Promise<void> | void
		overscan?: number
		class?: string
	}
	let {
		releases,
		row,
		rowHeight = 72,
		topInset = 0,
		leading,
		empty,
		initialScrollTop = 0,
		skipScrollRestore = false,
		onScroll,
		scrollLocked = false,
		onRefresh,
		overscan = 8,
		class: className = '',
	}: Props = $props()

	let scrollEl = $state<HTMLElement | null>(null)

	const virtualList = createVirtualList({
		count: () => releases.length,
		getScrollElement: () => scrollEl,
		estimateSize: () => () => rowHeight,
		overscan,
		getItemKey: (index) => releases[index]?.id ?? index,
	})

	// Locate: the parent can't scrollIntoView a row that isn't mounted, so it routes through the
	// virtualizer. Exposed via bind:this.
	export function scrollToIndex(index: number, opts?: { align?: 'start' | 'center' | 'end' | 'auto' }) {
		virtualList.scrollToIndex(index, opts)
	}

	// Smooth scroll the feed back to the top (iOS "re-tap the active tab to scroll to top"). Exposed via
	// bind:this so the host can drive it from the tab-bar re-tap signal.
	export function scrollToTop(smooth = true) {
		scrollEl?.scrollTo({ top: 0, behavior: smooth ? 'smooth' : 'auto' })
	}

	// Jump to an absolute offset via the virtualizer (the feed's boot restore: anchor scroll + pixel fallback).
	export function scrollToOffset(offset: number) {
		virtualList.scrollToOffset(offset)
	}

	// Coalesce scroll callbacks to one per frame — a fling fires `scroll` far faster than that.
	let scrollRaf = 0
	function handleScroll() {
		if (!onScroll) return
		if (scrollRaf) return
		scrollRaf = requestAnimationFrame(() => {
			scrollRaf = 0
			if (scrollEl) onScroll(scrollEl.scrollTop)
		})
	}
	$effect(() => () => {
		if (scrollRaf) cancelAnimationFrame(scrollRaf)
	})

	// One-shot scroll restore: only after the virtualizer has measured (totalSize > 0) so the spacer is
	// tall enough to accept the offset; tick() lets the spacer's height land in the DOM first.
	let didRestore = false
	$effect(() => {
		if (didRestore || skipScrollRestore || initialScrollTop <= 0 || !scrollEl) return
		if (virtualList.totalSize === 0) return
		didRestore = true
		const el = scrollEl
		tick().then(() => (el.scrollTop = initialScrollTop))
	})
</script>

<!-- Relative wrapper for the pull-to-refresh spinner. It fills the same flex slot the scroll element used
     to; the scroll element itself keeps owning the scroll (a pull pushes its content down via padding). -->
<div class="relative flex min-h-0 flex-1 flex-col">
	{#if onRefresh}
		<PullToRefresh {scrollEl} {onRefresh} {topInset} />
	{/if}
	<!-- With `onRefresh`, a pull-down past the top is the refresh gesture, so leave the rubber-band. Without
	     it there's nothing to reveal, so kill the top/bottom overscroll bounce (`overscroll-y-none`) — the
	     list shouldn't budge when it can't scroll further. -->
	<div
		bind:this={scrollEl}
		onscroll={handleScroll}
		class="min-h-0 flex-1 overflow-x-hidden {scrollLocked ? 'overflow-y-hidden' : 'overflow-y-auto'} {onRefresh
			? ''
			: 'overscroll-y-none'} {className}"
		style="padding-bottom: var(--mini-player-inset, 0px)"
	>
		{#if leading}{@render leading()}{/if}

		{#if releases.length === 0}
			{#if empty}{@render empty()}{/if}
		{:else}
			<!-- Spacer sized to the full virtual height; only the visible rows are absolutely positioned in it. -->
			<div style="height: {virtualList.totalSize}px; position: relative;">
				{#each virtualList.virtualItems as virtualItem (virtualItem.key)}
					{@const release = releases[virtualItem.index]}
					{#if release}
						<div
							style="position: absolute; top: 0; left: 0; width: 100%; height: {virtualItem.size}px; transform: translateY({virtualItem.start}px);"
						>
							{@render row({ release, index: virtualItem.index })}
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
</div>
