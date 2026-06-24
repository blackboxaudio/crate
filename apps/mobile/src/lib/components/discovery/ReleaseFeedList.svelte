<script lang="ts">
	import { tick } from 'svelte'
	import type { Snippet } from 'svelte'
	import type { DiscoveryRelease } from '$shared/types'
	import { createVirtualList } from '$shared/utils/virtualizer.svelte'

	// Shared VIRTUALIZED release list used by BOTH the Discovery feed and the playlist detail. Only the
	// rows in view mount, so a large synced collection (thousands of releases) stays responsive — the
	// playlist detail used to render every row, which froze the UI on big playlists.
	//
	// The virtualizer must own the scroll element (it builds a ResizeObserver on it), so this component
	// owns the scroll container and the parent reaches in only through the exported `scrollToIndex`
	// (locate) and the `onScroll` callback (persistence). Callers supply the row via a snippet so each
	// owns its own ReleaseCard props/context, plus optional `leading` (scrolls above the list) and
	// `empty` (shown when there are no rows) snippets.
	type Props = {
		releases: DiscoveryRelease[]
		row: Snippet<[{ release: DiscoveryRelease; index: number }]>
		rowHeight?: number
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
		overscan?: number
		class?: string
	}
	let {
		releases,
		row,
		rowHeight = 72,
		leading,
		empty,
		initialScrollTop = 0,
		skipScrollRestore = false,
		onScroll,
		scrollLocked = false,
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

<div
	bind:this={scrollEl}
	onscroll={handleScroll}
	class="min-h-0 flex-1 overflow-x-hidden {scrollLocked ? 'overflow-y-hidden' : 'overflow-y-auto'} {className}"
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
