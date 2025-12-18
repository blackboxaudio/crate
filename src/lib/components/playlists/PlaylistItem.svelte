<script lang="ts">
  import type { Playlist } from '$lib/types';

  type Props = {
    playlist: Playlist;
    selected?: boolean;
    depth?: number;
    expanded?: boolean;
    hasChildren?: boolean;
    onclick?: () => void;
    onToggle?: () => void;
    oncontextmenu?: (e: MouseEvent) => void;
  }

  let {
    playlist,
    selected = false,
    depth = 0,
    expanded = false,
    hasChildren = false,
    onclick,
    onToggle,
    oncontextmenu
  }: Props = $props();

  let paddingLeft = $derived(`${depth * 12 + 8}px`);
</script>

<div
  role="treeitem"
  tabindex="0"
  aria-selected={selected}
  aria-expanded={playlist.is_folder ? expanded : undefined}
  class="flex items-center gap-2 py-1.5 pr-2 rounded cursor-pointer transition-colors {selected ? 'bg-blue-600/20 text-zinc-100' : 'text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200'}"
  style="padding-left: {paddingLeft}"
  onclick={onclick}
  oncontextmenu={oncontextmenu}
  onkeydown={(e) => e.key === 'Enter' && onclick?.()}
>
  <!-- Expand/Collapse toggle for folders -->
  {#if playlist.is_folder && hasChildren}
    <button
      type="button"
      aria-label={expanded ? 'Collapse' : 'Expand'}
      class="w-4 h-4 flex items-center justify-center text-zinc-500 hover:text-zinc-300"
      onclick={(e) => { e.stopPropagation(); onToggle?.(); }}
    >
      <svg
        class="w-3 h-3 transition-transform {expanded ? 'rotate-90' : ''}"
        fill="currentColor"
        viewBox="0 0 24 24"
      >
        <path d="M8 5v14l11-7z" />
      </svg>
    </button>
  {:else}
    <span class="w-4"></span>
  {/if}

  <!-- Icon -->
  <span class="flex-shrink-0">
    {#if playlist.is_folder}
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
      </svg>
    {:else if playlist.is_smart}
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
      </svg>
    {:else}
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3" />
      </svg>
    {/if}
  </span>

  <!-- Name -->
  <span class="flex-1 truncate text-sm">
    {playlist.name}
  </span>

  <!-- Track count -->
  {#if !playlist.is_folder}
    <span class="text-xs text-zinc-500">
      {playlist.track_count}
    </span>
  {/if}
</div>
