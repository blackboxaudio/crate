<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { Playlist } from '$shared/types'
	import { playlistsStore } from '$shared/stores/playlists'
	import { getSmartPlaylistReleases } from '$shared/api/playlists'
	import { discoveryPlaylistStore, discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
	import { mobileUIStore, playlistReorderMode, selectMode, selectedReleaseIds } from '$lib/stores/mobileUI'
	import { confirmDialog } from '$lib/utils/dialog'
	import { refreshPlaylistCovers } from '$lib/stores/playlistCovers'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import ReleaseCard from '$lib/components/discovery/ReleaseCard.svelte'
	import ReleaseFeedList from '$lib/components/discovery/ReleaseFeedList.svelte'
	import ReleaseContextMenu from '$lib/components/discovery/ReleaseContextMenu.svelte'
	import SelectionBar from '$lib/components/discovery/SelectionBar.svelte'
	import PlaylistPickerSheet from './PlaylistPickerSheet.svelte'
	import SortableReleaseList from './SortableReleaseList.svelte'

	type Props = {
		playlist: Playlist
	}
	let { playlist }: Props = $props()

	let open = $state(true)
	let loading = $state(false)
	let releases = $derived($discoveryPlaylistReleases)
	let pickerOpen = $state(false)
	let pickerReleaseIds = $state<string[]>([])

	const isReorderMode = $derived($playlistReorderMode)
	const isSelectMode = $derived($selectMode)

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
	z={30}
	scrimZ={20}
	scrimDismiss={false}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={playlist.name}
	class="pt-safe flex w-full flex-col bg-surface-0"
>
	{#snippet children({ animating })}
		<!-- Header -->
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
		</div>

		<!-- Content. The list branch hands its scroll container to ReleaseFeedList (the same virtualized list
		     the Discovery feed uses), so it renders directly as the flex child — no outer scroll wrapper, which
		     would nest scrollers and double the mini-player inset. Loading / empty / reorder keep their own
		     scrollable wrapper; reorder stays non-virtualized because SortableReleaseList's drag math needs
		     every row present. -->
		{#if loading}
			<div class="flex flex-1 items-center justify-center py-12">
				<Spinner class="h-6 w-6 text-text-tertiary" />
			</div>
		{:else if releases.length === 0}
			<div class="flex-1 px-4 py-12 text-center text-sm text-text-secondary">
				{$translate('discovery.noReleasesYet')}
			</div>
		{:else if isReorderMode}
			<div
				class="min-h-0 flex-1 overflow-x-hidden {animating ? 'overflow-y-hidden' : 'overflow-y-auto'}"
				style="padding-bottom: var(--mini-player-inset, 0px)"
			>
				<SortableReleaseList {releases} onReorder={handleReorder} />
			</div>
		{:else}
			<ReleaseFeedList {releases} scrollLocked={animating} row={releaseRow} />
		{/if}
	{/snippet}
</Drawer>

{#snippet releaseRow({ release })}
	<ReleaseCard {release} playlistId={playlist.id} context="playlist" />
{/snippet}

{#if isSelectMode}
	<SelectionBar
		playlistId={playlist.id}
		onRemoveFromPlaylist={batchRemoveFromPlaylist}
		onAddToPlaylist={openPickerForSelection}
	/>
{/if}

<ReleaseContextMenu
	context="playlist"
	{releases}
	playlistId={playlist.id}
	onAddToPlaylist={openPickerForSingle}
	onRemoveFromPlaylist={removeFromPlaylist}
/>

<PlaylistPickerSheet open={pickerOpen} releaseIds={pickerReleaseIds} onClose={() => (pickerOpen = false)} />
