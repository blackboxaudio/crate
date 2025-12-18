<script lang="ts">
  import { playerStore, currentTrack, isPlaying, playbackPosition, playbackDuration, volume } from '$lib/stores';
  import PlaybackControls from './PlaybackControls.svelte';
  import SeekBar from './SeekBar.svelte';
  import VolumeControl from './VolumeControl.svelte';
  import TrackInfo from './TrackInfo.svelte';

  function handlePlayPause() {
    playerStore.togglePlayPause();
  }

  function handleStop() {
    playerStore.stop();
  }

  function handleSeek(position: number) {
    playerStore.seek(position);
  }

  function handleVolumeChange(vol: number) {
    playerStore.setVolume(vol);
  }
</script>

<div class="flex items-center gap-4 px-4 py-3 bg-zinc-900 border-t border-zinc-800">
  <!-- Track Info -->
  <div class="w-64 flex-shrink-0">
    <TrackInfo track={$currentTrack} />
  </div>

  <!-- Center Controls -->
  <div class="flex-1 flex flex-col items-center gap-2 max-w-2xl mx-auto">
    <PlaybackControls
      isPlaying={$isPlaying}
      hasTrack={$currentTrack !== null}
      onPlayPause={handlePlayPause}
      onStop={handleStop}
    />

    <SeekBar
      position={$playbackPosition}
      duration={$playbackDuration}
      disabled={!$currentTrack}
      onSeek={handleSeek}
    />
  </div>

  <!-- Volume -->
  <div class="w-40 flex-shrink-0 flex justify-end">
    <VolumeControl
      volume={$volume}
      onVolumeChange={handleVolumeChange}
    />
  </div>
</div>
