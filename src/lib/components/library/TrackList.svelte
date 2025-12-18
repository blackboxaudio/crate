<script lang="ts">
  import type { Track, SortConfig } from '$lib/types';
  import { handleSelection } from '$lib/utils/selection';
  import TrackListHeader from './TrackListHeader.svelte';
  import TrackRow from './TrackRow.svelte';

  type Props = {
    tracks: Track[];
    selectedIds: Set<string>;
    playingTrackId?: string | null;
    sortConfig: SortConfig;
    onSelectionChange?: (ids: Set<string>) => void;
    onTrackPlay?: (track: Track) => void;
    onSortChange?: (config: SortConfig) => void;
    onContextMenu?: (e: MouseEvent, track: Track) => void;
  }

  let {
    tracks,
    selectedIds,
    playingTrackId = null,
    sortConfig,
    onSelectionChange,
    onTrackPlay,
    onSortChange,
    onContextMenu
  }: Props = $props();

  let lastClickedId: string | null = $state(null);

  function handleTrackClick(track: Track, e: MouseEvent) {
    const result = handleSelection(tracks, selectedIds, track.id, lastClickedId, {
      shiftKey: e.shiftKey,
      metaKey: e.metaKey,
      ctrlKey: e.ctrlKey
    });

    lastClickedId = result.lastClickedId;
    onSelectionChange?.(result.selectedIds);
  }

  function handleTrackDoubleClick(track: Track) {
    onTrackPlay?.(track);
  }

  function handleTrackContextMenu(track: Track, e: MouseEvent) {
    e.preventDefault();

    // If track not selected, select it
    if (!selectedIds.has(track.id)) {
      onSelectionChange?.(new Set([track.id]));
    }

    onContextMenu?.(e, track);
  }
</script>

<div class="flex flex-col h-full bg-zinc-900">
  <TrackListHeader {sortConfig} onSort={onSortChange} />

  <div class="flex-1 overflow-auto">
    {#if tracks.length === 0}
      <div class="flex flex-col items-center justify-center h-full text-zinc-500 p-8">
        <svg class="w-16 h-16 mb-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="1.5"
            d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"
          />
        </svg>
        <p class="text-lg font-medium mb-2">No tracks yet</p>
        <p class="text-sm">Drag and drop audio files here to import them</p>
      </div>
    {:else}
      {#each tracks as track (track.id)}
        <TrackRow
          {track}
          selected={selectedIds.has(track.id)}
          playing={playingTrackId === track.id}
          onclick={(e) => handleTrackClick(track, e)}
          ondblclick={() => handleTrackDoubleClick(track)}
          oncontextmenu={(e) => handleTrackContextMenu(track, e)}
        />
      {/each}
    {/if}
  </div>
</div>
