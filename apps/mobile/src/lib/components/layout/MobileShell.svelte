<script lang="ts">
	import { onMount } from 'svelte'
	import { fly, fade } from 'svelte/transition'
	import { easeFluid } from '$lib/easing'
	import {
		activeTab,
		selectMode,
		selectedReleaseIds,
		detailPlaylistId,
		detailTagId,
		detailFollowSourceId,
	} from '$lib/stores/mobileUI'
	import { previewInfo } from '$shared/stores/player'
	import { sortedReleases } from '$shared/stores/discovery'
	import Header from './Header.svelte'
	import TabBar from './TabBar.svelte'
	import SelectionBar from '$lib/components/discovery/SelectionBar.svelte'
	import ReleaseContextMenu from '$lib/components/discovery/ReleaseContextMenu.svelte'
	import PlaylistPickerSheet from '$lib/components/playlists/PlaylistPickerSheet.svelte'
	import MobileDiscoveryView from '$lib/components/discovery/MobileDiscoveryView.svelte'
	import FollowingView from '$lib/components/following/FollowingView.svelte'
	import PlaylistsView from '$lib/components/playlists/PlaylistsView.svelte'
	import TagsView from '$lib/components/tags/TagsView.svelte'
	import SettingsView from '$lib/components/settings/SettingsView.svelte'

	let feedPickerOpen = $state(false)
	let feedPickerReleaseIds = $state<string[]>([])

	function openFeedPicker(releaseId: string) {
		feedPickerReleaseIds = [releaseId]
		feedPickerOpen = true
	}

	function openFeedPickerForSelection() {
		feedPickerReleaseIds = [...$selectedReleaseIds]
		feedPickerOpen = true
	}

	// Composition root for the mobile app: a branded fixed Header (top), the active tab's view, and the
	// fixed bottom TabBar. `<main>` is a non-scrolling frame (overflow-hidden) — each view owns its own
	// scroll container (the discovery feed must be the virtualizer's scroll element). The frame's padding
	// reserves space for the Header (top) and the tab bar (bottom) so a view's content can't run under that
	// chrome. 3.5rem matches the Header / TabBar height (`h-14`). The full-screen overlays (release detail,
	// expanded player) are layered on top by +page.
	//
	// The mini-player is intentionally NOT reserved here: it's a floating liquid-glass card meant to sit
	// *over* the feed so releases stay visible (blurred) underneath it. Instead we publish its clearance as
	// `--mini-player-inset`, which each scroll view applies as trailing padding — that lets content scroll
	// under the card while still letting the last row clear it. 5rem ≈ the card's height + its gap above the
	// tab bar; 0 when no preview is active (the card is hidden).
	const miniPlayerInset = $derived($previewInfo ? '5rem' : '0px')

	// Tab-change transition: the incoming view rises a few px and fades in while the outgoing one fades
	// out. They stack absolutely in the content frame, so they cross-fade rather than push each other.
	// Honors the OS reduced-motion setting by collapsing to an instant swap.
	let reduceMotion = $state(false)
	onMount(() => {
		const mq = window.matchMedia('(prefers-reduced-motion: reduce)')
		reduceMotion = mq.matches
		const update = () => (reduceMotion = mq.matches)
		mq.addEventListener('change', update)
		return () => mq.removeEventListener('change', update)
	})
	const inMs = $derived(reduceMotion ? 0 : 240)
	const outMs = $derived(reduceMotion ? 0 : 160)
</script>

<div class="relative h-dvh w-screen overflow-hidden bg-surface-0" style="--mini-player-inset: {miniPlayerInset}">
	<Header />

	<main
		class="relative h-full overflow-hidden"
		style="padding-top: calc(3.5rem + env(safe-area-inset-top)); padding-bottom: calc(3.5rem + env(safe-area-inset-bottom))"
	>
		<!-- Content frame: fills main's content box (below the header, above the tab bar) and is the
		     positioning context for the tab views, which stack absolutely while a swap cross-fades. -->
		<div class="relative h-full w-full">
			{#key $activeTab}
				<div
					class="absolute inset-0"
					in:fly={{ y: 8, duration: inMs, easing: easeFluid }}
					out:fade={{ duration: outMs }}
				>
					{#if $activeTab === 'discovery'}
						<MobileDiscoveryView />
					{:else if $activeTab === 'following'}
						<FollowingView />
					{:else if $activeTab === 'playlists'}
						<PlaylistsView />
					{:else if $activeTab === 'tags'}
						<TagsView />
					{:else}
						<SettingsView />
					{/if}
				</div>
			{/key}
		</div>
	</main>

	<TabBar />

	<!-- Multi-select action bar overlays the tab bar's slot while the discovery feed is in select mode.
	     Suppressed when a playlist or tag overlay is open — each renders its own bar above its own feed. -->
	{#if $selectMode && !$detailPlaylistId && !$detailTagId && !$detailFollowSourceId}
		<SelectionBar onAddToPlaylist={openFeedPickerForSelection} />
	{/if}
</div>

<!-- Always mounted; opens only for its own context, so the feed and playlist-detail instances never both
     render for a release present in both. -->
<ReleaseContextMenu context="feed" releases={$sortedReleases} onAddToPlaylist={openFeedPicker} />
<PlaylistPickerSheet open={feedPickerOpen} releaseIds={feedPickerReleaseIds} onClose={() => (feedPickerOpen = false)} />
