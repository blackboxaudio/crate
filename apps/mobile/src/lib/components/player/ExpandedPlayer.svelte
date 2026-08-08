<script lang="ts">
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { writeText } from '@tauri-apps/plugin-clipboard-manager'
	import { shareUrl } from '$shared/api/app'
	import { toastStore } from '$shared/stores/toast'
	import { slide, fade, type TransitionConfig } from 'svelte/transition'
	import { easeFluid } from '$lib/easing'
	import { translate } from '$shared/i18n'
	import {
		playerStore,
		previewInfo,
		isPlaying,
		previewLoading,
		playbackPosition,
		playbackDuration,
		playbackSpeed,
		shuffleEnabled,
		repeatMode,
	} from '$shared/stores/player'
	import { canAdvance, upNext, peekUpcoming, peekPrevious, type Pick as QueuePick } from '$shared/stores/playbackQueue'
	import { discoveryStore } from '$shared/stores/discovery'
	import { formatDuration } from '$shared/utils/format'
	import { getReleasePlatformName } from '$shared/utils/discoveryLinks'
	import { isIOS } from '$shared/utils/platform'
	import { getDiscoveryArtworkSrc } from '$shared/utils/artwork'
	import { cacheReleaseArtwork } from '$shared/api/discovery'
	import { mobileAppDataDir } from '$lib/stores/appData'
	import { mobileUIStore, isPlayerExpanded } from '$lib/stores/mobileUI'
	import { lightTap } from '$lib/utils/haptics'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import CoverPager, { type TrackChangeFx } from './CoverPager.svelte'
	import Slider from '$shared/components/Slider.svelte'
	import Spinner from '$lib/components/common/Spinner.svelte'
	import ContextMenu from '$lib/components/common/ContextMenu.svelte'
	import ContextMenuItem from '$lib/components/common/ContextMenuItem.svelte'
	import SourceIcon from '../discovery/SourceIcon.svelte'
	import EditReleaseSheet from '../discovery/EditReleaseSheet.svelte'
	import PlaylistPickerSheet from '$lib/components/playlists/PlaylistPickerSheet.svelte'
	import UpNextSheet from './UpNextSheet.svelte'

	// Full-screen preview player: large artwork over a blurred album-art wash, an interactive scrubber,
	// prev / play-pause / next transport, a like toggle, and a tempo (±10% speed) control. Slide / scrim /
	// drag-to-dismiss come from the shared `Drawer` baseline (direction="bottom"); this stays mounted while
	// a preview exists and opens when `$isPlayerExpanded`. Reads/writes the shared playerStore so it stays
	// in sync with the mini-player and the OS media session.
	const track = $derived($previewInfo ? $previewInfo.release.tracks[$previewInfo.trackIndex] : null)

	// Cache-first cover for both the blurred wash and the foreground art: prefers the on-disk
	// cached copy (renders offline) and falls back to the remote URL, downloading it to disk on
	// first display so it's cached next time. Local state so the download flips the src without a
	// store round-trip; reset only when the release IDENTITY changes — the store's release object
	// never learns the cached path, so unconditionally re-seeding from it on every effect run
	// clobbered the resolved path back to null and re-invoked cacheReleaseArtwork, which resolved
	// and re-triggered the effect: an infinite IPC loop (thousands of invokes/sec, each touching
	// the DB) that pegged the CPU and got the app killed by iOS. Mirrors ReleaseArtwork.svelte.
	let artCachePath = $state<string | null>(null)
	let artReleaseId = $state<string | null>(null)
	$effect(() => {
		const rel = $previewInfo?.release
		if (!rel) {
			artReleaseId = null
			artCachePath = null
			return
		}
		if (rel.id !== artReleaseId) {
			artReleaseId = rel.id
			artCachePath = rel.artwork_cache_path
		} else if (rel.artwork_cache_path && !artCachePath) {
			artCachePath = rel.artwork_cache_path
		}
		if (!artCachePath && rel.artwork_url) {
			const id = rel.id
			void cacheReleaseArtwork(id).then((path) => {
				if (path && $previewInfo?.release.id === id) artCachePath = path
			})
		}
	})
	const artSrc = $derived(
		$previewInfo
			? getDiscoveryArtworkSrc(
					{ artwork_url: $previewInfo.release.artwork_url, artwork_cache_path: artCachePath },
					$mobileAppDataDir
				)
			: undefined
	)
	// Loading covers both the initial stream fetch (before `previewInfo` is set — e.g. tapping a track from
	// idle, which expands this player immediately) and mid-playback buffering / speed re-buffering.
	const loading = $derived(
		$previewLoading != null && ($previewInfo == null || $previewLoading.releaseId === $previewInfo.releaseId)
	)
	// Next is available when the two-tier queue can produce another track — a user-queue item, a forward
	// step, or more context (shuffle pick / next-or-cross-release). The model computes it for us.
	const canNext = $derived($previewInfo != null && $canAdvance)

	// Mode-specific accessible name for the cycling repeat button (its icon is the visual state).
	const repeatLabel = $derived(
		$repeatMode === 'track'
			? $translate('player.repeatTrack')
			: $repeatMode === 'release'
				? $translate('player.repeatRelease')
				: $repeatMode === 'context'
					? $translate('player.repeatAll')
					: $translate('player.repeat')
	)

	const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

	// --- track-change transitions -------------------------------------------------------------------
	// Everything animates off `$previewInfo` changes (never off the gesture directly), so the transport
	// buttons, auto-advance, UpNext taps, and iOS lock-screen skips — which land asynchronously via the
	// native engine's onTrackChanged — all run the identical transition. The swipe pager only registers
	// pending gestures before driving the player; the watcher consumes them to attribute each change.
	// A FIFO (not a single slot) because stream resolution can take seconds and rapid swipes may queue
	// several page requests before the first change lands — each landing consumes its matching head.
	let pendingGestures = $state<Array<{ dir: 1 | -1; targetKey: string | null }>>([])
	let changeFx = $state<TrackChangeFx | null>(null)
	let fxSeq = 0
	// Untracked snapshot of the last-seen track (and its rendered cover src, kept fresh as the
	// cache-first resolution flips remote → local) so a change knows what it's transitioning FROM.
	let lastTrack: { releaseId: string; trackIndex: number; artSrc: string | undefined } | null = null

	// `$effect.pre` runs BEFORE the DOM updates, so the keyed background/title blocks re-render with the
	// fresh `changeFx` and their in/out transitions get the right direction.
	$effect.pre(() => {
		const info = $previewInfo
		if (!info) {
			lastTrack = null
			return
		}
		if (lastTrack && lastTrack.releaseId === info.releaseId && lastTrack.trackIndex === info.trackIndex) {
			lastTrack.artSrc = artSrc
			return
		}
		if (lastTrack) {
			const kind = lastTrack.releaseId === info.releaseId ? 'same' : 'cross'
			const head = pendingGestures[0]
			let viaGesture = false
			let dir: 1 | -1
			if (head && (head.targetKey === null || head.targetKey === `${info.releaseId}:${info.trackIndex}`)) {
				// This change is the oldest in-flight swipe landing — consume it, keep the rest queued.
				viaGesture = true
				dir = head.dir
				pendingGestures = pendingGestures.slice(1)
			} else {
				if (kind === 'same') {
					// Wrapping last → first still reads as "next"; otherwise the index order is the direction.
					const wrapped = info.trackIndex === 0 && lastTrack.trackIndex === info.release.tracks.length - 1
					dir = wrapped || info.trackIndex > lastTrack.trackIndex ? 1 : -1
				} else {
					dir = 1
				}
				// An unrelated change (UpNext tap, lock-screen skip) invalidates any queued swipes.
				if (pendingGestures.length > 0) pendingGestures = []
			}
			changeFx = { kind, dir, viaGesture, outgoingSrc: lastTrack.artSrc, seq: ++fxSeq }
		}
		lastTrack = { releaseId: info.releaseId, trackIndex: info.trackIndex, artSrc }
	})

	// Neighbor picks for the pager's peeking covers, re-peeked on every queue emission ($upNext re-emits
	// on each mutation/advance) so they always match what a swipe would actually play.
	const nextPick = $derived.by(() => {
		void $upNext
		return $previewInfo ? (peekUpcoming(1)[0] ?? null) : null
	})
	const prevPick = $derived.by(() => {
		void $upNext
		return $previewInfo ? peekPrevious() : null
	})
	// "Previous" pages only when a previous pick exists — a swipe never falls into the restart branch.
	const canPrevPage = $derived(prevPick != null)

	function requestPage(dir: 1 | -1, target: QueuePick | null) {
		pendingGestures = [
			...pendingGestures,
			{ dir, targetKey: target ? `${target.release.id}:${target.trackIndex}` : null },
		]
		if (dir === 1) void playerStore.nextTrack()
		else void playerStore.previousTrack({ skipRestartThreshold: true })
	}
	function onSettleTimeout() {
		pendingGestures = []
	}

	// Title/artist text transitions, per line (each line sits in its own overflow-hidden mask, and the
	// artist trails the title by a beat for a cascade feel). Same-release changes do a masked "ticker
	// roll": the old line rolls out of the clip while the new one rolls in from the opposite edge, up
	// for next / down for previous. Cross-release changes slide with the change's direction while
	// dissolving. Transform + opacity ONLY — an animated `filter: blur()` re-rasterizes the text every
	// frame in WKWebView. Params are read when the transition fires, so outros pick up the direction
	// of the change that replaced them. Reduced motion → instant.
	function textIn(node: Element, { delay = 0 }: { delay?: number } = {}): TransitionConfig {
		const fx = changeFx
		if (reducedMotion || !fx) return { duration: 0 }
		if (fx.kind === 'same') {
			const h = node.clientHeight
			return {
				delay,
				duration: 340,
				easing: easeFluid,
				css: (t, u) => `transform: translateY(${(u * h * fx.dir).toFixed(2)}px); opacity: ${t}`,
			}
		}
		return {
			delay,
			duration: 360,
			easing: easeFluid,
			css: (t, u) => `transform: translateX(${(u * fx.dir * 32).toFixed(2)}px); opacity: ${t}`,
		}
	}
	function textOut(node: Element, { delay = 0 }: { delay?: number } = {}): TransitionConfig {
		const fx = changeFx
		if (reducedMotion || !fx) return { duration: 0 }
		if (fx.kind === 'same') {
			const h = node.clientHeight
			return {
				delay,
				duration: 340,
				easing: easeFluid,
				css: (t, u) => `transform: translateY(${(u * -h * fx.dir).toFixed(2)}px); opacity: ${t}`,
			}
		}
		return {
			delay,
			duration: 240,
			easing: easeFluid,
			css: (t, u) => `transform: translateX(${(u * fx.dir * -24).toFixed(2)}px); opacity: ${t}`,
		}
	}
	// The blurred wash crossfades only across releases (it's keyed by releaseId, so same-release changes
	// never touch it). A short opacity-only fade is kept under reduced motion.
	const bgFadeMs = reducedMotion ? 150 : 600

	// Tempo reveal, staged in two beats: opening, the row's height animates first (carrying the transport
	// above it up) and its contents fade in as that settles; closing runs the reverse — contents fade out,
	// then the height collapses. Hence the separate in/out slides: the outro needs the fade's delay.
	const tempoSlideMs = reducedMotion ? 0 : 220
	const tempoFadeMs = reducedMotion ? 0 : 140
	const tempoFadeDelay = reducedMotion ? 0 : 130
	const tempoCollapseMs = reducedMotion ? 0 : 110

	// Scrubbing: while the user drags the slider, show the local value and only commit on release so the
	// live position updates don't fight the thumb.
	let scrubbing = $state(false)
	let scrubValue = $state(0)
	const sliderValue = $derived(scrubbing ? scrubValue : $playbackPosition)

	function onScrubInput(e: Event) {
		scrubbing = true
		scrubValue = Number((e.target as HTMLInputElement).value)
	}
	function onScrubCommit() {
		void playerStore.seek(scrubValue)
		scrubbing = false
	}

	// Tempo: a ±10% bipolar speed fader (the shared Slider, same as the desktop TempoControl), revealed by
	// the metronome toggle in the transport row. The Slider's snapToCenter detents the fader at exactly
	// 1.0x. On iOS the native engine applies the rate immediately via setSpeed→setRate; on the HTML5 path
	// we commit on release. Reset snaps to 1.0x.
	let showTempo = $state(false)
	const tempoPct = $derived(Math.round(($playbackSpeed - 1) * 1000) / 10)

	// The "Up Next" sheet, opened from the queue button at the transport row's right edge.
	let showQueue = $state(false)

	// Always reopen with the tempo fader hidden + queue sheet closed: reset them whenever the player
	// collapses. Covers every close path (drag-dismiss, programmatic collapse, the preview ending) since
	// they all flip `$isPlayerExpanded` false. The metronome / queue toggles re-reveal for the session.
	$effect(() => {
		if (!$isPlayerExpanded) {
			showTempo = false
			showQueue = false
			menuOpen = false
			pendingGestures = []
		}
	})

	function onTempoInput(e: Event) {
		void playerStore.setSpeed(1 + parseFloat((e.target as HTMLInputElement).value) / 100)
	}
	function onTempoCommit(e: Event) {
		void playerStore.setSpeed(1 + parseFloat((e.target as HTMLInputElement).value) / 100)
		if (!isIOS()) playerStore.commitPreviewSpeed()
	}
	function resetTempo() {
		void playerStore.setSpeed(1)
		if (!isIOS()) playerStore.commitPreviewSpeed()
	}

	function toggleLike() {
		if ($previewInfo && track) void discoveryStore.toggleTrackLiked($previewInfo.releaseId, track.id)
	}

	// Overflow (⋯) menu: an iOS-style context-menu platter anchored to the "more" button beside Like,
	// gathering release-level actions for the currently-playing preview. Tap-triggered, so the platter just
	// springs from the button with no lifted preview. Add-to-playlist / Edit open their own bottom sheets.
	let menuOpen = $state(false)
	let menuButtonEl = $state<HTMLButtonElement | null>(null)
	let menuAnchor = $state<{ top: number; left: number; width: number; height: number } | null>(null)
	let editSheetOpen = $state(false)
	let playlistPickerOpen = $state(false)

	const platformName = $derived($previewInfo ? getReleasePlatformName($previewInfo.release.source_type) : null)

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
	// Go to release: collapse the player and open the release's detail screen — same as tapping the title.
	function menuGoToRelease() {
		menuOpen = false
		if ($previewInfo) mobileUIStore.locateRelease($previewInfo.releaseId)
	}
	function menuEdit() {
		menuOpen = false
		editSheetOpen = true
	}
	function menuOpenInSource() {
		menuOpen = false
		if ($previewInfo) void openUrl($previewInfo.release.url).catch(() => {})
	}
	function menuShare() {
		menuOpen = false
		const info = $previewInfo
		if (!info) return
		// The OS share sheet is the feedback — no toast.
		void shareUrl(info.release.url, info.release.title ?? undefined).catch(() => {})
	}
	async function menuCopyUrl() {
		menuOpen = false
		const info = $previewInfo
		if (!info) return
		try {
			await writeText(info.release.url)
			// Exception to the sparing-toasts rule: a clipboard write has no other visible feedback.
			toastStore.info($translate('discovery.copiedUrl'))
		} catch {
			// Clipboard denied — nothing useful to surface.
		}
	}
</script>

<Drawer
	direction="bottom"
	open={$isPlayerExpanded && $previewInfo != null}
	onClose={mobileUIStore.collapsePlayer}
	z={50}
	scrimDismiss={false}
	ariaLabel={$previewInfo?.release.title ?? $translate('common.untitled')}
	class="h-full overflow-hidden bg-surface-0"
>
	{#snippet children({ dragging })}
		{#if $previewInfo}
			<!-- Album-art background: a blurred, slowly drifting wash with a theme-aware legibility scrim.
			     Full-bleed and first in the DOM; the relative content wrapper below paints over it. Keyed by
			     the RELEASE id: same-release track changes never touch it, while a cross-release change
			     cross-dissolves the whole layer (slowly, against the cover's fast slide — reads as parallax).
			     The fresh keyed img restarts the Ken-Burns drift at its base pose, so there's no jump. -->
			{#key $previewInfo.releaseId}
				<div
					class="pointer-events-none absolute inset-0"
					in:fade={{ duration: bgFadeMs, easing: easeFluid }}
					out:fade={{ duration: bgFadeMs, easing: easeFluid }}
				>
					{#if artSrc}
						<!-- Drift on the unfiltered wrapper, blur + static base scale on the img — see the
						     .art-wash note in style.css for why the two must not share an element. -->
						<div class="art-wash absolute inset-0">
							<img src={artSrc} alt="" class="art-wash-img h-full w-full object-cover blur-2xl" decoding="async" />
						</div>
						<div class="absolute inset-0 bg-gradient-to-b from-surface-0/80 via-surface-0/25 to-surface-0/90"></div>
					{/if}
				</div>
			{/key}

			<div class="pt-safe pb-safe relative flex h-full flex-col">
				<!-- Drag handle: hidden until dragging, near the top; collapse is by dragging the sheet down. -->
				<span
					class="pointer-events-none absolute top-2 left-1/2 h-1 w-10 -translate-x-1/2 rounded-full bg-text-primary/40 transition-opacity duration-200"
					style="opacity: {dragging ? 1 : 0}"
				></span>

				<!-- Artwork: a swipeable 3-slot cover strip. Swipe left/right pages next/previous with the
				     neighboring covers peeking in; store-driven changes (buttons, auto-advance, lock screen)
				     slide through the same strip. Vertical drags still dismiss the sheet (JS axis-lock). -->
				<CoverPager
					{artSrc}
					{changeFx}
					currentKey={`${$previewInfo.releaseId}:${$previewInfo.trackIndex}`}
					{prevPick}
					{nextPick}
					{canNext}
					canPrev={canPrevPage}
					enabled={$isPlayerExpanded && !dragging}
					onRequestPage={requestPage}
					{onSettleTimeout}
				/>

				<!-- Track info + transport. No parent gap/space-y: each row carries its own `mt-4`, so the
				     tempo control's top margin belongs to the sliding element itself and Svelte's `slide`
				     animates it away with the height. A selector-based gap/space-y collapses un-animated when
				     the tempo row unmounts, which jumped the control box. -->
				<div class="flex flex-col px-4 pb-3">
					<div class="flex items-center gap-3">
						<!-- Grid-stacked so the outgoing and incoming title layers overlap while they change. Each
						     line lives in its own overflow-hidden mask and transitions per line, artist a beat after
						     the title: same-release changes ticker-roll within the clip (up for next, down for
						     previous), cross-release ones slide with the change's direction through a blur dissolve.
						     The like button stays outside the keyed block (its state updates in place). -->
						<div class="grid min-w-0 flex-1">
							{#key `${$previewInfo.releaseId}:${$previewInfo.trackIndex}`}
								<div class="col-start-1 row-start-1 flex min-w-0 flex-col">
									<div class="overflow-hidden">
										<!-- Tap the title to locate the release: collapse the player, scroll the feed to it (behind
										     the overlay), and open its detail screen. Desktop parity with the player's title locate. -->
										<button
											type="button"
											class="block max-w-full truncate text-left text-xl font-semibold text-text-primary active:opacity-60"
											onclick={() => $previewInfo && mobileUIStore.locateRelease($previewInfo.releaseId)}
											in:textIn
											out:textOut
										>
											{track?.name ?? $previewInfo.release.title ?? $translate('common.untitled')}
										</button>
									</div>
									<div class="overflow-hidden">
										<span
											class="block truncate text-base text-text-secondary"
											in:textIn={{ delay: 60 }}
											out:textOut={{ delay: 40 }}
										>
											{$previewInfo.release.artist ?? $translate('common.unknownArtist')}
										</span>
									</div>
								</div>
							{/key}
						</div>
						<button
							type="button"
							class="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
							aria-label={track?.is_liked ? $translate('discovery.unlike') : $translate('discovery.like')}
							onclick={toggleLike}
						>
							<svg
								class="h-6 w-6 {track?.is_liked ? 'text-brand-primary' : ''}"
								viewBox="0 0 24 24"
								fill={track?.is_liked ? 'currentColor' : 'none'}
								stroke="currentColor"
								stroke-width="2"
							>
								<path
									d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78L12 21.23l8.84-8.84a5.5 5.5 0 0 0 0-7.78z"
								/>
							</svg>
						</button>
						<!-- Overflow menu: opens the release-actions context menu. Lives beside Like (Apple-Music
						     style) so the transport row below keeps two accessories per side — the repeat button
						     took the row slot this used to occupy. Horizontal dots, matching ReleaseDetail. -->
						<button
							bind:this={menuButtonEl}
							type="button"
							class="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-md text-text-primary transition-colors active:bg-surface-2"
							aria-label={$translate('common.more')}
							aria-haspopup="menu"
							aria-expanded={menuOpen}
							onclick={openMenu}
						>
							<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor">
								<circle cx="5" cy="12" r="1.6" />
								<circle cx="12" cy="12" r="1.6" />
								<circle cx="19" cy="12" r="1.6" />
							</svg>
						</button>
					</div>

					<!-- Scrubber (unipolar fill; stop pointerdown from reaching the sheet's drag gesture). -->
					<div class="mt-4 flex flex-col gap-1">
						<Slider
							value={sliderValue}
							min={0}
							max={$playbackDuration || 0}
							hitSize={20}
							activeScale={1.4}
							ariaLabel={$translate('player.seek')}
							oninput={onScrubInput}
							onchange={onScrubCommit}
							onpointerdown={(e) => e.stopPropagation()}
						/>
						<div class="flex justify-between text-xs text-text-tertiary tabular-nums">
							<span>{formatDuration(sliderValue)}</span>
							<span>{formatDuration($playbackDuration)}</span>
						</div>
					</div>

					<!-- Transport: one justify-between row — shuffle · repeat · [prev/play/next] · tempo · queue.
					     The prev/play/next box is a single fixed item in the middle; with two equal-width
					     accessories on each side, justify-between spaces everything evenly AND keeps that box (so
					     the play button) dead-centered. That two-per-side balance is why the overflow menu moved
					     up beside Like when repeat arrived — a third accessory on one side would de-center play. -->
					<div class="mt-4 flex items-center justify-between">
						<button
							type="button"
							class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md transition-colors active:bg-surface-2 {$shuffleEnabled
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-primary'}"
							aria-label={$translate('player.shuffle')}
							aria-pressed={$shuffleEnabled}
							onclick={() => playerStore.toggleShuffle()}
						>
							<svg
								class="h-5 w-5"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
							>
								<!-- Tabler Icons "arrows-shuffle" (MIT) -->
								<path d="M18 4l3 3l-3 3" />
								<path d="M18 20l3 -3l-3 -3" />
								<path d="M3 7h3a5 5 0 0 1 5 5a5 5 0 0 0 5 5h4" />
								<path d="M21 7h-4a4.978 4.978 0 0 0 -3 1m-4 8a4.984 4.984 0 0 1 -3 1h-4" />
							</svg>
						</button>
						<button
							type="button"
							class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md transition-colors {$repeatMode !==
							'off'
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-primary active:bg-surface-2'}"
							aria-label={repeatLabel}
							aria-pressed={$repeatMode !== 'off'}
							onclick={() => playerStore.cycleRepeatMode()}
						>
							<svg
								class="h-5 w-5"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
							>
								<!-- Tabler Icons "repeat" / "repeat-once" (MIT); the center dot marks repeat-release
								     (no standard glyph distinguishes release from the full context). -->
								<path d="M4 12v-3a3 3 0 0 1 3 -3h13m-3 -3l3 3l-3 3" />
								<path d="M20 12v3a3 3 0 0 1 -3 3h-13m3 3l-3 -3l3 -3" />
								{#if $repeatMode === 'track'}
									<path d="M11 11l1 -1v4" />
								{:else if $repeatMode === 'release'}
									<circle cx="12" cy="12" r="1.6" fill="currentColor" stroke="none" />
								{/if}
							</svg>
						</button>

						<!-- Main controls: their own box — previous / play / next, evenly spaced around play. Fixed
						     size + width (flex-shrink-0); only play is a circle. -->
						<div class="flex flex-shrink-0 items-center gap-6">
							<button
								type="button"
								class="flex h-12 w-12 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
								aria-label={$translate('player.previous')}
								onclick={() => playerStore.previousTrack()}
							>
								<svg class="h-7 w-7" viewBox="0 0 24 24" fill="currentColor">
									<path d="M6 6h2v12H6zM20 6v12L9 12z" />
								</svg>
							</button>
							<button
								type="button"
								class="flex h-16 w-16 items-center justify-center rounded-full bg-brand-primary text-white active:opacity-80"
								aria-label={$isPlaying ? $translate('player.pause') : $translate('player.play')}
								onclick={() => playerStore.togglePlayPause()}
							>
								{#if loading}
									<Spinner class="h-8 w-8" />
								{:else if $isPlaying}
									<svg class="h-8 w-8" viewBox="0 0 24 24" fill="currentColor"
										><path d="M6 5h4v14H6zM14 5h4v14h-4z" /></svg
									>
								{:else}
									<svg class="h-8 w-8" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z" /></svg>
								{/if}
							</button>
							<button
								type="button"
								class="flex h-12 w-12 items-center justify-center rounded-md text-text-primary active:bg-surface-2"
								aria-label={$translate('player.next')}
								onclick={() => playerStore.nextTrack()}
							>
								<svg class="h-7 w-7" viewBox="0 0 24 24" fill="currentColor">
									<path d="M16 6h2v12h-2zM4 6l11 6L4 18z" />
								</svg>
							</button>
						</div>

						<button
							type="button"
							class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md transition-colors active:bg-surface-2 {showTempo
								? 'bg-brand-muted text-brand-primary'
								: 'text-text-primary'}"
							aria-label={$translate('player.tempo')}
							aria-pressed={showTempo}
							onclick={() => (showTempo = !showTempo)}
						>
							<svg
								class="h-5 w-5"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								stroke-width="2"
								stroke-linecap="round"
								stroke-linejoin="round"
							>
								<!-- Tabler Icons "metronome" (MIT) -->
								<path
									d="M14.153 8.188l-.72 -3.236a2.493 2.493 0 0 0 -4.867 0l-3.025 13.614a2 2 0 0 0 1.952 2.434h7.014a2 2 0 0 0 1.952 -2.434l-.524 -2.357m-4.935 1.791l9 -13"
								/>
								<path d="M19 5a1 1 0 1 0 2 0a1 1 0 1 0 -2 0" />
							</svg>
						</button>
						<!-- Up Next sheet trigger. Flush to the right edge, mirroring shuffle on the left so the
						     row stays symmetric. -->
						<button
							type="button"
							class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md text-text-primary transition-colors active:bg-surface-2"
							aria-label={$translate('queue.openQueue')}
							onclick={() => (showQueue = true)}
						>
							<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<!-- Queue: list lines + a play triangle. -->
								<path d="M4 6h16M4 12h16M4 18h9" stroke-linecap="round" />
								<path d="M15 16.5l5 2.5-5 2.5z" fill="currentColor" stroke="none" />
							</svg>
						</button>
					</div>

					<!-- Tempo (±10% speed fader): revealed by the metronome toggle. The row's HEIGHT animates, so
					     the transport above it rides up with the reveal instead of jumping in one frame the way a
					     transform-only fly did. Affordable here because the cover pager's tile is width-driven
					     (`aspect-square w-full`): the shrinking flex-1 box reflows the column without rescaling any
					     artwork. The slider is bipolar — its fill grows out from the centre (0%) toward the thumb,
					     with a small detent at zero. The readout on the left balances the reset on the right. -->
					{#if showTempo}
						<div
							class="mt-4"
							in:slide={{ duration: tempoSlideMs, easing: easeFluid }}
							out:slide={{ duration: tempoSlideMs, delay: tempoCollapseMs, easing: easeFluid }}
						>
							<div
								class="flex items-center gap-3"
								in:fade={{ duration: tempoFadeMs, delay: tempoFadeDelay, easing: easeFluid }}
								out:fade={{ duration: tempoCollapseMs, easing: easeFluid }}
							>
								<span class="w-12 flex-shrink-0 text-right text-xs text-text-secondary tabular-nums">
									{tempoPct >= 0 ? '+' : ''}{tempoPct.toFixed(1)}%
								</span>
								<div class="flex flex-1 items-center">
									<Slider
										value={tempoPct}
										min={-10}
										max={10}
										step={0.1}
										bipolar
										snapToCenter={0.5}
										hitSize={20}
										activeScale={1.4}
										ariaLabel={$translate('player.tempo')}
										oninput={onTempoInput}
										onchange={onTempoCommit}
										onpointerdown={(e) => e.stopPropagation()}
									/>
								</div>
								<div class="flex w-12 flex-shrink-0 justify-start">
									<button
										type="button"
										class="flex h-8 w-8 items-center justify-center rounded-md text-text-secondary active:bg-surface-2 disabled:opacity-30"
										aria-label={$translate('player.resetTempo')}
										disabled={tempoPct === 0}
										onclick={resetTempo}
									>
										<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
											<path
												d="M3 2v6h6M3.51 15a9 9 0 1 0 .49-9.36L3 8"
												stroke-linecap="round"
												stroke-linejoin="round"
											/>
										</svg>
									</button>
								</div>
							</div>
						</div>
					{/if}
				</div>
			</div>
		{/if}
	{/snippet}
</Drawer>

<UpNextSheet open={showQueue} onClose={() => (showQueue = false)} />

{#if $previewInfo}
	<PlaylistPickerSheet
		open={playlistPickerOpen}
		releaseIds={[$previewInfo.releaseId]}
		onClose={() => (playlistPickerOpen = false)}
	/>
	<EditReleaseSheet open={editSheetOpen} release={$previewInfo.release} onClose={() => (editSheetOpen = false)} />
{/if}

<!-- Release-actions "more" menu (opened by the ⋯ button beside Like). Tap-triggered, so no lifted preview. -->
<ContextMenu open={menuOpen} anchorRect={menuAnchor} tapTriggered onClose={() => (menuOpen = false)}>
	<ContextMenuItem onclick={menuAddToPlaylist}>
		{$translate('contextMenu.addToPlaylist')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M12 5v14M5 12h14" stroke-linecap="round" />
			</svg>
		{/snippet}
	</ContextMenuItem>

	<ContextMenuItem onclick={menuGoToRelease}>
		{$translate('discovery.goToRelease')}
		{#snippet icon()}
			<svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M9 6l6 6-6 6" stroke-linecap="round" stroke-linejoin="round" />
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
			{#if $previewInfo}<SourceIcon source={$previewInfo.release.source_type} />{/if}
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
</ContextMenu>
