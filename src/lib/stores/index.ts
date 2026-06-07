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
	rightSidebarVisible,
	rightSidebarWidth,
	selectedTreeIds,
	contextMenuPlaylistId,
	contextMenuDiscoveryTrackId,
	scrollOffset,
	playlistScrollOffsets,
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
	newOnly,
} from './discovery'
export { followStore, followedSources, followNewCount, followedEntityKeys, sortedFollowedSources } from './follow'
export type { FollowSort } from './follow'
export { updaterStore, updateStatus, updateAvailable } from './updater'
export { expandedReleaseIds } from './expandedReleases'
export { discoveryPlaylistStore, discoveryPlaylistReleases } from './discoveryPlaylist'
export { pageActions } from './pageActions'
export type { PageActions } from './pageActions'
export { locateStore, pendingScrollTrackId, pendingScrollReleaseId } from './locate'
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
