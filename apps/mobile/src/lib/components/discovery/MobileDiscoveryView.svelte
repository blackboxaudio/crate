<script lang="ts">
	import { onMount, tick } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import { discoveryStore, isDiscoveryLoading } from '$shared/stores/discovery'
	import { mobileUIStore, scrollTargetReleaseId, mobileDisplayedReleases } from '$lib/stores/mobileUI'
	import { createVirtualList } from '$shared/utils/virtualizer.svelte'
	import MobileListSkeleton from '$lib/components/common/MobileListSkeleton.svelte'
	import DiscoveryToolbar from './DiscoveryToolbar.svelte'
	import ReleaseCard from './ReleaseCard.svelte'
	import AddReleaseModal from './AddReleaseModal.svelte'

	// Real Discovery feed: a search/sort/filter toolbar over a VIRTUALIZED list of release cards. The list
	// uses the shared `createVirtualList` (the same util the desktop feed uses) — only the rows in view
	// mount. This matters a lot on mobile: a large synced collection (thousands of releases) rendered
	// un-virtualized instantiates thousands of row components on the main thread, freezing the UI (taps
	// included). With virtualization the render is O(viewport) and the thread stays responsive.
	onMount(() => {
		// This view remounts on every return to the Discovery tab, so only fetch when the feed is empty —
		// otherwise a tab switch would re-trigger the loading state and flash the cached releases.
		if (get(discoveryStore).releases.length === 0) discoveryStore.loadReleases()
	})

	// `releases` is the displayed (search + sort + tag-filtered) list the virtualizer renders — the shared
	// `mobileDisplayedReleases`, the very same list the playback queue captures, so the feed and what
	// shuffle/auto-advance span never drift. `totalReleases` is the raw loaded count, used to tell "nothing
	// added yet" (show the CTA) from "filters hid everything".
	const releases = $derived($mobileDisplayedReleases)
	const totalReleases = $derived($discoveryStore.releases.length)

	// This view owns its own scroll container (the shell frame is overflow-hidden); the virtualizer observes
	// it. `releases` is a plain reactive snapshot so the virtualizer's option closures track the feed.
	let scrollEl = $state<HTMLElement | null>(null)

	// Fixed row height: ReleaseCard is artwork (48px) beside a 3-line title/artist/label column ≈ 72px.
	const ROW_HEIGHT = 72

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
		// A scroll closes any revealed swipe-to-delete row (no-op when none is open).
		mobileUIStore.setOpenRow(null)
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

<div class="flex h-full flex-col">
	<DiscoveryToolbar />

	<!-- Scroll container lives BELOW the toolbar (the toolbar must stay outside it so the virtualizer's
	     rect math / scroll restore stay correct). Trailing padding lets the feed scroll *under* the floating
	     mini-player while the last release still clears it; the shell publishes the inset (0 with no preview). -->
	<div
		bind:this={scrollEl}
		onscroll={handleScroll}
		class="min-h-0 flex-1 overflow-x-hidden overflow-y-auto"
		style="padding-bottom: var(--mini-player-inset, 0px)"
	>
		{#if $isDiscoveryLoading && totalReleases === 0}
			<div role="status" aria-label={$translate('common.loading')}>
				<MobileListSkeleton />
			</div>
		{:else if totalReleases === 0}
			<!-- Truly empty: the standalone "add your first discovery" CTA (never a sync prompt). -->
			<div class="flex h-full flex-col items-center justify-center gap-5 px-8 text-center">
				<div class="flex h-16 w-16 items-center justify-center rounded-full bg-surface-2 text-text-tertiary">
					<svg viewBox="0 0 24 24" class="h-8 w-8" fill="currentColor">
						<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
					</svg>
				</div>
				<div class="space-y-1">
					<p class="text-base font-semibold text-text-primary">{$translate('discovery.noReleasesYet')}</p>
					<p class="text-sm text-text-secondary">{$translate('discovery.mobileAddHint')}</p>
				</div>
				<button
					type="button"
					class="inline-flex items-center gap-1.5 rounded-lg bg-brand-primary px-4 py-2.5 text-sm font-semibold text-white active:opacity-90"
					onclick={mobileUIStore.openAddRelease}
				>
					<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M12 5v14M5 12h14" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
					{$translate('discovery.addRelease')}
				</button>
			</div>
		{:else if releases.length === 0}
			<!-- Releases exist, but the active search/filter hid them all — not a reason to show the add CTA. -->
			<div class="flex h-full items-center justify-center px-8 text-center text-sm text-text-secondary">
				{$translate('discovery.noResults')}
			</div>
		{:else}
			<!-- Spacer sized to the full virtual height; only the visible rows are absolutely positioned in it. -->
			<div style="height: {virtualList.totalSize}px; position: relative;">
				{#each virtualList.virtualItems as virtualItem (virtualItem.key)}
					{@const release = releases[virtualItem.index]}
					{#if release}
						<div
							style="position: absolute; top: 0; left: 0; width: 100%; height: {virtualItem.size}px; transform: translateY({virtualItem.start}px);"
						>
							<ReleaseCard {release} />
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
</div>

<!-- Add-release entry point (placeholder body; the functional flow is issue #56). Reads its own open state. -->
<AddReleaseModal />
