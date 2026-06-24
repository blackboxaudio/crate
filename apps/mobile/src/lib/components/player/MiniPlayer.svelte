<script lang="ts">
	import { fly } from 'svelte/transition'
	import { easeFluid } from '$lib/easing'
	import { translate } from '$shared/i18n'
	import { playerStore, previewInfo, isPlaying, previewLoading, playbackProgress } from '$shared/stores/player'
	import {
		mobileUIStore,
		detailCovering,
		playlistDetailCovering,
		tagDetailCovering,
		followDetailCovering,
	} from '$lib/stores/mobileUI'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// Persistent mini-player for discovery preview playback: a floating liquid-glass card (Spotify-style)
	// that sits just above the bottom tab bar, visually separated from it by a gap, side margins, rounded
	// corners, and a lift shadow. Tap the card to open the full-screen ExpandedPlayer; the play/pause button
	// toggles playback without expanding. There is intentionally no drag handle / swipe-up — tapping is the
	// only way up. A thin progress line runs along the bottom edge. Renders only while a preview is active;
	// lives at the app root so it persists across the feed ↔ release-detail navigation.
	const track = $derived($previewInfo ? $previewInfo.release.tracks[$previewInfo.trackIndex] : null)
	// Loading covers the initial stream fetch (before `previewInfo` is set) as well as mid-playback
	// buffering, so the spinner shows the moment a track is tapped, not only once playback is ready.
	const loading = $derived(
		$previewLoading != null && ($previewInfo == null || $previewLoading.releaseId === $previewInfo.releaseId)
	)

	// Float above the bottom tab bar (its 3.5rem + safe-area height) on the main shell, with a small gap.
	// When a full-screen detail overlay covers the tab bar — the release detail, or a playlist / tag /
	// followed-source drill-in — drop to float just above the bottom safe-area instead; otherwise it would
	// hang 3.5rem up, leaving a gap over the now-covered tab bar. Tracks the *covering* flags (not the ids),
	// which drop the instant a close starts — so it rises back over the tab bar *as* the detail slides out.
	const overDetail = $derived($detailCovering || $playlistDetailCovering || $tagDetailCovering || $followDetailCovering)

	// Tap handling for this fixed card. iOS WebKit defers `click` dispatch to a fixed element like this
	// one while the discovery feed coasts to a stop from a momentum ("flick") scroll — the same deferral
	// the tab bar had — so a tap mid-scroll felt unacknowledged. Run the action on pointer-DOWN for touch
	// (it fires on contact, the touch that also halts the scroll), then swallow the trailing compatibility
	// `click` so the action doesn't run twice (which would un-toggle play/pause). Mouse, pen, and
	// keyboard/VoiceOver fall through to the `click` path for natural press semantics + activation a11y.
	let touchHandled = false
	function tapDown(e: PointerEvent, action: () => void) {
		if (e.pointerType !== 'touch') return
		touchHandled = true
		action()
	}
	function tapClick(action: () => void) {
		if (touchHandled) {
			touchHandled = false
			return
		}
		action()
	}

	function expand() {
		mobileUIStore.expandPlayer()
	}
</script>

{#if $previewInfo}
	<div
		class="fixed inset-x-0 z-40 px-2 transition-[bottom] duration-300 ease-out"
		style="bottom: calc({overDetail ? '0px' : '3.5rem'} + env(safe-area-inset-bottom) + 0.5rem)"
		transition:fly={{ y: 96, duration: 320, easing: easeFluid }}
	>
		<div class="glass relative overflow-hidden rounded-2xl border border-stroke/60 shadow-lg shadow-black/25">
			<!-- Tap anywhere on the card (except the play/pause control) to open the full-screen player. -->
			<button
				type="button"
				class="flex w-full items-center gap-3 py-2.5 pr-16 pl-2.5 text-left"
				aria-label={$translate('player.expand')}
				onpointerdown={(e) => tapDown(e, expand)}
				onclick={() => tapClick(expand)}
			>
				{#if $previewInfo.release.artwork_url}
					<img
						src={$previewInfo.release.artwork_url}
						alt=""
						class="h-11 w-11 flex-shrink-0 rounded-lg object-cover shadow-sm"
					/>
				{:else}
					<div
						class="flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-lg bg-surface-2 text-text-tertiary"
					>
						<svg viewBox="0 0 24 24" class="h-5 w-5" fill="currentColor">
							<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
						</svg>
					</div>
				{/if}
				<div class="flex min-w-0 flex-1 flex-col">
					<span class="truncate text-sm font-medium text-text-primary">
						{track?.name ?? $previewInfo.release.title ?? $translate('common.untitled')}
					</span>
					<span class="truncate text-xs text-text-secondary">
						{$previewInfo.release.artist ?? $translate('common.unknownArtist')}
					</span>
				</div>
			</button>

			<!-- Play/pause: overlaid at the right edge so it stays outside the card's tap-to-expand target. -->
			<button
				type="button"
				class="absolute top-1/2 right-2 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full text-text-primary active:bg-surface-2"
				aria-label={$isPlaying ? $translate('player.pause') : $translate('player.play')}
				onpointerdown={(e) => tapDown(e, () => playerStore.togglePlayPause())}
				onclick={() => tapClick(() => playerStore.togglePlayPause())}
			>
				{#if loading}
					<Spinner class="h-5 w-5" />
				{:else if $isPlaying}
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor"><path d="M6 5h4v14H6zM14 5h4v14h-4z" /></svg>
				{:else}
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z" /></svg>
				{/if}
			</button>

			<!-- Progress line along the bottom edge (ends clipped by the card's rounded corners). -->
			<div class="absolute inset-x-0 bottom-0 h-0.5 bg-text-tertiary/15">
				<div class="h-full bg-brand-primary" style="width: {$playbackProgress}%"></div>
			</div>
		</div>
	</div>
{/if}
