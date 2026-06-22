<script lang="ts">
	import { fly } from 'svelte/transition'
	import { easeFluid } from '$lib/easing'
	import { translate } from '$shared/i18n'
	import { playerStore, previewInfo, isPlaying, previewLoadingReleaseId, playbackProgress } from '$shared/stores/player'
	import { mobileUIStore, detailCovering } from '$lib/stores/mobileUI'
	import Spinner from '$lib/components/common/Spinner.svelte'

	// Persistent mini-player for discovery preview playback: a floating liquid-glass card (Spotify-style)
	// that sits just above the bottom tab bar, visually separated from it by a gap, side margins, rounded
	// corners, and a lift shadow. Tap the card to open the full-screen ExpandedPlayer; the play/pause button
	// toggles playback without expanding. There is intentionally no drag handle / swipe-up — tapping is the
	// only way up. A thin progress line runs along the bottom edge. Renders only while a preview is active;
	// lives at the app root so it persists across the feed ↔ release-detail navigation.
	const track = $derived($previewInfo ? $previewInfo.release.tracks[$previewInfo.trackIndex] : null)
	const loading = $derived($previewInfo != null && $previewLoadingReleaseId === $previewInfo.releaseId)

	// Float above the bottom tab bar (its 3.5rem + safe-area height) on the main shell, with a small gap.
	// When a full-screen release detail covers the tab bar, drop to float just above the bottom safe-area.
	// Tracks `detailCovering` (not `detailReleaseId`), which drops the instant a close starts — so this
	// rises back over the tab bar *as* the detail slides out, rather than after it finishes.
	const overDetail = $derived($detailCovering)

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
				onclick={expand}
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
				onclick={() => playerStore.togglePlayPause()}
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
