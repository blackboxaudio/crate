<script lang="ts">
	import {
		playerStore,
		currentTrack,
		isPlaying,
		shuffleEnabled,
		repeatMode,
		playbackPosition,
		playbackDuration,
		volume,
		playbackSpeed,
		previewInfo,
		discoveryStore,
	} from '$lib/stores'
	import { canAdvance, userQueueCount } from '$shared/stores/playbackQueue'
	import { uiLayoutStore, queuePanelVisible } from '$lib/stores/uiLayout'
	import { onMount } from 'svelte'
	import PlaybackControls from './PlaybackControls.svelte'
	import QueuePanel from './QueuePanel.svelte'
	import SeekBar from './SeekBar.svelte'
	import TempoControl from './TempoControl.svelte'
	import VolumeControl from './VolumeControl.svelte'
	import TrackInfo from './TrackInfo.svelte'

	type Props = {
		onNext?: () => void
		onPrevious?: () => void
		onLocateTrack?: () => void
	}

	let { onNext, onPrevious, onLocateTrack }: Props = $props()

	const hasTrack = $derived($currentTrack !== null || $previewInfo !== null)

	function handlePlayPause() {
		playerStore.togglePlayPause()
	}

	function handleStop() {
		playerStore.stop()
	}

	function handleSeek(position: number) {
		playerStore.seek(position)
	}

	function handleVolumeChange(vol: number) {
		playerStore.setVolume(vol)
	}

	function handleSpeedChange(speed: number) {
		playerStore.setSpeed(speed)
	}

	function handleSpeedCommit() {
		playerStore.commitPreviewSpeed()
	}

	function handleLikeToggle() {
		if ($previewInfo) {
			const track = $previewInfo.release.tracks[$previewInfo.trackIndex]
			discoveryStore.toggleTrackLiked($previewInfo.releaseId, track.id)
		}
	}

	// The queue panel hangs from the bar's top edge but right-aligns to its button (the rightmost
	// transport control), so it reads as the button's popover. Measured on open and on resize (the
	// centre block is fluid, so the button's x shifts with the window).
	let barEl: HTMLDivElement | undefined = $state()
	let queueButtonEl: HTMLDivElement | undefined = $state()
	let panelRight = $state(16)

	function updatePanelAnchor() {
		if (!barEl || !queueButtonEl) return
		panelRight = barEl.getBoundingClientRect().right - queueButtonEl.getBoundingClientRect().right
	}

	$effect(() => {
		if ($queuePanelVisible) updatePanelAnchor()
	})

	onMount(() => {
		if (!barEl) return
		const observer = new ResizeObserver(updatePanelAnchor)
		observer.observe(barEl)
		return () => observer.disconnect()
	})
</script>

<div class="relative flex items-center gap-4 border-t border-stroke bg-surface-1 px-4 py-3" bind:this={barEl}>
	<!-- Track Info -->
	<div class="w-64 flex-shrink-0">
		<TrackInfo
			track={$currentTrack}
			previewInfo={$previewInfo}
			onLocate={onLocateTrack}
			onLikeToggle={handleLikeToggle}
		/>
	</div>

	<!-- Center Controls -->
	<div class="mx-auto flex max-w-2xl flex-1 flex-col items-center gap-2">
		<PlaybackControls
			isPlaying={$isPlaying}
			{hasTrack}
			canAdvance={$canAdvance}
			shuffleEnabled={$shuffleEnabled}
			repeatMode={$repeatMode}
			onPlayPause={handlePlayPause}
			onStop={handleStop}
			onToggleShuffle={() => playerStore.toggleShuffle()}
			onCycleRepeat={() => playerStore.cycleRepeatMode()}
			queueOpen={$queuePanelVisible}
			queueCount={$userQueueCount}
			onToggleQueue={() => uiLayoutStore.toggleQueuePanel()}
			bind:queueButtonEl
			{onPrevious}
			{onNext}
		/>

		<SeekBar position={$playbackPosition} duration={$playbackDuration} disabled={!hasTrack} onSeek={handleSeek} />
	</div>

	<!-- Queue: sits with the secondary controls (tempo, volume) rather than in the transport row, so
	     shuffle / repeat keep flanking prev-play-next. Vertically centred in the bar; `mr-auto` splits
	     the bar's slack evenly on either side of it — with only the centre block's auto margins, all
	     of that slack landed between the seek bar and this button. -->
	<!-- Tempo -->
	<TempoControl speed={$playbackSpeed} onSpeedChange={handleSpeedChange} onSpeedCommit={handleSpeedCommit} />

	<!-- Volume -->
	<div class="flex w-40 flex-shrink-0 justify-end">
		<VolumeControl volume={$volume} onVolumeChange={handleVolumeChange} />
	</div>

	{#if $queuePanelVisible}
		<QueuePanel right={panelRight} trigger={queueButtonEl} onClose={() => uiLayoutStore.setQueuePanelVisible(false)} />
	{/if}
</div>
