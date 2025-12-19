// Re-export all stores
export { libraryStore, sortedTracks, displayedTracks, trackCount, isLoading } from './library'
export {
	playerStore,
	isPlaying,
	currentTrack,
	playbackPosition,
	playbackDuration,
	volume,
	playbackProgress,
} from './player'
export { tagsStore, allTags, getTagById, getCategoryById } from './tags'
export { playlistsStore, rootPlaylists, getPlaylistChildren, buildPlaylistTree } from './playlists'
export type { PlaylistTreeNode } from './playlists'
export { uiStore, selectedTrackIds, selectedTrackCount, hasSelection, searchQuery, isSearchActive } from './ui'
export { toastStore, toasts, hasToasts } from './toast'
export type { Toast, ToastType } from './toast'
export { settingsStore, theme, accentColor, resolvedTheme, settingsLoading } from './settings'
