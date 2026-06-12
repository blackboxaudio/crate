// Shared store re-exports — cross-platform stores usable by both the desktop and mobile apps.
// Desktop-only stores (and the `uiLayout` store) live in apps/desktop/src/lib/stores and are
// re-exported there alongside these via the desktop barrel.
export {
	playerStore,
	isPlaying,
	currentTrack,
	playbackPosition,
	playbackDuration,
	volume,
	playbackProgress,
	shuffleEnabled,
	playbackSource,
	playbackSpeed,
	previewInfo,
	previewTrackIndex,
} from './player'
export { tagsStore, allTags, getTagById, getCategoryById, computeTagStates } from './tags'
export { playlistsStore, rootPlaylists, getPlaylistChildren, buildPlaylistTree } from './playlists'
export type { PlaylistTreeNode } from './playlists'
export {
	uiStore,
	activeView,
	selectedTrackIds,
	selectedTrackCount,
	hasSelection,
	selectedReleaseIds,
	selectedReleaseCount,
	recentlyToggledMixedTags,
	selectedTagIds,
	tagFilterMode,
	scrollOffset,
} from './ui'
export { toastStore, toasts, hasToasts } from './toast'
export type { Toast, ToastType } from './toast'
export {
	settingsStore,
	theme,
	accentColor,
	resolvedTheme,
	settingsLoading,
	keyNotationFormat,
	language,
	dateFormat,
	continuousPlayback,
	hasCompletedOnboarding,
	hasCompletedWizard,
} from './settings'
export {
	discoveryStore,
	sortedReleases,
	displayedReleases,
	releaseCount,
	isDiscoveryLoading,
	refreshingReleaseIds,
	newOnly,
} from './discovery'
export { expandedReleaseIds } from './expandedReleases'
export { discoveryPlaylistStore, discoveryPlaylistReleases } from './discoveryPlaylist'
export {
	cloudSyncStore,
	syncStatus,
	syncPhase,
	isSignedIn,
	isSyncAvailable,
	cloudDevices,
	libraryRoots,
	unmappedRootIds,
	signingIn,
} from './cloudSync'
