<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { Playlist, DiscoveryRelease } from '$shared/types'
	import { playlistsStore } from '$shared/stores/playlists'
	import { discoveryPlaylistStore, discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
	import { playerStore, previewInfo } from '$shared/stores/player'
	import { mobileUIStore, playlistReorderMode, selectMode, selectedReleaseIds } from '$lib/stores/mobileUI'
	import { confirmDialog } from '$lib/utils/dialog'
	import { lightTap } from '$lib/utils/haptics'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import ReleaseCard from '$lib/components/discovery/ReleaseCard.svelte'
	import ReleaseActionsSheet from '$lib/components/discovery/ReleaseActionsSheet.svelte'
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
		const fetched = await playlistsStore.getPlaylistReleases(playlist.id)
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

	function playRelease(release: DiscoveryRelease, index: number) {
		void lightTap()
		void playerStore.playPreview(release, 0, releases)
		mobileUIStore.expandPlayer()
	}

	async function handleReorder(releaseIds: string[]) {
		discoveryPlaylistStore.reorderInCache(playlist.id, releaseIds)
		await playlistsStore.reorderReleases(playlist.id, releaseIds)
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

		<!-- Content -->
		<div
			class="flex-1 overflow-x-hidden {animating ? 'overflow-y-hidden' : 'overflow-y-auto'}"
			style="padding-bottom: var(--mini-player-inset, 0px)"
		>
			{#if loading}
				<div class="flex items-center justify-center py-12">
					<Spinner class="h-6 w-6 text-text-tertiary" />
				</div>
			{:else if releases.length === 0}
				<div class="px-4 py-12 text-center text-sm text-text-secondary">
					{$translate('playlists.noPlaylists')}
				</div>
			{:else if isReorderMode}
				<SortableReleaseList {releases} onReorder={handleReorder} />
			{:else}
				<div class="flex flex-col">
					{#each releases as release, index (release.id)}
						<div class="h-[72px]">
							<ReleaseCard {release} playlistId={playlist.id} context="playlist" />
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/snippet}
</Drawer>

{#if isSelectMode}
	<SelectionBar
		playlistId={playlist.id}
		onRemoveFromPlaylist={batchRemoveFromPlaylist}
		onAddToPlaylist={openPickerForSelection}
	/>
{/if}

<ReleaseActionsSheet
	{releases}
	playlistId={playlist.id}
	onAddToPlaylist={openPickerForSingle}
	onRemoveFromPlaylist={removeFromPlaylist}
/>

<PlaylistPickerSheet open={pickerOpen} releaseIds={pickerReleaseIds} onClose={() => (pickerOpen = false)} />
