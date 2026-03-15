// =============================================================================
// Re-export shared stores
// =============================================================================

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
	previewLoadingReleaseId,
	isMuted,
	tagsStore,
	allTags,
	getTagById,
	getCategoryById,
	computeTagStates,
	playlistsStore,
	rootPlaylists,
	getPlaylistChildren,
	buildPlaylistTree,
	getPlaylistPath,
	buildBreadcrumbItems,
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
	playlistScrollOffsets,
	toastStore,
	toasts,
	hasToasts,
	settingsStore,
	theme,
	accentColor,
	font,
	resolvedTheme,
	settingsLoading,
	keyNotationFormat,
	language,
	dateFormat,
	exportFormat,
	autoAnalyzeOnImport,
	autoSyncOnConnect,
	autoSyncOnChange,
	continuousPlayback,
	autoFetchMetadata,
	transferTagsOnImport,
	removeReleaseAfterImport,
	ignoredDeviceIds,
	lastBackupAt,
	backupFrequency,
	lastBackupType,
	hasCompletedOnboarding,
	hasCompletedWizard,
	audioDevice,
	audioDevices,
	discoveryStore,
	sortedReleases,
	displayedReleases,
	releaseCount,
	isDiscoveryLoading,
	refreshingReleaseIds,
	expandedReleaseIds,
	discoveryPlaylistStore,
	discoveryPlaylistReleases,
} from '$shared/stores'
export type {
	PlaylistTreeNode,
	Toast,
	ToastType,
	PlayerStoreHooks,
	PlaylistsStoreHooks,
	SettingsStoreHooks,
} from '$shared/stores'

// =============================================================================
// Desktop-only stores
// =============================================================================

export { appStore, appInfo, isDev, appVersion, appEnvironment, appDataDir, appLoading, devToolsOpen } from './app'
export { libraryStore, sortedTracks, displayedTracks, trackCount, isLoading } from './library'
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
export { updaterStore, updateStatus, updateAvailable } from './updater'
export { pageActions } from './pageActions'
export type { PageActions } from './pageActions'
export { locateStore, pendingScrollTrackId, pendingScrollReleaseId } from './locate'
export { syncStore } from './sync'
export { exportStore } from './export'
export { splashVisible, dismissSplash } from './splash'
export { backupStore } from './backup'
