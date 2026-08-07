<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease, DiscoverySortConfig, DiscoverySortField, Playlist } from '$shared/types'
	import { playlistsStore } from '$shared/stores/playlists'
	import { getSmartPlaylistReleases } from '$shared/api/playlists'
	import { discoveryPlaylistStore, discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
	import { sortDiscoveryReleases } from '$shared/utils/sorting'
	import {
		mobileUIStore,
		playlistReorderMode,
		selectMode,
		selectedReleaseIds,
		overlayPopNonce,
		detailReleaseId,
	} from '$lib/stores/mobileUI'
	import { fullyCachedIds } from '$lib/stores/offlineCache'
	import { ownedReleaseIds } from '$shared/stores/collection'
	import {
		applyViewFilter,
		emptyViewFilter,
		hasActiveViewFilter,
		RELEASE_SORT_OPTIONS,
		type SortOption,
	} from '$lib/utils/listControls'
	import { confirmDialog } from '$lib/utils/dialog'
	import { refreshPlaylistCovers } from '$lib/stores/playlistCovers'
	import { registerBackLayer } from '$lib/androidBack'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import EmptyState from '$lib/components/common/EmptyState.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import ReleaseCard from '$lib/components/discovery/ReleaseCard.svelte'
	import ReleaseFeedList from '$lib/components/discovery/ReleaseFeedList.svelte'
	import ReleaseContextMenu from '$lib/components/discovery/ReleaseContextMenu.svelte'
	import ListControlsBar from '$lib/components/discovery/ListControlsBar.svelte'
	import SelectionBar from '$lib/components/discovery/SelectionBar.svelte'
	import PlaylistPickerSheet from './PlaylistPickerSheet.svelte'
	import SortableReleaseList from './SortableReleaseList.svelte'

	type Props = {
		playlist: Playlist
	}
	let { playlist }: Props = $props()

	let open = $state(true)
	// Boot-restored (this overlay was open when the app was last killed): appear in place, no slide-in.
	const enterInstant = mobileUIStore.consumeBootRestoredOverlay('playlist')
	let loading = $state(false)
	let releases = $derived($discoveryPlaylistReleases)
	let pickerOpen = $state(false)
	let pickerReleaseIds = $state<string[]>([])

	const isReorderMode = $derived($playlistReorderMode)
	const isSelectMode = $derived($selectMode)

	// View-level (session-local) sort + filter over the playlist's releases. Sort is
	// non-destructive — null = the playlist's own order (junction positions, or smart-rule
	// order) — and never writes positions. Reset naturally on close (the component unmounts).
	let viewSort = $state<DiscoverySortConfig | null>(null)
	let viewFilter = $state(emptyViewFilter())
	const filtered = $derived(applyViewFilter(releases, viewFilter, $fullyCachedIds, $ownedReleaseIds))
	const displayed = $derived(viewSort ? sortDiscoveryReleases(filtered, viewSort) : filtered)
	// Manual reorder writes junction positions, which is only meaningful while the user is looking
	// at the unfiltered natural order — a sorted/filtered list would persist a misleading result.
	const canReorder = $derived(!playlist.is_smart && viewSort === null && !hasActiveViewFilter(viewFilter))

	// "Playlist order" leads the sort options as the directionless natural-order choice.
	const sortOptions: SortOption[] = [
		{ field: 'playlist_order', labelKey: 'playlists.playlistOrder', defaultDir: 'asc', directionless: true },
		...RELEASE_SORT_OPTIONS,
	]

	function onSelectSort(field: string, direction: 'asc' | 'desc') {
		viewSort = field === 'playlist_order' ? null : { field: field as DiscoverySortField, direction }
	}

	// Publish the displayed list so playback started from this view queues exactly what's on
	// screen (sorted/filtered); cleared when the view unmounts or before each re-publish.
	$effect(() => {
		mobileUIStore.setOverlayReleases(displayed)
		return () => mobileUIStore.setOverlayReleases(null)
	})

	// Android Back exits reorder mode before it closes this drawer (#62): reorder activates after
	// the drawer opened, so this registration naturally stacks above the drawer's own.
	$effect(() => {
		if (isReorderMode) return registerBackLayer(() => mobileUIStore.exitReorderMode())
	})

	$effect(() => {
		loadReleases()
	})

	async function loadReleases() {
		const cached = discoveryPlaylistStore.getCached(playlist.id)
		if (cached) {
			discoveryPlaylistStore.setReleases(cached)
			return
		}
		loading = true
		// Smart playlists hold no junction rows — their releases are computed from rules, fetched via a
		// separate command (mirrors desktop's `is_smart` branch). Without this, every smart playlist opened
		// empty because `get_playlist_releases` only reads `playlist_discovery_releases`.
		const fetched = playlist.is_smart
			? await getSmartPlaylistReleases(playlist.id).catch(() => [])
			: await playlistsStore.getPlaylistReleases(playlist.id)
		discoveryPlaylistStore.cacheAndSet(playlist.id, fetched)
		loading = false
	}

	function startClose() {
		open = false
		mobileUIStore.beginClosePlaylist()
	}

	// iOS "re-tap the active tab to pop to root": the tab bar bumps `overlayPopNonce`. Close on the bump, but
	// defer to the release detail when it's stacked on top (that closes first; a second tap then reaches here).
	let seenPopNonce = get(overlayPopNonce)
	$effect(() => {
		const n = $overlayPopNonce
		if (n === seenPopNonce) return
		seenPopNonce = n
		if (open && get(detailReleaseId) === null) startClose()
	})

	function onClosed() {
		discoveryPlaylistStore.clearReleases()
		mobileUIStore.closePlaylist()
	}

	async function handleReorder(releaseIds: string[]) {
		discoveryPlaylistStore.reorderInCache(playlist.id, releaseIds)
		await playlistsStore.reorderReleases(playlist.id, releaseIds)
		// Reorder can change which four releases lead the playlist → refresh its mosaic thumbnail.
		void refreshPlaylistCovers(playlist.id)
	}

	function openPickerForSingle(releaseId: string) {
		pickerReleaseIds = [releaseId]
		pickerOpen = true
	}

	function openPickerForSelection() {
		pickerReleaseIds = [...$selectedReleaseIds]
		pickerOpen = true
	}

	async function removeFromPlaylist(releaseId: string) {
		const t = get(translate)
		const ok = await confirmDialog(
			t('modals.confirm.removeDiscoveryReleasesFromPlaylistMessage', { values: { count: 1 } }),
			{
				title: t('modals.confirm.removeDiscoveryReleasesFromPlaylistTitle'),
				confirmLabel: t('common.remove'),
			}
		)
		if (!ok) return
		await playlistsStore.removeReleases(playlist.id, [releaseId])
		discoveryPlaylistStore.filterOutAndCache(playlist.id, [releaseId])
		void refreshPlaylistCovers(playlist.id)
	}

	async function batchRemoveFromPlaylist() {
		const ids = [...$selectedReleaseIds]
		const t = get(translate)
		const ok = await confirmDialog(
			t('modals.confirm.removeDiscoveryReleasesFromPlaylistMessage', { values: { count: ids.length } }),
			{
				title: t('modals.confirm.removeDiscoveryReleasesFromPlaylistTitle'),
				confirmLabel: t('common.remove'),
			}
		)
		if (!ok) return
		mobileUIStore.exitSelectMode()
		await playlistsStore.removeReleases(playlist.id, ids)
		discoveryPlaylistStore.filterOutAndCache(playlist.id, ids)
		void refreshPlaylistCovers(playlist.id)
	}
</script>

<Drawer
	{open}
	direction="right"
	onClose={startClose}
	{onClosed}
	{enterInstant}
	z={30}
	scrimZ={20}
	scrimDismiss={false}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={playlist.name}
	class="flex w-full flex-col bg-surface-0"
>
	{#snippet children({ animating })}
		<!-- Header. Owns the top safe-area inset and mirrors the fixed top bar's surface-1 + hairline so
		     drill-in headers read as the same app chrome. -->
		<div class="pt-safe border-b border-stroke-subtle bg-surface-1">
			<div class="flex items-center justify-between gap-1 px-2 py-2">
				<div class="flex min-w-0 items-center gap-1">
					<button
						type="button"
						class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
						aria-label={$translate('common.close')}
						onclick={startClose}
					>
						<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
						</svg>
					</button>
					<h1 class="truncate text-lg font-semibold text-text-primary">{playlist.name}</h1>
				</div>
				<!-- Reorder only applies to manual playlists in their natural, unfiltered order — a smart
				     playlist's order is rule-derived, and reordering a sorted/filtered view would persist
				     a misleading result. -->
				{#if canReorder}
					<div class="flex items-center gap-1">
						{#if isReorderMode}
							<button
								type="button"
								class="rounded-md px-3 py-2 text-sm font-medium text-brand-primary active:bg-surface-2"
								onclick={() => mobileUIStore.exitReorderMode()}
							>
								{$translate('common.done')}
							</button>
						{:else}
							<button
								type="button"
								class="flex h-10 w-10 items-center justify-center rounded-md text-text-secondary active:bg-surface-2"
								aria-label={$translate('queue.reorder')}
								onclick={() => mobileUIStore.toggleReorderMode()}
							>
								<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M7 15l5 5 5-5M7 9l5-5 5 5" stroke-linecap="round" stroke-linejoin="round" />
								</svg>
							</button>
						{/if}
					</div>
				{/if}
			</div>
		</div>

		<!-- Content. The list branch hands its scroll container to ReleaseFeedList (the same virtualized list
		     the Discovery feed uses), so it renders directly as the flex child — no outer scroll wrapper, which
		     would nest scrollers and double the mini-player inset. Loading / empty / reorder keep their own
		     scrollable wrapper; reorder stays non-virtualized because SortableReleaseList's drag math needs
		     every row present. -->
		{#if !loading && releases.length > 0 && !isReorderMode}
			<ListControlsBar
				{sortOptions}
				currentSort={viewSort ?? { field: 'playlist_order', direction: 'asc' }}
				{onSelectSort}
				filter={viewFilter}
				onFilterChange={(f) => (viewFilter = f)}
			/>
		{/if}

		{#if loading}
			<div class="flex flex-1 items-center justify-center py-12">
				<Spinner class="h-6 w-6 text-text-tertiary" />
			</div>
		{:else if releases.length === 0}
			<div class="flex-1 px-4 py-6">
				<EmptyState
					title={$translate('discovery.noReleasesYet')}
					hint={playlist.is_smart ? undefined : $translate('playlists.detailEmptyHint')}
				>
					{#snippet icon()}
						<svg class="h-8 w-8" viewBox="0 0 24 24" fill="currentColor">
							<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
						</svg>
					{/snippet}
				</EmptyState>
			</div>
		{:else if isReorderMode}
			<div
				class="min-h-0 flex-1 overflow-x-hidden {animating ? 'overflow-y-hidden' : 'overflow-y-auto'}"
				style="padding-bottom: var(--mini-player-inset, 0px)"
			>
				<SortableReleaseList {releases} onReorder={handleReorder} />
			</div>
		{:else if displayed.length === 0}
			<!-- The playlist has releases, but the active view filter hides them all. -->
			<div class="flex-1 px-4 py-8">
				<p class="text-center text-sm text-text-secondary">{$translate('discovery.noResults')}</p>
			</div>
		{:else}
			<ReleaseFeedList
				releases={displayed}
				scrollLocked={animating}
				row={releaseRow}
				onScroll={() => mobileUIStore.setOpenRow(null)}
			/>
		{/if}
	{/snippet}
</Drawer>

{#snippet releaseRow({ release }: { release: DiscoveryRelease })}
	<ReleaseCard {release} playlistId={playlist.id} context="playlist" />
{/snippet}

{#if isSelectMode}
	<SelectionBar
		playlistId={playlist.id}
		onRemoveFromPlaylist={playlist.is_smart ? undefined : batchRemoveFromPlaylist}
		onAddToPlaylist={openPickerForSelection}
	/>
{/if}

<!-- A smart playlist has no junction rows, so per-release reorder/remove don't apply — passing a null
     playlistId hides both menu items (its only internal use is that gate). -->
<ReleaseContextMenu
	context="playlist"
	{releases}
	playlistId={playlist.is_smart ? null : playlist.id}
	{canReorder}
	onAddToPlaylist={openPickerForSingle}
	onRemoveFromPlaylist={removeFromPlaylist}
/>

<PlaylistPickerSheet open={pickerOpen} releaseIds={pickerReleaseIds} onClose={() => (pickerOpen = false)} />
