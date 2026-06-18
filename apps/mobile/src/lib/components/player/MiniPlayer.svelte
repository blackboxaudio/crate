<script lang="ts">
	import { translate } from '$shared/i18n'
	import { playerStore, previewInfo, isPlaying, previewLoadingReleaseId, playbackProgress } from '$shared/stores/player'
	import { mobileUIStore } from '$lib/stores/mobileUI'
	import { swipeVertical } from '$lib/actions/swipeVertical'

	// Persistent mini-player bar for discovery preview playback: artwork + current track, a play/pause
	// toggle, and a thin progress indicator. Tap the track area or swipe up anywhere on the bar to open
	// the full-screen ExpandedPlayer. Renders only while a preview is active; lives at the app root so
	// it persists across the feed ↔ release-detail navigation. Full transport (scrubber, prev/next)
	// lives in the expanded player.
	const track = $derived($previewInfo ? $previewInfo.release.tracks[$previewInfo.trackIndex] : null)
	const loading = $derived($previewInfo != null && $previewLoadingReleaseId === $previewInfo.releaseId)

	function expand() {
		mobileUIStore.expandPlayer()
	}
</script>

{#if $previewInfo}
	<div
		class="pb-safe fixed inset-x-0 bottom-0 z-40 border-t border-stroke bg-surface-1"
		use:swipeVertical={{ onSwipeUp: expand }}
	>
		<!-- Non-interactive progress indicator across the top edge of the bar. -->
		<div class="h-0.5 w-full bg-surface-2">
			<div class="h-full bg-brand-primary" style="width: {$playbackProgress}%"></div>
		</div>
		<div class="flex items-center gap-3 px-4 py-2">
			<button
				type="button"
				class="flex min-w-0 flex-1 items-center gap-3 text-left"
				aria-label={$translate('player.expand')}
				onclick={expand}
			>
				{#if $previewInfo.release.artwork_url}
					<img src={$previewInfo.release.artwork_url} alt="" class="h-10 w-10 flex-shrink-0 rounded object-cover" />
				{:else}
					<div class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded bg-surface-2 text-text-tertiary">
						<svg viewBox="0 0 24 24" class="h-4 w-4" fill="currentColor">
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
			<button
				type="button"
				class="flex h-10 w-10 flex-shrink-0 items-center justify-center rounded-full text-text-primary active:bg-surface-2"
				aria-label={$isPlaying ? $translate('player.pause') : $translate('player.play')}
				onclick={() => playerStore.togglePlayPause()}
			>
				{#if loading}
					<svg class="h-5 w-5 animate-spin" viewBox="0 0 24 24" fill="none">
						<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
						<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.4 0 0 5.4 0 12h4z" />
					</svg>
				{:else if $isPlaying}
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor"><path d="M6 5h4v14H6zM14 5h4v14h-4z" /></svg>
				{:else}
					<svg class="h-5 w-5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z" /></svg>
				{/if}
			</button>
		</div>
	</div>
{/if}
