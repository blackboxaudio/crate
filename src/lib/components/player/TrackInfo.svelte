<script lang="ts">
  import type { Track } from '$lib/types';
  import { getTrackDisplayName, getTrackDisplayArtist } from '$lib/utils/format';

  type Props = {
    track: Track | null;
  }

  let { track }: Props = $props();
</script>

<div class="flex items-center gap-3 min-w-0">
  <!-- Album art placeholder -->
  <div class="w-12 h-12 bg-zinc-800 rounded flex-shrink-0 flex items-center justify-center">
    {#if track}
      <svg class="w-6 h-6 text-zinc-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="1.5"
          d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
        />
      </svg>
    {/if}
  </div>

  <!-- Track info -->
  <div class="min-w-0 flex-1">
    {#if track}
      <p class="text-sm font-medium text-zinc-100 truncate">
        {getTrackDisplayName(track)}
      </p>
      <p class="text-xs text-zinc-400 truncate">
        {getTrackDisplayArtist(track)}
      </p>
    {:else}
      <p class="text-sm text-zinc-500">No track selected</p>
    {/if}
  </div>
</div>
