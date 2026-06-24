<script lang="ts">
	import type { FollowedSource } from '$shared/types'
	import { translate } from '$shared/i18n'
	import { discoveryStore, isDiscoveryLoading } from '$shared/stores/discovery'
	import { releasesFromSource } from '$shared/utils'
	import { mobileUIStore, selectMode, selectedReleaseIds } from '$lib/stores/mobileUI'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import ReleaseCard from '$lib/components/discovery/ReleaseCard.svelte'
	import ReleaseFeedList from '$lib/components/discovery/ReleaseFeedList.svelte'
	import ReleaseContextMenu from '$lib/components/discovery/ReleaseContextMenu.svelte'
	import SelectionBar from '$lib/components/discovery/SelectionBar.svelte'
	import PlaylistPickerSheet from '$lib/components/playlists/PlaylistPickerSheet.svelte'

	// Follow-source detail: a full-screen drill-in feed of the discovery releases from this followed artist or
	// label (opened by tapping a row in the Following tab). Mirrors TagDetailView — a right-edge Drawer over the
	// shell rendering the same virtualized ReleaseFeedList the Discovery feed uses. The release set is derived
	// client-side: a release belongs to the source when the source URL loosely matches the release's own artist
	// page (Bandcamp subdomain / SoundCloud profile) or the label page it was discovered from — the same match
	// the desktop DiscoveryRow uses for its follow indicator — so it needs no separate store or fetch. Long-press
	// / multi-select reuse the shared release machinery via a dedicated `follow` actions context.
	type Props = {
		source: FollowedSource
	}
	let { source }: Props = $props()

	let open = $state(true)

	function domain(url: string): string {
		try {
			return new URL(url).host
		} catch {
			return url
		}
	}

	// Releases from this source, taken from the shared discovery set (re-derives as releases change / sync
	// lands). Same match the playback-context scoping uses, so the queue spans exactly this list.
	const releases = $derived(releasesFromSource($discoveryStore.releases, source.url))

	const isSelectMode = $derived($selectMode)

	let pickerOpen = $state(false)
	let pickerReleaseIds = $state<string[]>([])

	// The feed normally loads these on boot, but the Following tab can be reached first — fetch if the set is empty.
	$effect(() => {
		if ($discoveryStore.releases.length === 0) discoveryStore.loadReleases()
	})

	function startClose() {
		open = false
		mobileUIStore.beginCloseFollowSource()
	}

	function onClosed() {
		mobileUIStore.closeFollowSource()
	}

	function openPickerForSingle(releaseId: string) {
		pickerReleaseIds = [releaseId]
		pickerOpen = true
	}

	function openPickerForSelection() {
		pickerReleaseIds = [...$selectedReleaseIds]
		pickerOpen = true
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
	ariaLabel={source.name ?? domain(source.url)}
	class="pt-safe flex w-full flex-col bg-surface-0"
>
	{#snippet children({ animating })}
		<!-- Header: back chevron + the source's avatar + its name. -->
		<div class="flex items-center gap-2 px-2 py-2">
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
			{#if source.artworkUrl}
				<img src={source.artworkUrl} alt="" class="h-8 w-8 flex-shrink-0 rounded object-cover" />
			{:else}
				<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary">
					{#if source.followType === 'label'}
						<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<circle cx="12" cy="12" r="9" />
							<circle cx="12" cy="12" r="2.5" />
						</svg>
					{:else}
						<svg
							class="h-4 w-4"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
						>
							<circle cx="12" cy="8" r="4" />
							<path d="M4 20c0-4 4-6 8-6s8 2 8 6" />
						</svg>
					{/if}
				</div>
			{/if}
			<h1 class="min-w-0 flex-1 truncate text-lg font-semibold text-text-primary">
				{source.name ?? domain(source.url)}
			</h1>
		</div>

		<!-- Content: the same virtualized list the Discovery feed uses, fed this source's releases. -->
		{#if $isDiscoveryLoading && $discoveryStore.releases.length === 0}
			<div class="flex flex-1 items-center justify-center py-12">
				<Spinner class="h-6 w-6 text-text-tertiary" />
			</div>
		{:else if releases.length === 0}
			<div class="flex-1 px-4 py-12 text-center text-sm text-text-secondary">
				{$translate('discovery.noReleasesYet')}
			</div>
		{:else}
			<ReleaseFeedList {releases} scrollLocked={animating} row={releaseRow} />
		{/if}
	{/snippet}
</Drawer>

{#snippet releaseRow({ release })}
	<ReleaseCard {release} context="follow" />
{/snippet}

{#if isSelectMode}
	<SelectionBar onAddToPlaylist={openPickerForSelection} />
{/if}

<ReleaseContextMenu context="follow" {releases} onAddToPlaylist={openPickerForSingle} />

<PlaylistPickerSheet open={pickerOpen} releaseIds={pickerReleaseIds} onClose={() => (pickerOpen = false)} />
