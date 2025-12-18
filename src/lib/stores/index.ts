// Re-export all stores
export { libraryStore, sortedTracks, trackCount, isLoading } from './library';
export {
  playerStore,
  isPlaying,
  currentTrack,
  playbackPosition,
  playbackDuration,
  volume,
  playbackProgress
} from './player';
export { tagsStore, allTags, getTagById, getCategoryById } from './tags';
export {
  playlistsStore,
  rootPlaylists,
  getPlaylistChildren,
  buildPlaylistTree
} from './playlists';
export type { PlaylistTreeNode } from './playlists';
export {
  uiStore,
  selectedTrackIds,
  selectedTrackCount,
  hasSelection,
  searchQuery,
  isSearchActive
} from './ui';
