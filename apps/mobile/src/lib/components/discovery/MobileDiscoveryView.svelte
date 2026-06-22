<script lang="ts">
	import { onMount, tick } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import { discoveryStore, sortedReleases, isDiscoveryLoading } from '$shared/stores/discovery'
	import { previewInfo, previewLoadingReleaseId } from '$shared/stores/player'
	import { mobileUIStore, scrollTargetReleaseId } from '$lib/stores/mobileUI'
	import { createVirtualList } from '$shared/utils/virtualizer.svelte'
	import MobileListItem from '$lib/components/common/MobileListItem.svelte'
	import MobileListSkeleton from '$lib/components/common/MobileListSkeleton.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// Real Discovery feed: artist/title + remote artwork; tapping a release opens its detail screen.
	// The list is VIRTUALIZED (shared `createVirtualList`, the same util the desktop feed uses) — only the
	// rows in view mount. This matters a lot on mobile: a large synced collection (thousands of releases)
	// rendered un-virtualized instantiates thousands of row components on the main thread, which blocks it
	// for a long time and freezes the whole UI, taps included (the bottom tab bar appeared "dead"). With
	// virtualization the render is O(viewport) and the thread stays responsive.
	onMount(() => {
		// This view remounts on every return to the Discovery tab, so only fetch when the feed is empty —
		// otherwise a tab switch would re-trigger the loading state and flash the cached releases.
		if (get(discoveryStore).releases.length === 0) discoveryStore.loadReleases()
	})

	// This view owns its own scroll container (the shell frame is overflow-hidden); the virtualizer observes
	// it. `releases` is a plain reactive snapshot so the virtualizer's option closures track the feed.
	let scrollEl = $state<HTMLElement | null>(null)
	const releases = $derived($sortedReleases)

	// Fixed row height: MobileListItem is `min-h-[44px]` with `py-2` around a 48px (h-12) artwork ≈ 64px.
	const ROW_HEIGHT = 64

	const virtualList = createVirtualList({
		count: () => releases.length,
		getScrollElement: () => scrollEl,
		estimateSize: () => () => ROW_HEIGHT,
		overscan: 8,
		getItemKey: (index) => releases[index]?.id ?? index,
	})

	// Background "scroll to release" (locate from the expanded player): the target row may not be mounted,
	// so an element ref + scrollIntoView won't work — center it via the virtualizer, then clear the
	// one-shot target.
	$effect(() => {
		const id = $scrollTargetReleaseId
		if (!id || !scrollEl) return
		const index = releases.findIndex((r) => r.id === id)
		if (index >= 0) {
			tick().then(() => virtualList.scrollToIndex(index, { align: 'center' }))
		}
		mobileUIStore.consumeScrollTarget()
	})

	// --- Scroll-position persistence ----------------------------------------------------------------
	// The shell remounts this view on every return to the Discovery tab (`{#key activeTab}`), recreating
	// the scroll container — so the offset would reset to the top, losing the user's place after a long
	// scroll. We save it to the store as the user scrolls and restore it on mount.

	// Snapshot both at mount, BEFORE any effect runs. `skipScrollRestore`: when we arrive via "locate"
	// (the locate effect above consumes `scrollTargetReleaseId` and drives its own scroll-to-release),
	// that scroll wins, so don't restore. `savedScrollTop`: read up front so an early scroll event at the
	// top can't clobber it before the restore effect reads it.
	const skipScrollRestore = get(mobileUIStore).scrollTargetReleaseId !== null
	const savedScrollTop = get(mobileUIStore).discoveryScrollTop

	// Coalesce saves to one write per frame — a fling fires `scroll` far faster than that.
	let scrollSaveRaf = 0
	function handleScroll() {
		if (scrollSaveRaf) return
		scrollSaveRaf = requestAnimationFrame(() => {
			scrollSaveRaf = 0
			if (scrollEl) mobileUIStore.setDiscoveryScrollTop(scrollEl.scrollTop)
		})
	}
	$effect(() => () => {
		if (scrollSaveRaf) cancelAnimationFrame(scrollSaveRaf)
	})

	// Restore once, after the virtualizer has measured (totalSize > 0) so the spacer is tall enough to
	// accept the offset — assigning before that clamps to ~0. tick() lets the spacer's height land in the
	// DOM first; the browser then clamps the assignment if the feed has since shrunk.
	let didRestoreScroll = false
	$effect(() => {
		if (didRestoreScroll || skipScrollRestore || savedScrollTop <= 0 || !scrollEl) return
		if (virtualList.totalSize === 0) return
		didRestoreScroll = true
		const el = scrollEl
		tick().then(() => (el.scrollTop = savedScrollTop))
	})
</script>

<!-- Trailing padding lets the feed scroll *under* the floating mini-player (visible, blurred, through its
     glass) while the last release still clears it. The shell publishes the inset; 0 with no preview. -->
<div
	bind:this={scrollEl}
	onscroll={handleScroll}
	class="h-full overflow-x-hidden overflow-y-auto"
	style="padding-bottom: var(--mini-player-inset, 0px)"
>
	{#if $isDiscoveryLoading && releases.length === 0}
		<div role="status" aria-label={$translate('common.loading')}>
			<MobileListSkeleton />
		</div>
	{:else if releases.length === 0}
		<div class="flex h-full items-center justify-center text-sm text-text-secondary">
			{$translate('discovery.noReleasesYet')}
		</div>
	{:else}
		<!-- Spacer sized to the full virtual height; only the visible rows are absolutely positioned in it. -->
		<div style="height: {virtualList.totalSize}px; position: relative;">
			{#each virtualList.virtualItems as virtualItem (virtualItem.key)}
				{@const release = releases[virtualItem.index]}
				<div
					style="position: absolute; top: 0; left: 0; width: 100%; height: {virtualItem.size}px; overflow: hidden; transform: translateY({virtualItem.start}px);"
				>
					<MobileListItem
						onclick={() => mobileUIStore.openDetail(release.id)}
						selected={$previewInfo?.releaseId === release.id}
						ariaLabel={`${release.artist ?? $translate('common.unknownArtist')} — ${release.title ?? $translate('common.untitled')}`}
					>
						{#snippet leading()}
							{#if release.artwork_url}
								<img src={release.artwork_url} alt="" class="h-12 w-12 rounded object-cover" loading="lazy" />
							{:else}
								<div class="flex h-12 w-12 items-center justify-center rounded bg-surface-2 text-text-tertiary">
									<svg viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
										<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
									</svg>
								</div>
							{/if}
						{/snippet}
						<div class="flex min-w-0 flex-col">
							<span class="truncate text-sm font-medium text-text-primary">
								{release.title ?? $translate('common.untitled')}
							</span>
							<span class="truncate text-xs text-text-secondary">
								{release.artist ?? $translate('common.unknownArtist')}
							</span>
						</div>
						{#snippet trailing()}
							{#if $previewLoadingReleaseId === release.id}
								<Spinner class="h-4 w-4" />
							{:else}
								<!-- Chevron: signals the row opens its detail screen (no hover affordance on touch). -->
								<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M9 18l6-6-6-6" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							{/if}
						{/snippet}
					</MobileListItem>
				</div>
			{/each}
		</div>
	{/if}
</div>
