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
} from '$shared/stores/player'
export { tagsStore, allTags, getTagById, getCategoryById, computeTagStates } from '$shared/stores/tags'
export { playlistsStore, rootPlaylists, getPlaylistChildren, buildPlaylistTree } from '$shared/stores/playlists'
export type { PlaylistTreeNode } from '$shared/stores/playlists'
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
} from '$shared/stores/ui'
export {
	uiLayoutStore,
	rightSidebarVisible,
	rightSidebarWidth,
	selectedTreeIds,
	contextMenuPlaylistId,
	contextMenuDiscoveryTrackId,
	playlistScrollOffsets,
} from './uiLayout'
export { toastStore, toasts, hasToasts } from '$shared/stores/toast'
export type { Toast, ToastType } from '$shared/stores/toast'
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
} from '$shared/stores/settings'
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
} from '$shared/stores/discovery'
export {
	followStore,
	followedSources,
	followNewCount,
	followedEntityKeys,
	sortedFollowedSources,
} from '$shared/stores/follow'
export type { FollowSort } from '$shared/stores/follow'
export { updaterStore, updateStatus, updateAvailable } from './updater'
export { expandedReleaseIds } from '$shared/stores/expandedReleases'
export { discoveryPlaylistStore, discoveryPlaylistReleases } from '$shared/stores/discoveryPlaylist'
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
} from '$shared/stores/cloudSync'
