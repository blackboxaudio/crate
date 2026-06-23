<script lang="ts">
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { toastStore } from '$shared/stores/toast'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { mobileUIStore, actionsReleaseId, actionsContext, actionsAnchorRect } from '$lib/stores/mobileUI'
	import { confirmDialog } from '$lib/utils/dialog'
	import { lightTap } from '$lib/utils/haptics'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import ReleaseCardContent from './ReleaseCardContent.svelte'
	import SourceIcon from './SourceIcon.svelte'

	// iOS-style context menu for a discovery release (the long-press menu on a feed/playlist row). Wraps the
	// generic `ContextMenu`: it lifts a preview of the row (rendered via `ReleaseCardContent`, no DOM clone)
	// and lists the same actions the bottom action sheet used to. Driven by the shared `actions*` store; the
	// caller mounts one always-present instance per context (feed vs playlist) and passes the `context` it
	// serves, so only the matching instance opens (and the close animation isn't cut short by an unmount).
	type Props = {
		/** Which context this instance serves. It opens only when the store's context matches, so the feed
		 *  and playlist-detail instances (both always mounted) never both render for the same release. */
		context: 'feed' | 'playlist'
		releases: DiscoveryRelease[]
		playlistId?: string | null
		onAddToPlaylist?: (releaseId: string) => void
		onRemoveFromPlaylist?: (releaseId: string) => void
	}
	let { context, releases, playlistId = null, onAddToPlaylist, onRemoveFromPlaylist }: Props = $props()

	const releaseId = $derived($actionsReleaseId)
	const anchorRect = $derived($actionsAnchorRect)
	const release = $derived(releaseId ? (releases.find((r) => r.id === releaseId) ?? null) : null)
	const open = $derived(release != null && anchorRect != null && $actionsContext === context)

	// Latch the release so the lifted preview keeps rendering through the dismiss animation (after an action
	// clears the store, `release` goes null but the menu is still sliding out). Cleared on `onClosed`.
	let displayed = $state<DiscoveryRelease | null>(null)
	$effect(() => {
		if (release) displayed = release
	})
	const platformName = $derived(displayed ? getReleasePlatformName(displayed.source_type) : null)

	// If the release vanishes while this instance's menu is open (e.g. a sync deletes it), tear it down.
	$effect(() => {
		if ($actionsContext === context && releaseId && !release) mobileUIStore.closeActionsSheet()
	})

	function close() {
		mobileUIStore.closeActionsSheet()
	}

	// Each handler snapshots the id/release up front: `close()` clears the store, after which the derived
	// `releaseId`/`release` read null — so we must capture before closing (and before any await).
	function handleSelect() {
		const id = releaseId
		if (!id) return
		close()
		mobileUIStore.enterSelectMode(id)
	}

	function handleAddToPlaylist() {
		const id = releaseId
		if (!id) return
		close()
		onAddToPlaylist?.(id)
	}

	function handleRemoveFromPlaylist() {
		const id = releaseId
		if (!id) return
		close()
		onRemoveFromPlaylist?.(id)
	}

	function handlePlayNext() {
		const r = release
		if (!r || r.tracks.length === 0) return
		void lightTap()
		playbackQueue.playNext(r, 0)
		toastStore.success(get(translate)('queue.playingNext'))
		close()
	}

	function handleAddToQueue() {
		const r = release
		if (!r || r.tracks.length === 0) return
		void lightTap()
		playbackQueue.addToQueue(r, 0)
		toastStore.success(get(translate)('queue.addedToQueue'))
		close()
	}

	function handleReorder() {
		close()
		mobileUIStore.toggleReorderMode()
	}

	async function handleDelete() {
		const id = releaseId
		if (!id) return
		const ok = await confirmDialog($translate('discovery.confirmDeleteMessage'), {
			title: $translate('discovery.confirmDeleteTitle', { values: { count: 1 } }),
			confirmLabel: $translate('common.delete'),
		})
		if (!ok) return
		close()
		await discoveryStore.deleteRelease(id)
	}

	function handleOpenInSource() {
		const r = release
		if (!r) return
		void openUrl(r.url).catch(() => {})
		close()
	}
</script>

<ContextMenu {open} {anchorRect} onClose={close} onClosed={() => (displayed = null)}>
	{#snippet preview()}
		{#if displayed}
			<ReleaseCardContent release={displayed} />
		{/if}
	{/snippet}

	<ContextMenuItem onclick={handleAddToPlaylist}>
		{$translate('contextMenu.addToPlaylist')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M12 5v14M5 12h14" stroke-linecap="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={handlePlayNext}>
		{$translate('queue.playNext')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
				<path d="M5 5l11 7-11 7z" />
				<rect x="17.5" y="5" width="2" height="14" rx="1" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={handleAddToQueue}>
		{$translate('queue.addToQueue')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M4 6h11M4 12h11M4 18h7M19 14v6M16 17h6" stroke-linecap="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	{#if context === 'playlist' && playlistId}
		<ContextMenuItem onclick={handleReorder}>
			{$translate('queue.reorder')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M7 15l5 5 5-5M7 9l5-5 5 5" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			{/snippet}
		</ContextMenuItem>

		<ContextMenuItem destructive onclick={handleRemoveFromPlaylist}>
			{$translate('contextMenu.removeFromPlaylist')}
			{#snippet icon()}
				<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M5 12h14" stroke-linecap="round" />
				</svg>
			{/snippet}
		</ContextMenuItem>
	{/if}

	<ContextMenuItem onclick={handleSelect}>
		{$translate('common.select')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M9 11l3 3L22 4" stroke-linecap="round" stroke-linejoin="round" />
				<path
					d="M21 12v7a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2h11"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem separatorBefore onclick={handleOpenInSource}>
		{platformName
			? $translate('discovery.openInApp', { values: { app: platformName } })
			: $translate('discovery.openInBrowser')}
		{#snippet icon()}
			{#if displayed}<SourceIcon source={displayed.source_type} />{/if}
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem destructive onclick={handleDelete}>
		{$translate('common.delete')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path
					d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/snippet}
	</ContextMenuItem>
</ContextMenu>
