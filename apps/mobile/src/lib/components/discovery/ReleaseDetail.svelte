<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { get } from 'svelte/store'
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease } from '$shared/types'
	import { discoveryStore } from '$shared/stores/discovery'
	import { playerStore, previewInfo, previewLoading, isPlaying } from '$shared/stores/player'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { toastStore } from '$shared/stores/toast'
	import { formatDate, formatDurationCompact } from '$shared/utils/format'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { mobileUIStore, mobileDisplayedReleases } from '$lib/stores/mobileUI'
	import { lightTap } from '$lib/utils/haptics'
	import { confirmDialog } from '$lib/utils/dialog'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import MobileModal from '$lib/components/common/MobileModal.svelte'
	import MarqueeText from '$lib/components/common/MarqueeText.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import EqualizerBars from '$lib/components/common/EqualizerBars.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import MobileTagPicker from './MobileTagPicker.svelte'
	import EditReleaseSheet from './EditReleaseSheet.svelte'
	import SourceIcon from './SourceIcon.svelte'
	import PlaylistPickerSheet from '$lib/components/playlists/PlaylistPickerSheet.svelte'

	// Full-screen release detail: large artwork, metadata, editable notes (auto-save on blur),
	// assignable tags (via the bottom-sheet picker), and the track list with per-track preview playback.
	// Reads the release from the discovery store (passed in by +page) so notes/tag edits reflect live.
	// Dismissed by the back chevron or an iOS-style left-edge swipe (finger-follow); the discovery feed
	// shows through a fading scrim behind it as it slides — matching the drawers and the player.
	type Props = {
		release: DiscoveryRelease
	}
	let { release }: Props = $props()

	let tagPickerOpen = $state(false)
	let editSheetOpen = $state(false)
	let playlistPickerOpen = $state(false)

	// Release-level "more" menu (the header ⋯ button): an iOS-style context-menu platter anchored to the
	// button, holding the actions that used to be cramped, unlabeled icon buttons (Edit, Open in source) plus
	// Delete. Tap-triggered, so no lifted preview — the platter just springs from the button.
	let menuOpen = $state(false)
	let menuButtonEl = $state<HTMLButtonElement | null>(null)
	let menuAnchor = $state<{ top: number; left: number; width: number; height: number } | null>(null)

	function openMenu() {
		void lightTap()
		const r = menuButtonEl?.getBoundingClientRect()
		menuAnchor = r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null
		menuOpen = true
	}
	function menuAddToPlaylist() {
		menuOpen = false
		playlistPickerOpen = true
	}
	function menuEdit() {
		menuOpen = false
		editSheetOpen = true
	}
	function menuOpenInSource() {
		menuOpen = false
		void openUrl(release.url).catch(() => {})
	}
	async function menuDelete() {
		menuOpen = false
		const ok = await confirmDialog($translate('discovery.confirmDeleteMessage'), {
			title: $translate('discovery.confirmDeleteTitle', { values: { count: 1 } }),
			confirmLabel: $translate('common.delete'),
		})
		if (!ok) return
		// Capture the id before closing: closeDetail() unmounts this screen (the +page {#if} drops it once
		// the release leaves the store), after which the delete still completes in the background.
		const id = release.id
		mobileUIStore.closeDetail()
		await discoveryStore.deleteRelease(id)
	}

	// Per-track queue actions: the trailing ⋯ on a row opens a small sheet offering Play next / Add to
	// queue for that track. Both feed the two-tier queue (and, on iOS, the native window) live.
	let actionTrackIndex = $state<number | null>(null)
	const actionTrack = $derived(actionTrackIndex != null ? release.tracks[actionTrackIndex] : null)

	function openTrackActions(index: number) {
		void lightTap()
		actionTrackIndex = index
	}
	function queuePlayNext() {
		if (actionTrackIndex == null) return
		playbackQueue.playNext(release, actionTrackIndex)
		toastStore.success(get(translate)('queue.playingNext'))
		actionTrackIndex = null
	}
	function queueAddLast() {
		if (actionTrackIndex == null) return
		playbackQueue.addToQueue(release, actionTrackIndex)
		toastStore.success(get(translate)('queue.addedToQueue'))
		actionTrackIndex = null
	}

	const isCurrentRelease = $derived($previewInfo?.releaseId === release.id)
	const platformName = $derived(getReleasePlatformName(release.source_type))

	// Per-track loading state. The store marks which (release, track) is resolving its stream; mirror it as
	// primitives so the delay effect below only re-runs when the *target* track changes, not on every
	// playback tick. The spinner replaces that row's track number.
	const loadingReleaseId = $derived($previewLoading?.releaseId ?? null)
	const loadingTrackIndex = $derived($previewLoading?.trackIndex ?? null)

	// Don't flash the spinner for already-cached tracks (the common case) — they start within a few
	// milliseconds, and a spinner that appears and vanishes reads as a glitch. Only arm it once a load has
	// outlived SPINNER_DELAY_MS; the row's pressed highlight already acknowledges the tap instantly.
	const SPINNER_DELAY_MS = 120
	let spinnerArmed = $state(false)
	$effect(() => {
		if (loadingReleaseId == null || loadingTrackIndex == null) {
			spinnerArmed = false
			return
		}
		// New (or re-tapped) load target — re-arm the delay.
		spinnerArmed = false
		const timer = setTimeout(() => (spinnerArmed = true), SPINNER_DELAY_MS)
		return () => clearTimeout(timer)
	})

	// Play (or restart) a track. We deliberately don't special-case "same track" — re-tapping the current
	// track re-runs playPreview, which replays it from the start, so a tap always means "play this now".
	// When nothing is playing yet, slide the full-screen player up so the user lands in it; if a preview is
	// already active, the tap just swaps/restarts the track and the mini-player updates in place.
	function playTrack(index: number) {
		void lightTap()
		const wasIdle = $previewInfo == null
		// Hand the player the whole feed list (what's on screen behind this detail) as the playback queue,
		// so next/previous/auto-advance and shuffle span every release — not just this one's tracks.
		void playerStore.playPreview(release, index, get(mobileDisplayedReleases))
		if (wasIdle) mobileUIStore.expandPlayer()
	}

	// Open on mount (this is only rendered while a release is selected). Dismissal flips `open` false; the
	// Drawer slides out, then `onClosed` clears the store so +page's {#if} unmounts only after the anim.
	let open = $state(true)

	// Start the dismissal. Besides sliding the drawer out, drop the store's `detailCovering` flag now (not
	// when the slide-out finishes) so the mini-player begins rising back over the tab bar *as* the detail
	// slides away. Both close paths route here: the back chevron and the Drawer's swipe/Esc `onClose`.
	function startClose() {
		open = false
		mobileUIStore.beginCloseDetail()
	}
</script>

<Drawer
	{open}
	direction="right"
	onClose={startClose}
	onClosed={mobileUIStore.closeDetail}
	z={35}
	scrimZ={34}
	scrimDismiss={false}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={release.title ?? $translate('common.untitled')}
	class="pt-safe flex w-full flex-col bg-surface-0"
>
	{#snippet children({ animating })}
		<!-- Header -->
		<div class="flex items-center gap-1 px-2 py-2">
			<button
				type="button"
				class="flex h-10 w-10 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
				aria-label={$translate('common.close')}
				onclick={startClose}
			>
				<svg class="h-6 w-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M15 18l-6-6 6-6" stroke-linecap="round" stroke-linejoin="round" />
				</svg>
			</button>
		</div>

		<!-- Scrollable content; bottom padding clears the mini-player bar. overflow-x is pinned hidden because
	     overflow-y-auto alone computes overflow-x to `auto`, which would let overflowing content scroll
	     sideways (and pinch-zoom) on iOS. -->
		<div class="flex-1 overflow-x-hidden px-4 pt-4 pb-28 {animating ? 'overflow-y-hidden' : 'overflow-y-auto'}">
			<!-- Artwork -->
			<div class="mb-4">
				{#if release.artwork_url}
					<img src={release.artwork_url} alt="" class="aspect-square w-full rounded-xl object-cover shadow-lg" />
				{:else}
					<div class="flex aspect-square w-full items-center justify-center rounded-xl bg-surface-2 text-text-tertiary">
						<svg viewBox="0 0 24 24" class="h-16 w-16" fill="currentColor">
							<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
						</svg>
					</div>
				{/if}
			</div>

			<!-- Metadata + a "more" action menu (the ⋯ sits to the right of the info block, vertically centered,
		     to save the vertical whitespace a standalone button row would add). -->
			<div class="flex items-center gap-3">
				<div class="min-w-0 flex-1">
					<h1 class="text-xl font-semibold text-text-primary">
						{release.title ?? $translate('common.untitled')}
					</h1>
					<p class="text-base text-text-secondary">{release.artist ?? $translate('common.unknownArtist')}</p>
					<p class="mt-0.5 text-sm text-text-tertiary">
						{#if release.label}{release.label}{/if}
						{#if release.label && release.release_date}
							·
						{/if}
						{#if release.release_date}{formatDate(release.release_date)}{/if}
					</p>
				</div>
				<!-- One "more" button gathers every release-level action (Add to Playlist, Edit, Open in source,
			     Delete) into the context-menu platter below — clearer (the actions are labeled) and tidier than
			     the row of icon buttons + the standalone Add-to-Playlist button it replaces. 44px hit target. -->
				<button
					bind:this={menuButtonEl}
					type="button"
					class="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-md border border-stroke text-text-primary transition-transform active:scale-95 active:bg-surface-2"
					aria-label={$translate('common.more')}
					aria-haspopup="menu"
					aria-expanded={menuOpen}
					onclick={openMenu}
				>
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
						<circle cx="5" cy="12" r="1.8" />
						<circle cx="12" cy="12" r="1.8" />
						<circle cx="19" cy="12" r="1.8" />
					</svg>
				</button>
			</div>

			<!-- Track list -->
			<div class="mt-6">
				<h2 class="mb-1.5 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
					{$translate('discovery.tracks')}
				</h2>
				<div class="flex flex-col">
					{#each release.tracks as track, index (track.id)}
						{@const isActive = isCurrentRelease && $previewInfo?.trackIndex === index}
						{@const isLoading = spinnerArmed && loadingReleaseId === release.id && loadingTrackIndex === index}
						<!-- One full-width tap target plays the track; the heart floats on top (absolute) so the whole
						     row — heart area included — shares the same pressed highlight, yet tapping the heart likes
						     the track instead of playing it. -->
						<div class="relative rounded {isActive ? 'bg-brand-muted' : ''}">
							<button
								type="button"
								class="flex min-h-[44px] w-full min-w-0 items-center gap-3 rounded py-2 pr-20 pl-2 text-left active:bg-surface-2"
								aria-label={$translate('discovery.playPreview')}
								onclick={() => playTrack(index)}
							>
								<span class="w-5 flex-shrink-0 text-center text-xs text-text-tertiary tabular-nums">
									{#if isLoading}
										<Spinner class="mx-auto h-3.5 w-3.5" />
									{:else if isActive}
										<EqualizerBars class="mx-auto h-3.5 w-3.5" playing={$isPlaying} />
									{:else}
										{index + 1}
									{/if}
								</span>
								<MarqueeText text={track.name} class="min-w-0 flex-1 text-sm text-text-primary" />
								{#if track.duration_ms != null}
									<span class="flex-shrink-0 text-xs text-text-tertiary tabular-nums">
										{formatDurationCompact(track.duration_ms)}
									</span>
								{/if}
							</button>
							<button
								type="button"
								class="absolute inset-y-0 right-10 flex w-10 items-center justify-center text-text-tertiary active:bg-surface-2"
								aria-label={$translate('queue.addToQueue')}
								onclick={() => openTrackActions(index)}
							>
								<svg class="h-4 w-4" viewBox="0 0 24 24" fill="currentColor">
									<circle cx="5" cy="12" r="1.6" />
									<circle cx="12" cy="12" r="1.6" />
									<circle cx="19" cy="12" r="1.6" />
								</svg>
							</button>
							<button
								type="button"
								class="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-r text-text-tertiary active:bg-surface-2"
								aria-label={track.is_liked ? $translate('discovery.unlike') : $translate('discovery.like')}
								onclick={() => discoveryStore.toggleTrackLiked(release.id, track.id)}
							>
								<svg
									class="h-4 w-4 {track.is_liked ? 'text-brand-primary' : ''}"
									viewBox="0 0 24 24"
									fill={track.is_liked ? 'currentColor' : 'none'}
									stroke="currentColor"
									stroke-width="2"
								>
									<path
										d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78L12 21.23l8.84-8.84a5.5 5.5 0 0 0 0-7.78z"
									/>
								</svg>
							</button>
						</div>
					{/each}
				</div>
			</div>

			<!-- Tags -->
			<div class="mt-6">
				<h2 class="mb-1.5 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
					{$translate('nav.tags')}
				</h2>
				<div class="flex flex-wrap items-center gap-1.5">
					{#each release.tags as tag (tag.id)}
						{@const color = tag.color ?? '#888888'}
						<span
							class="inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium"
							style="background-color: {color}20; color: {color}; border: 1px solid {color}40;"
						>
							{tag.name}
						</span>
					{/each}
					<button
						type="button"
						class="inline-flex items-center gap-1 rounded border border-dashed border-stroke px-2 py-0.5 text-xs text-text-secondary active:bg-surface-2"
						onclick={() => (tagPickerOpen = true)}
					>
						<svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M12 5v14M5 12h14" stroke-linecap="round" />
						</svg>
						{$translate('discovery.editor.addTags')}
					</button>
				</div>
			</div>

			<!-- Notes (read-only; editing is via the Edit sheet) -->
			{#if release.notes}
				<div class="mt-6">
					<h2 class="mb-1.5 text-xs font-semibold tracking-wide text-text-tertiary uppercase">
						{$translate('discovery.editor.notes')}
					</h2>
					<p class="text-sm whitespace-pre-wrap text-text-secondary">{release.notes}</p>
				</div>
			{/if}
		</div>
	{/snippet}
</Drawer>

<MobileTagPicker open={tagPickerOpen} releaseIds={[release.id]} onClose={() => (tagPickerOpen = false)} />
<EditReleaseSheet open={editSheetOpen} {release} onClose={() => (editSheetOpen = false)} />
<PlaylistPickerSheet open={playlistPickerOpen} releaseIds={[release.id]} onClose={() => (playlistPickerOpen = false)} />

<!-- Release-level "more" menu (opened by the header ⋯ button). Tap-triggered, so no lifted preview. -->
<ContextMenu open={menuOpen} anchorRect={menuAnchor} tapTriggered onClose={() => (menuOpen = false)}>
	<ContextMenuItem onclick={menuAddToPlaylist}>
		{$translate('contextMenu.addToPlaylist')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M12 5v14M5 12h14" stroke-linecap="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={menuEdit}>
		{$translate('discovery.editRelease')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path
					d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
				<path
					d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"
					stroke-linecap="round"
					stroke-linejoin="round"
				/>
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem separatorBefore onclick={menuOpenInSource}>
		{platformName
			? $translate('discovery.openInApp', { values: { app: platformName } })
			: $translate('discovery.openInBrowser')}
		{#snippet icon()}
			<SourceIcon source={release.source_type} />
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem destructive onclick={menuDelete}>
		{$translate('discovery.deleteRelease')}
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

<!-- Per-track queue actions (opened by a row's ⋯ button). -->
<MobileModal
	open={actionTrackIndex != null}
	onClose={() => (actionTrackIndex = null)}
	title={actionTrack?.name ?? $translate('queue.addToQueue')}
>
	<div class="flex flex-col">
		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
			onclick={queuePlayNext}
		>
			<svg class="h-5 w-5 flex-shrink-0 text-text-secondary" viewBox="0 0 24 24" fill="currentColor">
				<path d="M5 5l11 7-11 7z" />
				<rect x="17.5" y="5" width="2" height="14" rx="1" />
			</svg>
			{$translate('queue.playNext')}
		</button>
		<button
			type="button"
			class="flex items-center gap-3 rounded-md px-2 py-3 text-left text-sm text-text-primary active:bg-surface-2"
			onclick={queueAddLast}
		>
			<svg
				class="h-5 w-5 flex-shrink-0 text-text-secondary"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
			>
				<path d="M4 6h11M4 12h11M4 18h7M19 14v6M16 17h6" stroke-linecap="round" />
			</svg>
			{$translate('queue.addToQueue')}
		</button>
	</div>
</MobileModal>
