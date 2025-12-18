<script lang="ts">
  import { formatDuration } from '$lib/utils/format';

  type Props = {
    position: number;
    duration: number;
    disabled?: boolean;
    onSeek?: (position: number) => void;
  }

  let {
    position,
    duration,
    disabled = false,
    onSeek
  }: Props = $props();

  let isDragging = $state(false);
  let dragPosition = $state(0);

  let displayPosition = $derived(isDragging ? dragPosition : position);
  let progress = $derived(duration > 0 ? (displayPosition / duration) * 100 : 0);

  function handleMouseDown(e: MouseEvent) {
    if (disabled || duration === 0) return;
    isDragging = true;
    updatePositionFromEvent(e);
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isDragging) return;
    updatePositionFromEvent(e);
  }

  function handleMouseUp() {
    if (isDragging) {
      onSeek?.(dragPosition);
      isDragging = false;
    }
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', handleMouseUp);
  }

  function updatePositionFromEvent(e: MouseEvent) {
    const bar = (e.target as HTMLElement).closest('.seek-bar');
    if (!bar) return;
    const rect = bar.getBoundingClientRect();
    const x = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
    const percent = x / rect.width;
    dragPosition = Math.floor(percent * duration);
  }
</script>

<div class="flex items-center gap-2 w-full">
  <span class="text-xs tabular-nums text-zinc-400 w-10 text-right">
    {formatDuration(displayPosition)}
  </span>

  <div
    role="slider"
    tabindex="0"
    aria-label="Seek"
    aria-valuemin={0}
    aria-valuemax={duration}
    aria-valuenow={displayPosition}
    class="seek-bar relative flex-1 h-1.5 bg-zinc-700 rounded-full cursor-pointer group"
    class:opacity-50={disabled}
    class:cursor-not-allowed={disabled}
    onmousedown={handleMouseDown}
  >
    <!-- Progress -->
    <div
      class="absolute inset-y-0 left-0 bg-blue-500 rounded-full"
      style="width: {progress}%"
    ></div>

    <!-- Thumb -->
    <div
      class="absolute top-1/2 -translate-y-1/2 w-3 h-3 bg-white rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
      style="left: calc({progress}% - 6px)"
    ></div>
  </div>

  <span class="text-xs tabular-nums text-zinc-400 w-10">
    {formatDuration(duration)}
  </span>
</div>
