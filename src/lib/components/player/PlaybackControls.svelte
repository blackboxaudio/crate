<script lang="ts">
  import { IconButton } from '$lib/components/common';

  type Props = {
    isPlaying: boolean;
    hasTrack: boolean;
    onPlayPause?: () => void;
    onPrevious?: () => void;
    onNext?: () => void;
    onStop?: () => void;
  }

  let {
    isPlaying,
    hasTrack,
    onPlayPause,
    onPrevious,
    onNext,
    onStop
  }: Props = $props();
</script>

<div class="flex items-center gap-1">
  <!-- Previous -->
  <IconButton
    title="Previous"
    disabled={!hasTrack}
    onclick={onPrevious}
  >
    <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
      <path d="M6 6h2v12H6V6zm3.5 6l8.5 6V6l-8.5 6z" />
    </svg>
  </IconButton>

  <!-- Play/Pause -->
  <IconButton
    title={isPlaying ? 'Pause' : 'Play'}
    disabled={!hasTrack}
    size="lg"
    onclick={onPlayPause}
  >
    {#if isPlaying}
      <svg class="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
        <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
      </svg>
    {:else}
      <svg class="w-6 h-6" fill="currentColor" viewBox="0 0 24 24">
        <path d="M8 5v14l11-7z" />
      </svg>
    {/if}
  </IconButton>

  <!-- Stop -->
  <IconButton
    title="Stop"
    disabled={!hasTrack}
    onclick={onStop}
  >
    <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
      <path d="M6 6h12v12H6z" />
    </svg>
  </IconButton>

  <!-- Next -->
  <IconButton
    title="Next"
    disabled={!hasTrack}
    onclick={onNext}
  >
    <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
      <path d="M6 18l8.5-6L6 6v12zm2 0V6l6.5 6L8 18zM16 6v12h2V6h-2z" />
    </svg>
  </IconButton>
</div>
