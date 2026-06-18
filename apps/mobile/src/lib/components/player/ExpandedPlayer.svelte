<script lang="ts">
	import { onMount } from 'svelte'
	import { translate } from '$shared/i18n'
	import {
		playerStore,
		previewInfo,
		isPlaying,
		previewLoadingReleaseId,
		playbackPosition,
		playbackDuration,
	} from '$shared/stores/player'
	import { formatDuration } from '$shared/utils/format'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import { swipeVertical } from '$lib/actions/swipeVertical'

	// Full-screen preview player: large artwork over a blurred album-art wash, an interactive scrubber,
	// and prev / play-pause / next transport. Opened from the mini-player (tap or swipe-up); dismissed by
	// dragging the sheet down (finger-follow) or tapping the drag handle. The sheet slides over a scrim
	// that fades as you drag, revealing the discovery feed + mini-player behind it. Reads/writes the
	// shared playerStore so it stays in sync with the mini-player and the OS media session.
	const track = $derived($previewInfo ? $previewInfo.release.tracks[$previewInfo.trackIndex] : null)
	const loading = $derived($previewInfo != null && $previewLoadingReleaseId === $previewInfo.releaseId)
	const canNext = $derived($previewInfo != null && $previewInfo.trackIndex + 1 < $previewInfo.release.tracks.length)

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

	// Sheet position: a single translateY drives the open slide-up, finger-follow drag-down, snap-back,
	// and commit (slide fully down, then unmount). `openness` (1 = expanded, 0 = collapsed) feeds the
	// scrim opacity so the feed behind is revealed as you drag.
	let viewportH = $state(typeof window !== 'undefined' ? window.innerHeight : 800)
	let dragY = $state(0)
	let dragging = $state(false)
	let entered = $state(false)
	let collapsing = $state(false)

	const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v))
	const sheetOffsetPx = $derived(collapsing ? viewportH : dragging ? dragY : entered ? 0 : viewportH)
	const openness = $derived(clamp(1 - sheetOffsetPx / viewportH, 0, 1))
	const transitionOn = $derived(!dragging)

	onMount(() => {
		const onResize = () => (viewportH = window.innerHeight)
		window.addEventListener('resize', onResize)
		// Next frame: slide the sheet up from below (the CSS transition animates the transform change).
		const raf = requestAnimationFrame(() => (entered = true))
		return () => {
			window.removeEventListener('resize', onResize)
			cancelAnimationFrame(raf)
		}
	})

	// Finger-follow drag-to-dismiss: translate the sheet down with the finger; snap back if not committed.
	// swipeVertical emits onProgress(0) on release, which re-enables the transition for the snap/commit.
	function onDragProgress(dy: number) {
		if (dy === 0) {
			dragging = false
			dragY = 0
			return
		}
		dragging = true
		dragY = Math.max(0, dy)
	}

	// Commit: animate the sheet fully down, then unmount once the transform transition finishes.
	function collapse() {
		collapsing = true
	}
	function onSheetTransitionEnd(e: TransitionEvent) {
		if (e.target === e.currentTarget && collapsing && e.propertyName === 'transform') {
			mobileUIStore.collapsePlayer()
		}
	}
</script>

{#if $previewInfo}
	<div class="fixed inset-0 z-50">
		<!-- Scrim: dims and reveals the discovery feed + mini-player behind as the sheet drags down. -->
		<div
			class="pointer-events-none absolute inset-0 bg-black {transitionOn
				? 'transition-opacity duration-300 ease-out motion-reduce:transition-none'
				: ''}"
			style="opacity: {0.5 * openness}"
		></div>

		<!-- Sheet: the moving panel, carrying the album-art background and all controls. -->
		<div
			class="absolute inset-0 overflow-hidden bg-surface-0 {transitionOn
				? 'transition-transform duration-300 ease-out motion-reduce:transition-none'
				: ''}"
			style="transform: translateY({sheetOffsetPx}px)"
			use:swipeVertical={{ onSwipeDown: collapse, onProgress: onDragProgress }}
			ontransitionend={onSheetTransitionEnd}
		>
			<!-- Album-art background: a blurred, slowly drifting wash behind the content, with a
			     theme-aware legibility scrim so the handle and transport stay readable in both modes.
			     Full-bleed and first in the DOM; the relative content wrapper below paints over it. -->
			{#if $previewInfo.release.artwork_url}
				<img
					src={$previewInfo.release.artwork_url}
					alt=""
					class="art-wash pointer-events-none absolute inset-0 h-full w-full object-cover blur-2xl"
				/>
				<div
					class="pointer-events-none absolute inset-0 bg-gradient-to-b from-surface-0/80 via-surface-0/25 to-surface-0/90"
				></div>
			{/if}

			<div class="pt-safe pb-safe relative flex h-full flex-col">
				<!-- Drag handle: tap to collapse, drag (anywhere on the sheet) to dismiss. -->
				<div class="flex items-center justify-center px-4 py-3">
					<button
						type="button"
						class="flex h-6 w-16 items-center justify-center"
						aria-label={$translate('player.collapse')}
						onclick={collapse}
					>
						<span class="h-1 w-10 rounded-full bg-stroke"></span>
					</button>
				</div>

				<!-- Artwork -->
				<div class="flex flex-1 items-center justify-center px-8">
					{#if $previewInfo.release.artwork_url}
						<img
							src={$previewInfo.release.artwork_url}
							alt=""
							class="aspect-square w-full max-w-sm rounded-xl object-cover shadow-lg"
						/>
					{:else}
						<div
							class="flex aspect-square w-full max-w-sm items-center justify-center rounded-xl bg-surface-2 text-text-tertiary"
						>
							<svg viewBox="0 0 24 24" class="h-16 w-16" fill="currentColor">
								<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
							</svg>
						</div>
					{/if}
				</div>

				<!-- Track info + transport -->
				<div class="flex flex-col gap-4 px-8 pb-6">
					<div class="flex min-w-0 flex-col">
						<span class="truncate text-xl font-semibold text-text-primary">
							{track?.name ?? $previewInfo.release.title ?? $translate('common.untitled')}
						</span>
						<span class="truncate text-base text-text-secondary">
							{$previewInfo.release.artist ?? $translate('common.unknownArtist')}
						</span>
					</div>

					<!-- Scrubber (stop pointerdown from reaching the sheet's drag gesture). -->
					<div class="flex flex-col gap-1">
						<input
							type="range"
							min="0"
							max={$playbackDuration || 0}
							value={sliderValue}
							aria-label={$translate('player.seek')}
							class="h-1 w-full cursor-pointer"
							style="accent-color: var(--brand-primary)"
							oninput={onScrubInput}
							onchange={onScrubCommit}
							onpointerdown={(e) => e.stopPropagation()}
						/>
						<div class="flex justify-between text-xs text-text-tertiary tabular-nums">
							<span>{formatDuration(sliderValue)}</span>
							<span>{formatDuration($playbackDuration)}</span>
						</div>
					</div>

					<!-- Transport controls -->
					<div class="flex items-center justify-center gap-8">
						<button
							type="button"
							class="flex h-12 w-12 items-center justify-center rounded-full text-text-primary active:bg-surface-2"
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
								<svg class="h-8 w-8 animate-spin" viewBox="0 0 24 24" fill="none">
									<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
									<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z" />
								</svg>
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
							class="flex h-12 w-12 items-center justify-center rounded-full text-text-primary active:bg-surface-2 disabled:opacity-30"
							aria-label={$translate('player.next')}
							disabled={!canNext}
							onclick={() => playerStore.nextTrack()}
						>
							<svg class="h-7 w-7" viewBox="0 0 24 24" fill="currentColor">
								<path d="M16 6h2v12h-2zM4 6l11 6L4 18z" />
							</svg>
						</button>
					</div>
				</div>
			</div>
		</div>
	</div>
{/if}
