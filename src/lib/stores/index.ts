// Re-export all stores
export { appStore, appInfo, isDev, appVersion, appEnvironment, appDataDir, appLoading, devToolsOpen } from './app'
export { libraryStore, sortedTracks, displayedTracks, trackCount, isLoading } from './library'
export {
	playerStore,
	isPlaying,
	currentTrack,
	playbackPosition,
	playbackDuration,
	volume,
	playbackProgress,
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
	rightSidebarVisible,
	rightSidebarWidth,
	selectedTreeIds,
	contextMenuPlaylistId,
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
	dateFormat,
	continuousPlayback,
} from './settings'
export {
	devicesStore,
	devices,
	deviceCount,
	hasDevices,
	devicesLoading,
	visibleDevices,
	visibleDeviceCount,
	hasVisibleDevices,
} from './devices'
export { diagnosticsStore, diagnosticEntries, systemInfo, diagnosticsLoading, errorCount } from './diagnostics'
export { missingTracksStore, missingTrackIds, checkingTrackIds } from './missingTracks'
export {
	dragStore,
	isDragging,
	dragData,
	dragPosition,
	hoveredDropTarget,
	isDraggingTracks,
	isDraggingReleases,
	isDraggingPlaylist,
	isDraggingTag,
	needsDropTargetRefresh,
} from './drag'
export type { DragData } from './drag'
export { crashStore, hasCrashed, crashError } from './crash'
export type { CrashInfo } from './crash'
export { analysisStore, analyzingTrackIds, isAnalyzing } from './analysis'
export {
	discoveryStore,
	sortedReleases,
	displayedReleases,
	releaseCount,
	isDiscoveryLoading,
	refreshingReleaseIds,
} from './discovery'
export { updaterStore, updateStatus, updateAvailable } from './updater'
export { expandedReleaseIds } from './expandedReleases'
export { discoveryPlaylistStore, discoveryPlaylistReleases } from './discoveryPlaylist'
export { pageActions } from './pageActions'
export type { PageActions } from './pageActions'
