<script lang="ts">
	import {
		playerStore,
		currentTrack,
		isPlaying,
		playbackPosition,
		playbackDuration,
		volume,
		playbackSpeed,
		previewInfo,
		discoveryStore,
	} from '$lib/stores'
	import PlaybackControls from './PlaybackControls.svelte'
	import SeekBar from './SeekBar.svelte'
	import TempoControl from './TempoControl.svelte'
	import VolumeControl from './VolumeControl.svelte'
	import TrackInfo from './TrackInfo.svelte'

	type Props = {
		onNext?: () => void
		onPrevious?: () => void
	}

	let { onNext, onPrevious }: Props = $props()

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
</script>

<div class="flex items-center gap-4 border-t border-stroke bg-surface-1 px-4 py-3">
	<!-- Track Info -->
	<div class="w-64 flex-shrink-0">
		<TrackInfo track={$currentTrack} previewInfo={$previewInfo} onLikeToggle={handleLikeToggle} />
	</div>

	<!-- Center Controls -->
	<div class="mx-auto flex max-w-2xl flex-1 flex-col items-center gap-2">
		<PlaybackControls
			isPlaying={$isPlaying}
			{hasTrack}
			onPlayPause={handlePlayPause}
			onStop={handleStop}
			{onPrevious}
			{onNext}
		/>

		<SeekBar position={$playbackPosition} duration={$playbackDuration} disabled={!hasTrack} onSeek={handleSeek} />
	</div>

	<!-- Tempo -->
	<TempoControl speed={$playbackSpeed} onSpeedChange={handleSpeedChange} onSpeedCommit={handleSpeedCommit} />

	<!-- Volume -->
	<div class="flex w-40 flex-shrink-0 justify-end">
		<VolumeControl volume={$volume} onVolumeChange={handleVolumeChange} />
	</div>
</div>
