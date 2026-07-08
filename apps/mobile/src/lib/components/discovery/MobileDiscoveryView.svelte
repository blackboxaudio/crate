<script lang="ts">
	import { onMount, tick } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import { discoveryStore, isDiscoveryLoading } from '$shared/stores/discovery'
	import { followStore } from '$shared/stores/follow'
	import { mobileUIStore, scrollTargetReleaseId, mobileDisplayedReleases, scrollTopNonce } from '$lib/stores/mobileUI'
	import MobileListSkeleton from '$lib/components/common/MobileListSkeleton.svelte'
	import { pendingReleases } from '$lib/stores/pendingReleases'
	import DiscoveryToolbar from './DiscoveryToolbar.svelte'
	import ReleaseFeedList from './ReleaseFeedList.svelte'
	import ReleaseCard from './ReleaseCard.svelte'
	import PendingReleaseCard from './PendingReleaseCard.svelte'
	import AddReleaseModal from './AddReleaseModal.svelte'

	// Real Discovery feed: a search/sort/filter toolbar over a VIRTUALIZED list of release cards
	// (`ReleaseFeedList`, the shared list the playlist detail also uses). Only the rows in view mount, so a
	// large synced collection (thousands of releases) stays responsive — un-virtualized, thousands of row
	// components on the main thread would freeze the UI (taps included).
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

	// Bound feed-list instance, so the locate / scroll-to-top effects can drive the virtualizer's scroll.
	let feedList = $state<ReturnType<typeof ReleaseFeedList> | null>(null)

	// Snapshot at mount, BEFORE any effect runs. `skipScrollRestore`: when we arrive via "locate" (the
	// locate effect below drives its own scroll-to-release), that scroll wins, so don't restore.
	// `savedScrollTop`: read up front so an early scroll event at the top can't clobber it before the
	// feed list's restore reads it.
	const skipScrollRestore = get(mobileUIStore).scrollTargetReleaseId !== null
	const savedScrollTop = get(mobileUIStore).discoveryScrollTop

	// Background "scroll to release" (locate from the expanded player): the target row may not be mounted,
	// so center it via the virtualizer, then clear the one-shot target.
	$effect(() => {
		const id = $scrollTargetReleaseId
		if (!id || !feedList) return
		const index = releases.findIndex((r) => r.id === id)
		if (index >= 0) {
			tick().then(() => feedList?.scrollToIndex(index, { align: 'center' }))
		}
		mobileUIStore.consumeScrollTarget()
	})

	// A scroll closes any revealed swipe-to-delete row (no-op when none is open) and persists the offset — the
	// shell remounts this view on tab return, so the feed list restores from the store on mount.
	function handleScroll(scrollTop: number) {
		mobileUIStore.setOpenRow(null)
		mobileUIStore.setDiscoveryScrollTop(scrollTop)
	}

	// iOS "re-tap the active tab to scroll to top": the tab bar bumps `scrollTopNonce` when the active tab is
	// re-tapped with nothing to pop. Ignore the initial value so a normal mount (or scroll restore) doesn't
	// yank the feed to the top.
	let seenScrollNonce = get(mobileUIStore).scrollTopNonce
	$effect(() => {
		const n = $scrollTopNonce
		if (n === seenScrollNonce) return
		seenScrollNonce = n
		feedList?.scrollToTop()
	})

	// Pull-to-refresh: check every followed source for new releases, then reload the feed so any freshly
	// surfaced (is_new) releases appear inline. checkAll also refreshes the Following tab's counts.
	async function refreshFollowed() {
		await followStore.checkAll()
		await discoveryStore.loadReleases()
	}
</script>

<div class="relative flex h-full flex-col">
	<!-- Pinned search/sort/filter toolbar (the section title lives in the fixed top bar). It sits ABOVE the
	     feed's scroll container rather than scrolling with it, so a pull-to-refresh opens its gap in the space
	     *beneath* the toolbar — the spinner reads as coming out from under the search bar. The pending block
	     stays inside the list as its `leading` so it scrolls with the rows. ReleaseFeedList owns the scroll
	     container and shows the loading/empty states when nothing is displayed. -->
	<DiscoveryToolbar />
	<ReleaseFeedList
		bind:this={feedList}
		{releases}
		initialScrollTop={savedScrollTop}
		{skipScrollRestore}
		onScroll={handleScroll}
		onRefresh={refreshFollowed}
		leading={pendingBlock}
		empty={emptyState}
		row={releaseRow}
	/>
</div>

{#snippet releaseRow({ release })}
	<ReleaseCard {release} />
{/snippet}

{#snippet pendingBlock()}
	{#if $pendingReleases.length > 0}
		<div class="border-b border-stroke-subtle">
			{#each $pendingReleases as pending (pending.id)}
				<PendingReleaseCard {pending} />
			{/each}
		</div>
	{/if}
{/snippet}

{#snippet emptyState()}
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
	{:else}
		<!-- Releases exist, but the active search/filter hid them all — not a reason to show the add CTA. -->
		<div class="flex h-full items-center justify-center px-8 text-center text-sm text-text-secondary">
			{$translate('discovery.noResults')}
		</div>
	{/if}
{/snippet}

<!-- Add-release modal. Reads its own open state. -->
<AddReleaseModal />
