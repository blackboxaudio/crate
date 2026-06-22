<script lang="ts">
	import { slide, fade } from 'svelte/transition'
	import { easeFluid } from '$lib/easing'
	import { translate } from '$shared/i18n'
	import {
		playerStore,
		previewInfo,
		isPlaying,
		previewLoadingReleaseId,
		playbackPosition,
		playbackDuration,
		playbackSpeed,
	} from '$shared/stores/player'
	import { discoveryStore } from '$shared/stores/discovery'
	import { formatDuration } from '$shared/utils/format'
	import { isIOS } from '$shared/utils/platform'
	import { mobileUIStore, isPlayerExpanded } from '$lib/stores/mobileUI'
	import Drawer from '$lib/components/common/Drawer.svelte'
	import Slider from '$shared/components/Slider.svelte'

	// Full-screen preview player: large artwork over a blurred album-art wash, an interactive scrubber,
	// prev / play-pause / next transport, a like toggle, and a tempo (±10% speed) control. Slide / scrim /
	// drag-to-dismiss come from the shared `Drawer` baseline (direction="bottom"); this stays mounted while
	// a preview exists and opens when `$isPlayerExpanded`. Reads/writes the shared playerStore so it stays
	// in sync with the mini-player and the OS media session.
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

	// Tempo: a ±10% bipolar speed fader (the shared Slider, same as the desktop TempoControl), revealed by
	// the metronome toggle in the transport row. The Slider's snapToCenter detents the fader at exactly
	// 1.0x. On iOS the native engine applies the rate immediately via setSpeed→setRate; on the HTML5 path
	// we commit on release. Reset snaps to 1.0x.
	let showTempo = $state(false)
	const tempoPct = $derived(Math.round(($playbackSpeed - 1) * 1000) / 10)

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
				<!-- Drag handle: hidden until dragging, near the top; collapse is by dragging the sheet down. -->
				<span
					class="pointer-events-none absolute top-2 left-1/2 h-1 w-10 -translate-x-1/2 rounded-full bg-text-primary/40 transition-opacity duration-200"
					style="opacity: {dragging ? 1 : 0}"
				></span>

				<!-- Artwork -->
				<div class="flex flex-1 items-center justify-center px-4 pt-3">
					{#if $previewInfo.release.artwork_url}
						<img
							src={$previewInfo.release.artwork_url}
							alt=""
							class="aspect-square w-full max-w-sm rounded-2xl object-cover shadow-2xl"
						/>
					{:else}
						<div
							class="flex aspect-square w-full max-w-sm items-center justify-center rounded-2xl bg-surface-2 text-text-tertiary"
						>
							<svg viewBox="0 0 24 24" class="h-16 w-16" fill="currentColor">
								<path d="M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6zm-2 16a2 2 0 1 1 0-4 2 2 0 0 1 0 4z" />
							</svg>
						</div>
					{/if}
				</div>

				<!-- Track info + transport. No parent gap/space-y: each row carries its own `mt-4`, so the
				     tempo control's top margin belongs to the sliding element itself and Svelte's `slide`
				     animates it away with the height. A selector-based gap/space-y collapses un-animated when
				     the tempo row unmounts, which jumped the control box. -->
				<div class="flex flex-col px-4 pb-3">
					<div class="flex items-center gap-3">
						<div class="flex min-w-0 flex-1 flex-col">
							<span class="truncate text-xl font-semibold text-text-primary">
								{track?.name ?? $previewInfo.release.title ?? $translate('common.untitled')}
							</span>
							<span class="truncate text-base text-text-secondary">
								{$previewInfo.release.artist ?? $translate('common.unknownArtist')}
							</span>
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

					<!-- Transport: prev / play / next stay centered so the play button sits at the screen center.
					     The metronome (tempo) toggle is a right-hand accessory (absolute, so it doesn't shift
					     play); a left-hand shuffle will balance it later. Only play stays a circle. -->
					<div class="relative mt-4 flex items-center justify-center gap-6">
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
							class="flex h-12 w-12 items-center justify-center rounded-md text-text-primary active:bg-surface-2 disabled:opacity-30"
							aria-label={$translate('player.next')}
							disabled={!canNext}
							onclick={() => playerStore.nextTrack()}
						>
							<svg class="h-7 w-7" viewBox="0 0 24 24" fill="currentColor">
								<path d="M16 6h2v12h-2zM4 6l11 6L4 18z" />
							</svg>
						</button>
						<button
							type="button"
							class="absolute top-1/2 right-0 flex h-11 w-11 -translate-y-1/2 items-center justify-center rounded-md transition-colors active:bg-surface-2 {showTempo
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
					</div>

					<!-- Tempo (±10% speed fader): revealed by the metronome toggle, slides down into view. The
					     slider is bipolar — its fill grows out from the centre (0%) toward the thumb, with a small
					     detent at zero. The readout on the left balances the reset on the right. -->
					{#if showTempo}
						<div class="mt-4" transition:slide={{ duration: 250, easing: easeFluid }}>
							<div class="flex items-center gap-3" out:fade={{ duration: 120, easing: easeFluid }}>
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
