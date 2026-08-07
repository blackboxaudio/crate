<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { get } from 'svelte/store'
	import { cubicOut } from 'svelte/easing'
	import { translate } from '$shared/i18n'
	import type { DiscoveryRelease, DiscoveryTrack } from '$shared/types'
	import ReleaseArtwork from '$lib/components/common/ReleaseArtwork.svelte'
	import { discoveryStore } from '$shared/stores/discovery'
	import {
		precachePreviewStream,
		purgeReleaseAudioCache,
		getReleaseCacheState,
		type ReleaseCacheState,
	} from '$shared/api/discovery'
	import { playerStore, previewInfo, previewLoading, isPlaying } from '$shared/stores/player'
	import { dateFormat, language } from '$shared/stores/settings'
	import * as playbackQueue from '$shared/stores/playbackQueue'
	import { toastStore } from '$shared/stores/toast'
	import { shareUrl } from '$shared/api/app'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { formatDate, formatDurationCompact } from '$shared/utils/format'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { deriveArtistUrl, deriveLabelUrl, isCompilation } from '$shared/utils'
	import { mobileUIStore, activePlaybackContext, overlayPopNonce } from '$lib/stores/mobileUI'
	import { lightTap, rigidTap } from '$lib/utils/haptics'
	import { confirmDialog } from '$lib/utils/dialog'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import MarqueeText from '$lib/components/common/MarqueeText.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import PullToRefresh from '$lib/components/common/PullToRefresh.svelte'
	import EqualizerBars from '$lib/components/common/EqualizerBars.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import TrackListSkeleton from './TrackListSkeleton.svelte'
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
	function menuShare() {
		menuOpen = false
		// The OS share sheet is the feedback — no toast.
		void shareUrl(release.url, release.title ?? undefined).catch(() => {})
	}
	async function menuCopyUrl() {
		menuOpen = false
		try {
			await writeText(release.url)
			// Exception to the sparing-toasts rule: a clipboard write has no other visible feedback.
			toastStore.info(get(translate)('discovery.copiedUrl'))
		} catch {
			// Clipboard denied — nothing useful to surface.
		}
	}
	function menuRefresh() {
		menuOpen = false
		void discoveryStore.refreshMetadata(release.id)
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

	// Whether the release exposes a followable artist/label page — gates the "Follow" menu item (mirrors
	// the release feed's context menu). A Various-Artists comp has no artist target but may have a label.
	const canFollow = $derived.by(() => {
		const comp = isCompilation(release.artist)
		const artistUrl = comp ? null : deriveArtistUrl(release)
		const labelUrl = comp ? (deriveLabelUrl(release) ?? deriveArtistUrl(release)) : deriveLabelUrl(release)
		return !!artistUrl || !!labelUrl
	})

	// Whole-release queue actions + Follow, mirroring the feed's long-press menu so the "more" menu offers
	// the same actions. The opening haptic already fired in openMenu(), so these don't tap again (matching
	// the other menu items); the toast confirms since the queue isn't on screen.
	function menuPlayNext() {
		menuOpen = false
		if (release.tracks.length === 0) return
		playbackQueue.playReleaseNext(release)
		toastStore.success(get(translate)('queue.playingNext'))
	}
	function menuAddToQueue() {
		menuOpen = false
		if (release.tracks.length === 0) return
		playbackQueue.addReleaseToQueue(release)
		toastStore.success(get(translate)('queue.addedToQueue'))
	}
	function menuFollow() {
		menuOpen = false
		mobileUIStore.openFollowSheet(release.id)
	}

	// Offline download: pre-cache the release's audio bytes so it plays in airplane mode, and let the
	// user reclaim that space per-release. `cacheState` drives the header "downloaded" badge and gates
	// which menu item shows (Download vs Remove). Refreshed on open and after each action.
	let cacheState = $state<ReleaseCacheState | null>(null)
	let downloading = $state(false)
	const isFullyDownloaded = $derived(
		!!cacheState && cacheState.total_tracks > 0 && cacheState.cached_tracks >= cacheState.total_tracks
	)
	const hasSomeCached = $derived(!!cacheState && cacheState.cached_tracks > 0)

	async function refreshCacheState(id: string) {
		try {
			cacheState = await getReleaseCacheState(id)
		} catch {
			cacheState = null
		}
	}

	// Reload the cache state whenever this detail shows a different release.
	$effect(() => {
		void refreshCacheState(release.id)
	})

	async function menuDownloadForOffline() {
		menuOpen = false
		if (downloading || release.tracks.length === 0) return
		downloading = true
		// Sequential: the proxy downloads one stream at a time anyway, and this keeps memory + network
		// pressure low on mobile. Per-track failures are tolerated so a single dead stream doesn't abort
		// the whole release; the badge simply won't reach "fully downloaded".
		let anySucceeded = false
		for (const track of release.tracks) {
			// A track the source serves no preview for (pre-order, no duration) has nothing to download.
			if (!trackPlayable(track)) continue
			try {
				await precachePreviewStream(release.id, track.position)
				anySucceeded = true
			} catch {
				// keep going
			}
		}
		await refreshCacheState(release.id)
		downloading = false
		if (!anySucceeded) toastStore.error(get(translate)('discovery.downloadFailed'))
	}

	async function menuRemoveDownload() {
		menuOpen = false
		try {
			// Purge (bytes + URLs) — NOT the URL-only invalidate the error-retry uses; this is the
			// one place removing downloaded audio is the point.
			await purgeReleaseAudioCache(release.id)
		} catch {
			// best-effort; refresh reflects reality either way
		}
		await refreshCacheState(release.id)
	}

	// Per-track queue actions: a long-press on a track row lifts it and springs an iOS-style context menu
	// offering Play next / Add to queue for that track. Both feed the two-tier queue (and, on iOS, the
	// native window) live. Mirrors the long-press pattern used by the playlist rows and the release feed.
	let actionTrackIndex = $state<number | null>(null)
	const actionTrack = $derived(actionTrackIndex != null ? release.tracks[actionTrackIndex] : null)
	let trackMenuOpen = $state(false)
	// Viewport rect of the long-pressed row, so the context menu can lift a preview of it in place.
	let trackMenuRect = $state<{ top: number; left: number; width: number; height: number } | null>(null)

	let trackLongPressTimer = 0
	// A stationary long-press also synthesizes a click on release; this latches so we swallow that one
	// click (otherwise opening the menu would also play the track — the row is a real <button>).
	let suppressNextTrackClick = false

	function startTrackLongPress(e: PointerEvent, index: number) {
		suppressNextTrackClick = false
		if (trackLongPressTimer) clearTimeout(trackLongPressTimer)
		// Capture the row element now; `currentTarget` is nulled once the event finishes dispatching.
		const el = e.currentTarget as HTMLElement
		trackLongPressTimer = window.setTimeout(() => {
			trackLongPressTimer = 0
			void rigidTap()
			const r = el?.getBoundingClientRect()
			trackMenuRect = r ? { top: r.top, left: r.left, width: r.width, height: r.height } : null
			actionTrackIndex = index
			suppressNextTrackClick = true
			trackMenuOpen = true
		}, 450)
		window.addEventListener('pointermove', cancelTrackLongPress, { once: true, passive: true })
		window.addEventListener('pointerup', cancelTrackLongPress, { once: true })
		window.addEventListener('pointercancel', cancelTrackLongPress, { once: true })
	}

	function cancelTrackLongPress() {
		if (trackLongPressTimer) {
			clearTimeout(trackLongPressTimer)
			trackLongPressTimer = 0
		}
	}

	// Tear down a pending long-press timer if the detail unmounts mid-press (e.g. swiped closed).
	$effect(() => () => cancelTrackLongPress())

	// Swallow the synthesized click that follows a long-press so the menu doesn't also play the track.
	function onTrackClickCapture(e: MouseEvent) {
		if (!suppressNextTrackClick) return
		suppressNextTrackClick = false
		e.preventDefault()
		e.stopPropagation()
	}

	function queuePlayNext() {
		if (actionTrackIndex == null) return
		playbackQueue.playNext(release, actionTrackIndex)
		toastStore.success(get(translate)('queue.playingNext'))
		trackMenuOpen = false
	}
	function queueAddLast() {
		if (actionTrackIndex == null) return
		playbackQueue.addToQueue(release, actionTrackIndex)
		toastStore.success(get(translate)('queue.addedToQueue'))
		trackMenuOpen = false
	}

	// Track-level share/copy: prefer the track's own page (Bandcamp `/track/...`, SoundCloud
	// permalink — populated since the per-track url migration; refreshed metadata backfills older
	// releases), falling back to the release URL. Snapshot before closing — `actionTrack` derives
	// from an index the close animation clears.
	function trackShare() {
		const t = actionTrack
		trackMenuOpen = false
		if (!t) return
		void shareUrl(t.url ?? release.url, t.name).catch(() => {})
	}
	async function trackCopyUrl() {
		const t = actionTrack
		trackMenuOpen = false
		if (!t) return
		try {
			await writeText(t.url ?? release.url)
			toastStore.info(get(translate)('discovery.copiedUrl'))
		} catch {
			// Clipboard denied — nothing useful to surface.
		}
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

	// Whether a track can be previewed at all — the same gate desktop's `trackCanPlay` applies: no
	// duration means the source never exposed the track as playable (unreleased pre-order tracks,
	// unenriched rows), `preview_unavailable` is the extraction-confirmed flag, and Discogs tracks
	// play via their matched YouTube video only. Unplayable rows render greyed-out and inert.
	function trackPlayable(track: DiscoveryTrack): boolean {
		if (!track.duration_ms) return false
		if (track.preview_unavailable) return false
		if (release.source_type === 'discogs') return track.video_id != null
		return true
	}

	// Play (or restart) a track. We deliberately don't special-case "same track" — re-tapping the current
	// track re-runs playPreview, which replays it from the start, so a tap always means "play this now".
	// When nothing is playing yet, slide the full-screen player up so the user lands in it; if a preview is
	// already active, the tap just swaps/restarts the track and the mini-player updates in place.
	function playTrack(index: number) {
		const track = release.tracks[index]
		if (!track || !trackPlayable(track)) return
		void lightTap()
		const wasIdle = $previewInfo == null
		// Scope the playback queue to the view this detail was opened from — the discovery feed, or a
		// playlist / tag / followed-source drill-in — so next/previous/auto-advance and shuffle span exactly
		// those releases, not just this one's tracks. Record the origin too: only a discovery-feed-started
		// queue keeps following the feed's live filter (see +page); a tag/follow/playlist queue stays fixed.
		const context = get(activePlaybackContext)
		mobileUIStore.setQueueOrigin(context.origin)
		void playerStore.playPreview(release, index, context.releases)
		if (wasIdle) mobileUIStore.expandPlayer()
	}

	// Whether this release's metadata is currently being (re)fetched — drives the empty track-list
	// placeholder skeleton. The store tracks in-flight ids.
	const isRefreshing = $derived($discoveryStore.refreshingIds.has(release.id))

	// The scrollable content element, handed to PullToRefresh so a pull-down re-fetches this release's
	// metadata (the same action as the header menu's "Refresh Metadata").
	let contentEl = $state<HTMLElement | null>(null)
	async function refreshDetail() {
		await discoveryStore.refreshMetadata(release.id)
	}

	// Delay before clearing the "new" flag on open: long enough for the drawer to finish sliding in (500ms)
	// and the badge to sit visibly for a beat, so it then pops out (see out:pop) rather than vanishing
	// under the opening transition.
	const NEW_CLEAR_DELAY = 850

	// A little "pop" as the badge leaves: it grows and fades so clearing the "new" state is noticeable.
	function pop(_node: Element, { duration = 260 } = {}) {
		return {
			duration,
			easing: cubicOut,
			css: (t: number) => `transform: scale(${1 + (1 - t) * 0.4}); opacity: ${t}`,
		}
	}

	// Once-per-open setup. Runs when the detail is opened for a release (this screen mounts per open).
	let didInit = false
	$effect(() => {
		if (didInit) return
		didInit = true
		// Viewing the detail counts as "seen" — clear the release's "new" flag after the delay (no-op if not
		// new). Not cancelled on close so a quick open still registers the view; clearNew is safe post-unmount.
		setTimeout(() => discoveryStore.clearNew(release.id), NEW_CLEAR_DELAY)
		// Auto-fetch tracks when the release was opened empty (common for bulk-imported or watcher-surfaced
		// releases saved with only a URL). The backend's refresh creates the tracks from the source; the
		// live `release` prop then re-renders them. Guarded so a release that genuinely has no tracks
		// doesn't re-fetch on every reactive tick.
		if (release.tracks.length === 0) void discoveryStore.refreshMetadata(release.id)
		// Pre-order upkeep: if this release shows unavailable tracks (or hasn't released yet),
		// silently re-check availability at the source so the greyed rows heal on release day.
		discoveryStore.maybeRecheckAvailability(release)
	})

	// Open on mount (this is only rendered while a release is selected). Dismissal flips `open` false; the
	// Drawer slides out, then `onClosed` clears the store so +page's {#if} unmounts only after the anim.
	let open = $state(true)
	// Boot-restored (this overlay was open when the app was last killed): appear in place, no slide-in.
	const enterInstant = mobileUIStore.consumeBootRestoredOverlay('release')

	// Start the dismissal. Besides sliding the drawer out, drop the store's `detailCovering` flag now (not
	// when the slide-out finishes) so the mini-player begins rising back over the tab bar *as* the detail
	// slides away. Both close paths route here: the back chevron and the Drawer's swipe/Esc `onClose`.
	function startClose() {
		open = false
		mobileUIStore.beginCloseDetail()
	}

	// iOS "re-tap the active tab to pop to root": the tab bar bumps `overlayPopNonce`. The release detail is
	// always the topmost overlay, so it closes on the bump — the same animated path as the back chevron.
	let seenPopNonce = get(overlayPopNonce)
	$effect(() => {
		const n = $overlayPopNonce
		if (n === seenPopNonce) return
		seenPopNonce = n
		if (open) startClose()
	})
</script>

<Drawer
	{open}
	direction="right"
	onClose={startClose}
	onClosed={mobileUIStore.closeDetail}
	{enterInstant}
	z={35}
	scrimZ={34}
	scrimDismiss={false}
	closeEdgeFrom="left"
	closeEdgeSize={24}
	ariaLabel={release.title ?? $translate('common.untitled')}
	class="flex w-full flex-col bg-surface-0"
>
	{#snippet children({ animating })}
		<!-- Header. Owns the top safe-area inset and mirrors the fixed top bar's surface-1 + hairline so
		     drill-in headers read as the same app chrome. -->
		<div class="pt-safe border-b border-stroke-subtle bg-surface-1">
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
		</div>

		<!-- Scrollable content; bottom padding clears the mini-player bar. overflow-x is pinned hidden because
	     overflow-y-auto alone computes overflow-x to `auto`, which would let overflowing content scroll
	     sideways (and pinch-zoom) on iOS. Wrapped so a pull-down at the top re-fetches this release's
	     metadata (disabled while the drawer is animating). -->
		<div class="relative flex min-h-0 flex-1 flex-col">
			<PullToRefresh scrollEl={contentEl} onRefresh={refreshDetail} enabled={!animating} />
			<div
				bind:this={contentEl}
				class="min-h-0 flex-1 overflow-x-hidden px-4 pt-4 pb-28 {animating ? 'overflow-y-hidden' : 'overflow-y-auto'}"
			>
				<!-- Artwork -->
				<div class="mb-4">
					<ReleaseArtwork {release} class="aspect-square w-full rounded-xl object-cover shadow-lg" />
				</div>

				<!-- Metadata + a "more" action menu (the ⋯ sits to the right of the info block, vertically centered,
		     to save the vertical whitespace a standalone button row would add). -->
				<div class="flex items-center gap-3">
					<div class="min-w-0 flex-1">
						<h1 class="text-xl font-semibold text-text-primary">
							{release.title ?? $translate('common.untitled')}{#if release.is_new}<span
									out:pop
									class="ml-2 inline-block rounded-full bg-brand-muted px-1.5 py-0.5 align-middle text-[10px] font-semibold text-brand-primary"
									>{$translate('filters.new')}</span
								>{/if}
						</h1>
						<p class="text-base text-text-secondary">{release.artist ?? $translate('common.unknownArtist')}</p>
						<p class="mt-0.5 text-sm text-text-tertiary">
							{#if release.label}{release.label}{/if}
							{#if release.label && release.release_date}
								·
							{/if}
							{#if release.release_date}{formatDate(release.release_date, $dateFormat, $language)}{/if}
						</p>
						{#if downloading}
							<p class="mt-1 flex items-center gap-1.5 text-xs font-medium text-text-tertiary">
								<Spinner class="h-3 w-3" />
								{$translate('discovery.downloading')}
							</p>
						{:else if isFullyDownloaded}
							<p class="mt-1 flex items-center gap-1 text-xs font-medium text-brand-primary">
								<svg
									class="h-3.5 w-3.5"
									viewBox="0 0 24 24"
									fill="none"
									stroke="currentColor"
									stroke-width="2.5"
									stroke-linecap="round"
									stroke-linejoin="round"
								>
									<path d="M20 6 9 17l-5-5" />
								</svg>
								{$translate('discovery.downloadedForOffline')}
							</p>
						{/if}
					</div>
					<!-- One "more" button gathers every release-level action (Add to Playlist, Edit, Open in source,
			     Delete) into the context-menu platter below — clearer (the actions are labeled) and tidier than
			     the row of icon buttons + the standalone Add-to-Playlist button it replaces. 44px hit target. -->
					<button
						bind:this={menuButtonEl}
						type="button"
						class="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-md text-text-primary transition-transform active:scale-95 active:bg-surface-2"
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
						{#if release.tracks.length === 0}
							<!-- No tracks yet: either fetching them (auto on open / manual refresh) — show skeleton
						     rows shaped like real tracks — or the source genuinely had none. -->
							{#if isRefreshing}
								<div role="status" aria-label={$translate('discovery.fetchingMetadata')}>
									<TrackListSkeleton />
								</div>
							{:else}
								<div class="py-3 text-sm text-text-tertiary">{$translate('library.noTracksYet')}</div>
							{/if}
						{/if}
						{#each release.tracks as track, index (track.id)}
							{@const isActive = isCurrentRelease && $previewInfo?.trackIndex === index}
							{@const isLoading = spinnerArmed && loadingReleaseId === release.id && loadingTrackIndex === index}
							<!-- A tap plays the track; a long-press lifts the row and opens its context menu (Play next /
						     Add to queue). The heart floats on top (absolute) so the whole row shares the same pressed
						     highlight, yet tapping the heart likes the track instead of playing it. A track the source
						     serves no preview for (pre-order) renders greyed-out and inert — the heart still works. -->
							<div
								class="relative rounded {isActive ? 'bg-brand-muted' : ''}"
								onpointerdown={(e) => trackPlayable(track) && startTrackLongPress(e, index)}
								onclickcapture={onTrackClickCapture}
							>
								<button
									type="button"
									class="flex min-h-[44px] w-full min-w-0 items-center gap-3 rounded py-2 pr-10 pl-2 text-left active:bg-surface-2 disabled:opacity-40 disabled:active:bg-transparent"
									aria-label={$translate('discovery.playPreview')}
									disabled={!trackPlayable(track)}
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

	<ContextMenuItem onclick={menuPlayNext}>
		{$translate('queue.playNext')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
				<path d="M5 5l11 7-11 7z" />
				<rect x="17.5" y="5" width="2" height="14" rx="1" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={menuAddToQueue}>
		{$translate('queue.addToQueue')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M4 6h11M4 12h11M4 18h7M19 14v6M16 17h6" stroke-linecap="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	{#if canFollow}
		<ContextMenuItem onclick={menuFollow}>
			{$translate('discovery.following.follow')}
			{#snippet icon()}
				<svg
					class="h-5 w-5"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					<path d="M5 12a7 7 0 0 1 7 7" />
					<path d="M5 5a14 14 0 0 1 14 14" />
					<circle cx="5.5" cy="18.5" r="1.5" fill="currentColor" stroke="none" />
				</svg>
			{/snippet}
		</ContextMenuItem>
	{/if}

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

	<ContextMenuItem separatorBefore onclick={menuRefresh}>
		{$translate('discovery.refreshMetadata')}
		{#snippet icon()}
			<svg
				class="h-5 w-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M21 12a9 9 0 1 1-2.64-6.36" />
				<path d="M21 3v6h-6" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={menuOpenInSource}>
		{platformName
			? $translate('discovery.openInApp', { values: { app: platformName } })
			: $translate('discovery.openInBrowser')}
		{#snippet icon()}
			<SourceIcon source={release.source_type} />
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={menuShare}>
		{$translate('discovery.share')}
		{#snippet icon()}
			<svg
				class="h-5 w-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M12 3v12M8 7l4-4 4 4" />
				<path d="M5 12v7a2 2 0 002 2h10a2 2 0 002-2v-7" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={menuCopyUrl}>
		{$translate('discovery.copyUrl')}
		{#snippet icon()}
			<svg
				class="h-5 w-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<rect x="9" y="9" width="11" height="11" rx="2" />
				<path d="M5 15V5a2 2 0 012-2h10" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	{#if release.tracks.length > 0 && !isFullyDownloaded}
		<ContextMenuItem separatorBefore onclick={menuDownloadForOffline}>
			{downloading ? $translate('discovery.downloading') : $translate('discovery.downloadForOffline')}
			{#snippet icon()}
				<svg
					class="h-5 w-5"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
					<path d="M7 10l5 5 5-5" />
					<path d="M12 15V3" />
				</svg>
			{/snippet}
		</ContextMenuItem>
	{/if}

	{#if hasSomeCached}
		<ContextMenuItem separatorBefore={isFullyDownloaded} onclick={menuRemoveDownload}>
			{$translate('discovery.removeDownload')}
			{#snippet icon()}
				<svg
					class="h-5 w-5"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
				>
					<path d="M3 6h18M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2m2 0v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6" />
				</svg>
			{/snippet}
		</ContextMenuItem>
	{/if}

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

<!-- Per-track queue actions (opened by long-pressing a track row). Lifts a preview of the row, mirroring
     the release feed / playlist context menus. -->
<ContextMenu
	open={trackMenuOpen}
	anchorRect={trackMenuRect}
	onClose={() => (trackMenuOpen = false)}
	onClosed={() => {
		actionTrackIndex = null
		trackMenuRect = null
	}}
>
	{#snippet preview()}
		{#if actionTrack}
			<span class="w-5 flex-shrink-0 text-center text-xs text-text-tertiary tabular-nums">
				{(actionTrackIndex ?? 0) + 1}
			</span>
			<span class="min-w-0 flex-1 truncate text-sm text-text-primary">{actionTrack.name}</span>
			{#if actionTrack.duration_ms != null}
				<span class="flex-shrink-0 text-xs text-text-tertiary tabular-nums">
					{formatDurationCompact(actionTrack.duration_ms)}
				</span>
			{/if}
		{/if}
	{/snippet}

	<ContextMenuItem onclick={queuePlayNext}>
		{$translate('queue.playNext')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
				<path d="M5 5l11 7-11 7z" />
				<rect x="17.5" y="5" width="2" height="14" rx="1" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={queueAddLast}>
		{$translate('queue.addToQueue')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M4 6h11M4 12h11M4 18h7M19 14v6M16 17h6" stroke-linecap="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem separatorBefore onclick={trackShare}>
		{$translate('discovery.share')}
		{#snippet icon()}
			<svg
				class="h-5 w-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<path d="M12 3v12M8 7l4-4 4 4" />
				<path d="M5 12v7a2 2 0 002 2h10a2 2 0 002-2v-7" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={trackCopyUrl}>
		{$translate('discovery.copyUrl')}
		{#snippet icon()}
			<svg
				class="h-5 w-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
			>
				<rect x="9" y="9" width="11" height="11" rx="2" />
				<path d="M5 15V5a2 2 0 012-2h10" />
			</svg>
		{/snippet}
	</ContextMenuItem>
</ContextMenu>
