<script lang="ts">
	import { untrack } from 'svelte'
	import { get } from 'svelte/store'
	import { playlistsStore } from '$shared/stores/playlists'
	import {
		mobileUIStore,
		playlistFolderTrail,
		overlayPopNonce,
		detailPlaylistId,
		detailReleaseId,
	} from '$lib/stores/mobileUI'
	import { overlayMiniPlayerInset } from '$lib/stores/insets'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import DetailHeader from '$lib/components/common/DetailHeader.svelte'
	import PlaylistLevel from './PlaylistLevel.svelte'

	// One pushed folder level of the Playlists tab: the same right-edge Drawer + back header the playlist /
	// tag / follow details use, wrapping the shared `PlaylistLevel` list. +page mounts one per entry of the
	// folder trail, in trail order, so levels stack beneath the playlist detail; closing follows the detail
	// overlays' choreography (begin-close drops the covering flag, the trail truncates once the slide lands).
	type Props = {
		folderId: string
	}
	let { folderId }: Props = $props()

	const folder = $derived(
		$playlistsStore.playlists.find((p) => p.id === folderId && p.is_folder && p.context === 'discovery') ?? null
	)

	let open = $state(true)
	// Boot-restored (this level was open when the app was last killed): appear in place, no slide-in. Read
	// once at mount on purpose — a level is keyed by its folder id, so the prop never changes.
	const enterInstant = untrack(() => mobileUIStore.consumeBootRestoredFolder(folderId))

	function startClose() {
		open = false
		mobileUIStore.beginClosePlaylistFolder(folderId)
	}

	function onClosed() {
		mobileUIStore.closePlaylistFolder(folderId)
	}

	// iOS "re-tap the active tab to pop": the tab bar bumps `overlayPopNonce`. Only the topmost level answers
	// — the deepest folder, and only when no playlist / release detail is stacked above it (those close first;
	// a later tap then reaches here).
	let seenPopNonce = get(overlayPopNonce)
	$effect(() => {
		const n = $overlayPopNonce
		if (n === seenPopNonce) return
		seenPopNonce = n
		const trail = get(playlistFolderTrail)
		const isTop = trail[trail.length - 1] === folderId
		if (open && isTop && get(detailPlaylistId) === null && get(detailReleaseId) === null) startClose()
	})

	// The folder vanishing mid-session (deleted on another device, cloud-synced away) would leave this level
	// unmounted but still in the trail — with deeper levels orphaned above it and the covering flag stuck.
	// Once it has resolved at least once, its disappearance closes the level for good. Boot-time validation
	// (`navRestore`) covers the not-yet-loaded case.
	let resolvedOnce = false
	$effect(() => {
		if (folder) resolvedOnce = true
		else if (resolvedOnce) mobileUIStore.closePlaylistFolder(folderId)
	})
</script>

{#if folder}
	<!-- All pushed levels share z-30, so DOM order stacks them; the scrim at the same z lands between the
	     level beneath and this panel, dimming exactly that level during the slide. -->
	<Drawer
		{open}
		direction="right"
		onClose={startClose}
		{onClosed}
		{enterInstant}
		z={30}
		scrimZ={30}
		scrimDismiss={false}
		closeEdgeFrom="left"
		closeEdgeSize={24}
		ariaLabel={folder.name}
		class="flex w-full flex-col bg-surface-0"
		style="--mini-player-inset: {$overlayMiniPlayerInset}"
	>
		{#snippet children({ animating })}
			<DetailHeader title={folder.name} onBack={startClose} />
			<PlaylistLevel folderId={folder.id} scrollLocked={animating} />
		{/snippet}
	</Drawer>
{/if}
