<script lang="ts">
	import { onMount, onDestroy, tick } from 'svelte'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore, isDiscoveryLoading } from '$shared/stores/discovery'
	import { followStore } from '$shared/stores/follow'
	import {
		mobileUIStore,
		scrollTargetReleaseId,
		mobileDisplayedReleases,
		scrollTopNonce,
		discoveryViewMode,
		openRowId,
		DISCOVERY_ROW_HEIGHT,
		type ScrollGeom,
	} from '$lib/stores/mobileUI'
	import MobileListSkeleton from '$lib/components/common/MobileListSkeleton.svelte'
	import { pendingReleases } from '$lib/stores/pendingReleases'
	import DiscoveryToolbar from './DiscoveryToolbar.svelte'
	import ReleaseFeedList from './ReleaseFeedList.svelte'
	import ReleaseCard from './ReleaseCard.svelte'
	import ReleaseGridTile from './ReleaseGridTile.svelte'
	import PendingReleaseCard from './PendingReleaseCard.svelte'
	import AddReleaseModal from './AddReleaseModal.svelte'

	// Real Discovery feed: a search/sort/filter toolbar over a VIRTUALIZED list of release cards
	// (`ReleaseFeedList`, the shared list the playlist detail also uses). Only the rows in view mount, so a
	// large synced collection (thousands of releases) stays responsive — un-virtualized, thousands of row
	// components on the main thread would freeze the UI (taps included). Two layouts share the one
	// virtualizer: classic list rows, and a 3-column artwork grid whose virtual "row" is a chunk of three
	// releases (`gridRows`) with a taller rowHeight. The list must REMOUNT on a layout switch ({#key mode})
	// because the virtualizer captures rowHeight at creation.
	onMount(() => {
		// This view remounts on every return to the Discovery tab, so only fetch when the feed is empty —
		// otherwise a tab switch would re-trigger the loading state and flash the cached releases.
		if (get(discoveryStore).releases.length === 0) discoveryStore.loadReleases()
	})

	// `releases` is the displayed (search + sort + tag/downloaded-filtered) list the virtualizer renders —
	// the shared `mobileDisplayedReleases`, the very same list the playback queue captures, so the feed and
	// what shuffle/auto-advance span never drift. `totalReleases` is the raw loaded count, used to tell
	// "nothing added yet" (show the CTA) from "filters hid everything".
	const releases = $derived($mobileDisplayedReleases)
	const totalReleases = $derived($discoveryStore.releases.length)

	// --- Grid geometry --------------------------------------------------------------------------------
	const COLS = 3
	const mode = $derived($discoveryViewMode)
	// Container width drives the tile size: (width − 2×12px padding − 2×8px gaps) / 3, plus ~42px for the
	// two text lines + spacing. Falls back to a sane height for the pre-measure frame (width = 0).
	let listWidth = $state(0)
	const gridRowH = $derived(listWidth > 0 ? Math.round((listWidth - 24 - 16) / COLS) + 42 : 166)
	const geom = $derived<ScrollGeom>(
		mode === 'grid' ? { rowHeight: gridRowH, cols: COLS } : { rowHeight: DISCOVERY_ROW_HEIGHT, cols: 1 }
	)

	type GridRow = { id: string; items: DiscoveryRelease[] }
	const gridRows = $derived.by(() => {
		if (mode !== 'grid') return [] as GridRow[]
		const rows: GridRow[] = []
		for (let i = 0; i < releases.length; i += COLS) {
			const items = releases.slice(i, i + COLS)
			rows.push({ id: items[0].id, items })
		}
		return rows
	})

	// Bound feed-list instance, so the locate / scroll-to-top effects can drive the virtualizer's scroll.
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	let feedList = $state<any>(null)

	// Snapshot at mount, BEFORE any effect runs. `skipScrollRestore`: when we arrive via "locate" (the
	// locate effect below drives its own scroll-to-release), that scroll wins, so don't restore.
	// `savedScrollTop`: read up front so an early scroll event at the top can't clobber it before the
	// feed list's restore reads it. `bootAnchor`: the one-shot restart anchor (see mobileUI.ts) — while
	// it's live, the feed list's raw-pixel restore is suppressed and the anchor effect below drives the
	// restore instead (a deep offset only becomes reachable as the release pages stream in).
	const skipScrollRestore = get(mobileUIStore).scrollTargetReleaseId !== null
	const savedScrollTop = get(mobileUIStore).discoveryScrollTop
	const bootAnchor = get(mobileUIStore).discoveryRestoreAnchor

	// The scroll offset the (re)mounted list restores to. Starts at the persisted offset; a list↔grid
	// toggle converts it (below) so the same releases stay in view across the {#key} remount. The view
	// mode itself is persisted, so a boot restore always replays under the geometry it was saved with.
	let currentScrollTop = $state(savedScrollTop)

	// Intentional initial snapshot: the effect below compares against it and keeps it current.
	// svelte-ignore state_referenced_locally
	let prevMode = mode
	$effect.pre(() => {
		const m = mode
		if (m === prevMode) return
		const oldGeom: ScrollGeom =
			m === 'grid' ? { rowHeight: DISCOVERY_ROW_HEIGHT, cols: 1 } : { rowHeight: gridRowH, cols: COLS }
		const newGeom: ScrollGeom =
			m === 'grid' ? { rowHeight: gridRowH, cols: COLS } : { rowHeight: DISCOVERY_ROW_HEIGHT, cols: 1 }
		const topIndex = Math.floor(currentScrollTop / oldGeom.rowHeight) * oldGeom.cols
		currentScrollTop = Math.floor(topIndex / newGeom.cols) * newGeom.rowHeight
		mobileUIStore.setDiscoveryScrollTop(currentScrollTop, newGeom)
		prevMode = m
	})

	// Restart scroll restore: anchor by release ID so new releases shifting the list (or a partial load)
	// can't land us on the wrong rows. Re-runs as pages stream in; the moment the anchor release appears,
	// scroll to its row (rows are uniform, so the spacer already spans it). If the whole load finishes
	// without it (release deleted since last session), fall back to the raw offset, clamped by the browser.
	// Known imprecision: the `leading` pending-releases block sits above the virtual rows, so the anchor
	// math is off by its height while offline adds are pending — same class of error as the locate path.
	let anchorDone = bootAnchor === null || skipScrollRestore
	$effect(() => {
		const anchor = bootAnchor
		if (anchorDone || !feedList || !anchor) return
		const index = releases.findIndex((r) => r.id === anchor.releaseId)
		if (index >= 0) {
			anchorDone = true
			mobileUIStore.consumeDiscoveryAnchor()
			const rowIndex = Math.floor(index / geom.cols)
			tick().then(() => feedList?.scrollToOffset(rowIndex * geom.rowHeight + anchor.offset))
		} else if (!$isDiscoveryLoading && totalReleases > 0) {
			anchorDone = true
			mobileUIStore.consumeDiscoveryAnchor()
			tick().then(() => feedList?.scrollToOffset(savedScrollTop))
		}
	})

	// Background "scroll to release" (locate from the expanded player): the target row may not be mounted,
	// so center it via the virtualizer (in grid mode, the virtual row holding the tile), then clear the
	// one-shot target.
	$effect(() => {
		const id = $scrollTargetReleaseId
		if (!id || !feedList) return
		const index = releases.findIndex((r) => r.id === id)
		if (index >= 0) {
			const rowIndex = Math.floor(index / geom.cols)
			tick().then(() => feedList?.scrollToIndex(rowIndex, { align: 'center' }))
		}
		mobileUIStore.consumeScrollTarget()
	})

	// A scroll closes any revealed swipe row and stages the offset for the debounced restart persistence.
	// Both store touches are avoided on the per-frame path: `setOpenRow` only when a row is actually open
	// (an update() notifies every derived selector even when nothing changed), and the offset is STAGED
	// (`stageDiscoveryScrollTop`, no store write) rather than set — the reactive `discoveryScrollTop` is
	// only read at mount, so it's committed once at unmount (below) for the tab-return remount to restore
	// from. The live geometry rides along so the release-ID anchor maps offsets correctly in grid mode.
	function handleScroll(scrollTop: number) {
		currentScrollTop = scrollTop
		if ($openRowId !== null) mobileUIStore.setOpenRow(null)
		mobileUIStore.stageDiscoveryScrollTop(scrollTop, geom)
	}

	onDestroy(() => mobileUIStore.setDiscoveryScrollTop(currentScrollTop, geom))

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

<div class="relative flex h-full flex-col" bind:clientWidth={listWidth}>
	<!-- Pinned search/sort/filter toolbar (the section title lives in the fixed top bar). It sits ABOVE the
	     feed's scroll container rather than scrolling with it, so a pull-to-refresh opens its gap in the space
	     *beneath* the toolbar — the spinner reads as coming out from under the search bar. The pending block
	     stays inside the list as its `leading` so it scrolls with the rows. ReleaseFeedList owns the scroll
	     container and shows the loading/empty states when nothing is displayed. -->
	<DiscoveryToolbar />
	{#key mode}
		{#if mode === 'grid'}
			<ReleaseFeedList
				bind:this={feedList}
				releases={gridRows}
				rowHeight={gridRowH}
				initialScrollTop={currentScrollTop}
				skipScrollRestore={skipScrollRestore || bootAnchor !== null}
				onScroll={handleScroll}
				onRefresh={refreshFollowed}
				leading={pendingBlock}
				empty={emptyState}
				row={gridRow}
			/>
		{:else}
			<ReleaseFeedList
				bind:this={feedList}
				{releases}
				rowHeight={DISCOVERY_ROW_HEIGHT}
				initialScrollTop={currentScrollTop}
				skipScrollRestore={skipScrollRestore || bootAnchor !== null}
				onScroll={handleScroll}
				onRefresh={refreshFollowed}
				leading={pendingBlock}
				empty={emptyState}
				row={releaseRow}
			/>
		{/if}
	{/key}
</div>

{#snippet releaseRow({ release }: { release: DiscoveryRelease })}
	<ReleaseCard {release} />
{/snippet}

{#snippet gridRow({ release: chunk }: { release: GridRow })}
	<!-- One virtual row = up to three tiles; short rows keep their tiles left-aligned via spacers. -->
	<div class="flex h-full items-start gap-2 px-3 pt-2">
		{#each chunk.items as item (item.id)}
			<div class="min-w-0 flex-1"><ReleaseGridTile release={item} /></div>
		{/each}
		{#each Array(COLS - chunk.items.length) as _, i (i)}
			<div class="min-w-0 flex-1" aria-hidden="true"></div>
		{/each}
	</div>
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
