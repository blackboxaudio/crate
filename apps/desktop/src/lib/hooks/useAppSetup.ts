import { tick } from 'svelte'
import { get } from 'svelte/store'
import type {
	ActiveView,
	DiscoveryRelease,
	DiscoverySourceType,
	DuplicateTrack,
	Playlist,
	SettingsPage,
	Track,
	UsbDevice,
} from '$shared/types'
import {
	appStore,
	libraryStore,
	sortedTracks,
	displayedTracks,
	playerStore,
	currentTrack,
	tagsStore,
	playlistsStore,
	uiStore,
	activeView,
	selectedTrackIds,
	selectedReleaseIds,
	settingsStore,
	continuousPlayback,
	devicesStore,
	missingTracksStore,
	missingTrackIds,
	displayedReleases,
	expandedReleaseIds,
	discoveryStore,
	updaterStore,
	previewInfo,
} from '$lib/stores'
import { tagFilterMode } from '$shared/stores/ui'
import { recentlyToggledMixedTags } from '$shared/stores/ui'
import { syncStore } from '$lib/stores/sync'
import { toastStore } from '$shared/stores/toast'
import { exportStore } from '$lib/stores/export'
import { dismissSplash } from '$lib/stores/splash'
import { discoveryPlaylistStore } from '$shared/stores/discoveryPlaylist'
import {
	createTagController,
	createTrackController,
	createDeviceController,
	createExportController,
	createPlaylistController,
} from '$lib/controllers'
import { useAppInitialization } from './useAppInitialization'
import { useKeyboardShortcuts } from './useKeyboardShortcuts'
import { useMenuActions } from './useMenuActions'
import { useMediaKeys } from './useMediaKeys'
import { useDragDropCoordination } from './useDragDropCoordination'
import { translate } from '$shared/i18n'
import * as playlistsApi from '$shared/api/playlists'

// =============================================================================
// Types
// =============================================================================

export interface AppSetupConfig {
	getPlaylists: () => Playlist[]
	getDevices: () => UsbDevice[]
	getSelectedPlaylistId: () => string | null
	getSelectedFolderId: () => string | null
	getSelectedTagIds: () => string[]
	getModalOrchestrator: () => ModalOrchestratorRef | undefined
	handleViewChange: (view: ActiveView) => void
	setShowAddReleaseModal: () => void
	setIsDragOver: (dragOver: boolean) => void
}

interface ModalOrchestratorRef {
	isModalOpen: () => boolean
	openSettingsModal: (tab?: SettingsPage) => void
	openCreatePlaylistModal: (parentId: string | null) => void
	openCreateFolderModal: (parentId: string | null) => void
	openCreateSmartPlaylistModal: (parentId: string | null, context?: ActiveView) => void
	openEditSmartPlaylistModal: (playlist: Playlist) => void
	openRenamePlaylistModal: (playlist: Playlist) => void
	openDeletePlaylistModal: (playlist: Playlist, hasChildren: boolean) => void
	openDeletePlaylistBulkModal: (playlists: Playlist[]) => void
	openMoveConflictModal: (playlist: Playlist, conflict: Playlist, targetId: string | null) => void
	openRelocateModal: (track: Track) => void
	openRemoveFromPlaylistModal: (trackIds: string[], playlistId: string) => void
	openRemoveFromLibraryModal: (trackIds: string[]) => void
	openRemoveDiscoveryReleasesModal: (releaseIds: string[]) => void
	openRemoveDiscoveryReleasesFromPlaylistModal: (releaseIds: string[], playlistId: string) => void
	openDuplicateTrackModal: (
		duplicates: DuplicateTrack[],
		onComplete: (updatedTracks: Track[], newTracks: Track[], replacedTrackIds: string[]) => void
	) => void
	openDeviceInfoModal: (device: UsbDevice) => void
	openReformatDeviceModal: (device: UsbDevice) => void
	openExportToDeviceModal: (device: UsbDevice) => void
	openExportPlaylistModal: (playlist: Playlist) => void
	openQuickExportModal: () => void
	openExportFailureModal: (error: string, deviceId: string, mountPoint: string, filesCopied: number) => void
}

export interface AppSetupResult {
	tagController: ReturnType<typeof createTagController>
	trackController: ReturnType<typeof createTrackController>
	deviceController: ReturnType<typeof createDeviceController>
	exportController: ReturnType<typeof createExportController>
	playlistController: ReturnType<typeof createPlaylistController>
	playNextTrack: () => void
	playPreviousTrack: () => void
	onMountSetup: () => Promise<() => void>
	setupDragDrop: () => () => void
}

// =============================================================================
// Hook
// =============================================================================

export function createAppSetup(config: AppSetupConfig): AppSetupResult {
	const {
		getPlaylists,
		getDevices,
		getSelectedPlaylistId,
		getSelectedFolderId,
		getSelectedTagIds,
		getModalOrchestrator,
		handleViewChange,
		setShowAddReleaseModal,
		setIsDragOver,
	} = config

	// =========================================================================
	// Controllers
	// =========================================================================

	const tagController = createTagController({
		tagsStore,
		libraryStore,
		discoveryStore,
		uiStore,
		getSelectedTagIds,
		getSelectedPlaylistId,
		getTagFilterMode: () => get(tagFilterMode),
		getSelectedTrackIds: () => get(selectedTrackIds),
		getSelectedReleaseIds: () => get(selectedReleaseIds),
		getRecentlyToggledMixedTags: () => get(recentlyToggledMixedTags),
		getActiveView: () => get(activeView),
	})

	const trackController = createTrackController(
		{
			playerStore,
			libraryStore,
			playlistsStore,
			missingTracksStore,
			uiStore,
			toastStore,
			getSelectedPlaylistId,
			getPlaylists,
			getMissingTrackIds: () => get(missingTrackIds),
		},
		{
			openRelocateModal: (track) => getModalOrchestrator()?.openRelocateModal(track),
			openRemoveFromPlaylistModal: (trackIds, playlistId) =>
				getModalOrchestrator()?.openRemoveFromPlaylistModal(trackIds, playlistId),
			openRemoveFromLibraryModal: (trackIds) => getModalOrchestrator()?.openRemoveFromLibraryModal(trackIds),
			openDuplicateTrackModal: (duplicates, onComplete) =>
				getModalOrchestrator()?.openDuplicateTrackModal(duplicates, onComplete),
		}
	)

	const deviceController = createDeviceController(
		{ devicesStore, settingsStore, toastStore },
		{
			openDeviceInfoModal: (device) => getModalOrchestrator()?.openDeviceInfoModal(device),
			openReformatDeviceModal: (device) => getModalOrchestrator()?.openReformatDeviceModal(device),
		}
	)

	const exportController = createExportController(
		{
			exportStore,
			toastStore,
			getDevices,
			getPlaylists,
		},
		{
			openExportToDeviceModal: (device) => getModalOrchestrator()?.openExportToDeviceModal(device),
			openExportPlaylistModal: (playlist) => getModalOrchestrator()?.openExportPlaylistModal(playlist),
			openQuickExportModal: () => getModalOrchestrator()?.openQuickExportModal(),
			openExportFailureModal: (error, deviceId, mountPoint, filesCopied) =>
				getModalOrchestrator()?.openExportFailureModal(error, deviceId, mountPoint, filesCopied),
		}
	)

	const playlistController = createPlaylistController(
		{
			playlistsStore,
			discoveryStore,
			libraryStore,
			uiStore,
			toastStore,
			getPlaylists,
			getSelectedPlaylistId,
			getSelectedFolderId,
			getSelectedTagIds,
			getTagFilterMode: () => get(tagFilterMode),
			getActiveView: () => get(activeView),
			onDiscoveryPlaylistSelected: async (playlistId) => {
				const playlist = getPlaylists().find((p) => p.id === playlistId)
				const releases = playlist?.is_smart
					? await playlistsApi.getSmartPlaylistReleases(playlistId)
					: await playlistsStore.getPlaylistReleases(playlistId)
				discoveryPlaylistStore.cacheAndSet(playlistId, releases)
			},
		},
		{
			openCreatePlaylistModal: (parentId) => getModalOrchestrator()?.openCreatePlaylistModal(parentId),
			openCreateFolderModal: (parentId) => getModalOrchestrator()?.openCreateFolderModal(parentId),
			openCreateSmartPlaylistModal: (parentId, context) =>
				getModalOrchestrator()?.openCreateSmartPlaylistModal(parentId, context),
			openEditSmartPlaylistModal: (playlist) => getModalOrchestrator()?.openEditSmartPlaylistModal(playlist),
			openRenamePlaylistModal: (playlist) => getModalOrchestrator()?.openRenamePlaylistModal(playlist),
			openDeletePlaylistModal: (playlist, hasChildren) =>
				getModalOrchestrator()?.openDeletePlaylistModal(playlist, hasChildren),
			openMoveConflictModal: (playlist, conflict, targetId) =>
				getModalOrchestrator()?.openMoveConflictModal(playlist, conflict, targetId),
		}
	)

	// =========================================================================
	// Track Navigation
	// =========================================================================

	const PREVIEWABLE_SOURCES: Set<DiscoverySourceType> = new Set(['bandcamp', 'soundcloud', 'youtube'])

	function trackCanPlay(release: DiscoveryRelease, trackIndex: number): boolean {
		const track = release.tracks[trackIndex]
		if (!track?.duration_ms) return false
		if (release.source_type === 'discogs') return track.video_id !== null
		return PREVIEWABLE_SOURCES.has(release.source_type) || release.tracks.some((t) => t.video_id !== null)
	}

	function findPreviewableTrackIndex(release: DiscoveryRelease, direction: 'first' | 'last'): number {
		if (direction === 'first') {
			return release.tracks.findIndex((_, i) => trackCanPlay(release, i))
		}
		for (let i = release.tracks.length - 1; i >= 0; i--) {
			if (trackCanPlay(release, i)) return i
		}
		return -1
	}

	function playNextTrack() {
		const preview = get(previewInfo)
		if (preview) {
			// Try next previewable track in current release
			let nextIndex = preview.trackIndex + 1
			while (nextIndex < preview.release.tracks.length && !trackCanPlay(preview.release, nextIndex)) {
				nextIndex++
			}
			if (nextIndex < preview.release.tracks.length) {
				playerStore.playPreview(preview.release, nextIndex)
				return
			}

			// Move to next release in the filtered view, wrapping around
			const releases = get(displayedReleases)
			const releaseIdx = releases.findIndex((r) => r.id === preview.releaseId)
			if (releaseIdx === -1 || releases.length === 0) return

			for (let i = 1; i <= releases.length; i++) {
				const nextRelease = releases[(releaseIdx + i) % releases.length]
				const trackIdx = findPreviewableTrackIndex(nextRelease, 'first')
				if (trackIdx !== -1) {
					playerStore.playPreview(nextRelease, trackIdx)
					return
				}
			}
			return
		}
		const id = get(currentTrack)?.id
		if (!id) return
		const tracks = get(displayedTracks)
		const idx = tracks.findIndex((t) => t.id === id)
		if (idx >= 0 && idx < tracks.length - 1) trackController.play(tracks[idx + 1])
	}

	function playPreviousTrack() {
		const preview = get(previewInfo)
		if (preview) {
			// Try previous previewable track in current release
			let prevIndex = preview.trackIndex - 1
			while (prevIndex >= 0 && !trackCanPlay(preview.release, prevIndex)) {
				prevIndex--
			}
			if (prevIndex >= 0) {
				playerStore.playPreview(preview.release, prevIndex)
				return
			}

			// Move to previous release in the filtered view, wrapping around
			const releases = get(displayedReleases)
			const releaseIdx = releases.findIndex((r) => r.id === preview.releaseId)
			if (releaseIdx === -1 || releases.length === 0) return

			for (let i = 1; i <= releases.length; i++) {
				const prevRelease = releases[(releaseIdx - i + releases.length) % releases.length]
				const trackIdx = findPreviewableTrackIndex(prevRelease, 'last')
				if (trackIdx !== -1) {
					playerStore.playPreview(prevRelease, trackIdx)
					return
				}
			}
			return
		}
		const id = get(currentTrack)?.id
		if (!id) return
		const tracks = get(displayedTracks)
		const idx = tracks.findIndex((t) => t.id === id)
		if (idx > 0) trackController.play(tracks[idx - 1])
	}

	// =========================================================================
	// Shared Handlers (used by both keyboard shortcuts and menu actions)
	// =========================================================================

	const handlers = {
		playPause: () => {
			const state = get(currentTrack)
			const preview = get(previewInfo)

			// If a track or preview is loaded, toggle normally
			if (state || preview) {
				playerStore.togglePlayPause()
				return
			}

			// Nothing loaded — play first item in current view
			if (get(activeView) === 'discovery') {
				const releases = get(displayedReleases)
				for (const release of releases) {
					const trackIdx = findPreviewableTrackIndex(release, 'first')
					if (trackIdx !== -1) {
						playerStore.playPreview(release, trackIdx)
						return
					}
				}
			} else {
				const tracks = get(displayedTracks)
				if (tracks.length > 0) {
					trackController.play(tracks[0])
				}
			}
		},
		stop: () => playerStore.stop(),
		seekForward: () => playerStore.seekRelative(10000),
		seekBackward: () => playerStore.seekRelative(-10000),
		fineSeekForward: () => playerStore.seekRelative(1000),
		fineSeekBackward: () => playerStore.seekRelative(-1000),
		volumeUp: () => playerStore.adjustVolume(0.1),
		volumeDown: () => playerStore.adjustVolume(-0.1),
		toggleMute: () => playerStore.toggleMute(),

		selectAll: () => {
			if (get(activeView) === 'discovery') {
				uiStore.setSelectedReleases(new Set(get(displayedReleases).map((r) => r.id)))
			} else {
				uiStore.setSelectedTracks(new Set(get(sortedTracks).map((t) => t.id)))
			}
		},

		openSettings: (tab?: SettingsPage) => getModalOrchestrator()?.openSettingsModal(tab),

		quickExport: () => {
			if (getDevices().length > 0) getModalOrchestrator()?.openQuickExportModal()
		},

		jumpToPlayingTrack: () => {
			const track = get(currentTrack)
			if (!track) return
			if (getSelectedPlaylistId()) playlistController.handleLibraryClick()
			uiStore.selectTrack(track.id)
		},

		toggleView: () => {
			if (getModalOrchestrator()?.isModalOpen()) return
			const next = get(activeView) === 'library' ? 'discovery' : 'library'
			handleViewChange(next)
		},

		import: async () => {
			if (get(activeView) !== 'library') {
				handleViewChange('library')
				await tick()
			}
			trackController.handleImport()
		},

		addRelease: async () => {
			if (get(activeView) !== 'discovery') {
				handleViewChange('discovery')
				await tick()
			}
			setShowAddReleaseModal()
		},

		refreshMetadata: async () => {
			if (get(activeView) !== 'discovery') return
			const ids = [...get(selectedReleaseIds)]
			if (ids.length === 0) return
			await Promise.all(ids.map((id) => discoveryStore.refreshMetadata(id)))
		},
	}

	// =========================================================================
	// Mount Setup
	// =========================================================================

	async function onMountSetup(): Promise<() => void> {
		const splashStartTime = Date.now()
		const minDisplayTime = 1000

		await exportStore.startListening()

		const cleanupApp = await useAppInitialization({
			stores: {
				appStore,
				libraryStore,
				tagsStore,
				playlistsStore,
				settingsStore,
				devicesStore,
				syncStore,
				discoveryStore,
			},
			toastStore,
			onExternalFileDrop: trackController.handleExternalFileDrop,
			onDragStateChange: (dragOver) => setIsDragOver(dragOver),
		})

		// Restore last-playing track/preview from localStorage now that stores are loaded
		playerStore.restoreTrack(get(libraryStore).tracks)
		await playerStore.restorePreview()

		// Restore persisted navigation state (playlist/folder selection)
		const restoredState = get(uiStore)
		if (restoredState.selectedPlaylistId) {
			const playlist = getPlaylists().find((p) => p.id === restoredState.selectedPlaylistId)
			if (playlist) {
				await playlistController.handlePlaylistSelect(playlist)
			} else {
				// Persisted playlist was deleted — clear and fall back to library
				uiStore.selectPlaylist(null)
			}
		} else if (restoredState.selectedFolderId) {
			const folder = getPlaylists().find((p) => p.id === restoredState.selectedFolderId)
			if (!folder) {
				// Persisted folder was deleted — clear and fall back to library
				uiStore.selectFolder(null)
			}
		}

		const cleanupKeyboard = useKeyboardShortcuts({
			isModalOpen: () => getModalOrchestrator()?.isModalOpen() ?? false,
			onPlayPause: handlers.playPause,
			onFocusSearch: () => {
				const searchInput = document.querySelector('input[type="search"]') as HTMLInputElement
				searchInput?.focus()
			},
			onClearSelection: () => uiStore.clearSelection(),
			onSelectAll: handlers.selectAll,
			onOpenSettings: handlers.openSettings,
			onNewPlaylist: () => playlistController.handleCreatePlaylist(),
			onNewFolder: () => playlistController.handleCreateFolder(),
			onImport: handlers.import,
			onDeleteSelected: () => {
				const treeIds = get(uiStore).selectedTreeIds
				if (treeIds.size > 1) {
					const selected = getPlaylists().filter((p) => treeIds.has(p.id))
					if (selected.length > 0) {
						getModalOrchestrator()?.openDeletePlaylistBulkModal(selected)
					}
					return true
				}

				const playlistId = getSelectedPlaylistId()
				const playlists = getPlaylists()
				const currentPlaylist = playlistId ? playlists.find((p) => p.id === playlistId) : null

				if (get(activeView) === 'discovery') {
					const releaseIds = get(selectedReleaseIds)
					if (releaseIds.size > 0) {
						if (playlistId && !currentPlaylist?.is_smart) {
							getModalOrchestrator()?.openRemoveDiscoveryReleasesFromPlaylistModal(Array.from(releaseIds), playlistId)
						} else {
							getModalOrchestrator()?.openRemoveDiscoveryReleasesModal(Array.from(releaseIds))
						}
						return true
					}
				}

				const ids = [...get(selectedTrackIds)]
				if (ids.length > 0) {
					if (playlistId && !currentPlaylist?.is_smart) {
						getModalOrchestrator()?.openRemoveFromPlaylistModal(ids, playlistId)
					} else {
						getModalOrchestrator()?.openRemoveFromLibraryModal(ids)
					}
				} else if (playlistId) {
					if (currentPlaylist) playlistController.handlePlaylistDelete(currentPlaylist)
				} else {
					const folderId = getSelectedFolderId()
					if (folderId) {
						const folder = playlists.find((p) => p.id === folderId)
						if (folder) playlistController.handlePlaylistDelete(folder)
					}
				}
				return true
			},
			onPlaySelected: () => {
				if (get(activeView) === 'discovery') {
					const releaseIds = get(selectedReleaseIds)
					if (releaseIds.size > 0) {
						const releases = get(displayedReleases)
						expandedReleaseIds.toggleSelection(
							[...releaseIds],
							(id) => (releases.find((r) => r.id === id)?.tracks.length ?? 0) > 0
						)
					}
					return
				}
				const selectedIds = get(selectedTrackIds)
				if (selectedIds.size > 0) {
					const firstSelectedId = [...selectedIds][0]
					const track = get(displayedTracks).find((t) => t.id === firstSelectedId)
					if (track) trackController.play(track)
				}
			},
			onSeekBackward: handlers.seekBackward,
			onSeekForward: handlers.seekForward,
			onFineSeekBackward: handlers.fineSeekBackward,
			onFineSeekForward: handlers.fineSeekForward,
			onPreviousTrack: playPreviousTrack,
			onNextTrack: playNextTrack,
			onVolumeUp: handlers.volumeUp,
			onVolumeDown: handlers.volumeDown,
			onToggleMute: handlers.toggleMute,
			onSelectPreviousTrack: () => {
				if (get(activeView) === 'discovery') {
					const releases = get(displayedReleases)
					if (releases.length === 0) return
					const ids = get(selectedReleaseIds)
					if (ids.size === 0) {
						uiStore.selectRelease(releases[releases.length - 1].id)
					} else {
						const firstId = [...ids][0]
						const idx = releases.findIndex((r) => r.id === firstId)
						if (idx > 0) uiStore.selectRelease(releases[idx - 1].id)
					}
					return
				}
				const tracks = get(displayedTracks)
				if (tracks.length === 0) return
				const ids = get(selectedTrackIds)
				if (ids.size === 0) {
					uiStore.selectTrack(tracks[tracks.length - 1].id)
				} else {
					const firstId = [...ids][0]
					const idx = tracks.findIndex((t) => t.id === firstId)
					if (idx > 0) uiStore.selectTrack(tracks[idx - 1].id)
				}
			},
			onSelectNextTrack: () => {
				if (get(activeView) === 'discovery') {
					const releases = get(displayedReleases)
					if (releases.length === 0) return
					const ids = get(selectedReleaseIds)
					if (ids.size === 0) {
						uiStore.selectRelease(releases[0].id)
					} else {
						const lastId = [...ids].pop()
						const idx = releases.findIndex((r) => r.id === lastId)
						if (idx >= 0 && idx < releases.length - 1) uiStore.selectRelease(releases[idx + 1].id)
					}
					return
				}
				const tracks = get(displayedTracks)
				if (tracks.length === 0) return
				const ids = get(selectedTrackIds)
				if (ids.size === 0) {
					uiStore.selectTrack(tracks[0].id)
				} else {
					const lastId = [...ids].pop()
					const idx = tracks.findIndex((t) => t.id === lastId)
					if (idx >= 0 && idx < tracks.length - 1) uiStore.selectTrack(tracks[idx + 1].id)
				}
			},
			onQuickExport: handlers.quickExport,
			onJumpToPlayingTrack: handlers.jumpToPlayingTrack,
			onToggleView: handlers.toggleView,
			onAddRelease: handlers.addRelease,
			onRefreshMetadata: handlers.refreshMetadata,
		})

		const cleanupMenu = await useMenuActions({
			onImport: handlers.import,
			onAddRelease: handlers.addRelease,
			onCreatePlaylist: playlistController.handleCreatePlaylist,
			onCreateFolder: playlistController.handleCreateFolder,
			onSelectAll: handlers.selectAll,
			onPlayPause: handlers.playPause,
			onStop: handlers.stop,
			onNextTrack: playNextTrack,
			onPreviousTrack: playPreviousTrack,
			onSeekForward: handlers.seekForward,
			onSeekBackward: handlers.seekBackward,
			onFineSeekForward: handlers.fineSeekForward,
			onFineSeekBackward: handlers.fineSeekBackward,
			onVolumeUp: handlers.volumeUp,
			onVolumeDown: handlers.volumeDown,
			onToggleMute: handlers.toggleMute,
			onOpenSettings: handlers.openSettings,
			onQuickExport: handlers.quickExport,
			onJumpToPlayingTrack: handlers.jumpToPlayingTrack,
			onToggleView: handlers.toggleView,
			onToggleEditor: () => uiStore.toggleRightSidebar(),
			onExpandAllReleases: () => {
				const releases = get(displayedReleases)
				expandedReleaseIds.expandAll(releases.filter((r) => r.tracks.length > 0).map((r) => r.id))
			},
			onCollapseAllReleases: () => expandedReleaseIds.collapseAll(),
			onRefreshMetadata: handlers.refreshMetadata,
		})

		const cleanupMediaKeys = await useMediaKeys({
			onPlayPause: handlers.playPause,
			onNextTrack: playNextTrack,
			onPreviousTrack: playPreviousTrack,
		})

		// Register Media Session API handlers for next/previous so that media keys
		// work during preview playback. WKWebView's HTML5 Audio element creates its
		// own media session that takes priority over souvlaki — without these
		// handlers, next/previous keys are silently consumed by the webview.
		if ('mediaSession' in navigator) {
			navigator.mediaSession.setActionHandler('nexttrack', playNextTrack)
			navigator.mediaSession.setActionHandler('previoustrack', playPreviousTrack)
		}

		playerStore.onTrackEnd(() => {
			if (get(continuousPlayback)) {
				playNextTrack()
			}
		})

		const elapsed = Date.now() - splashStartTime
		if (elapsed < minDisplayTime) {
			await new Promise((r) => setTimeout(r, minDisplayTime - elapsed))
		}

		if (get(activeView) === 'discovery') {
			await discoveryStore.loadReleases()
		}

		updaterStore.check(true)
		const updateInterval = setInterval(() => updaterStore.check(true), 60 * 60 * 1000)

		dismissSplash()

		return () => {
			cleanupApp()
			cleanupKeyboard()
			cleanupMenu()
			cleanupMediaKeys()
			if ('mediaSession' in navigator) {
				navigator.mediaSession.setActionHandler('nexttrack', null)
				navigator.mediaSession.setActionHandler('previoustrack', null)
			}
			playerStore.onTrackEnd(null)
			exportStore.stopListening()
			clearInterval(updateInterval)
		}
	}

	// =========================================================================
	// Drag-Drop Setup
	// =========================================================================

	function setupDragDrop(): () => void {
		return useDragDropCoordination({
			getPlaylists,
			getDevices,
			onTracksDropOnPlaylist: trackController.handleTracksDropOnPlaylist,
			onReleasesDropOnPlaylist: async (playlistId: string, releaseIds: string[]) => {
				await playlistsStore.addReleases(playlistId, releaseIds)
			},
			onPlaylistMove: playlistController.handlePlaylistDragMove,
			onBulkPlaylistMove: playlistController.handleBulkPlaylistMove,
			onPlaylistExportToDevice: exportController.handlePlaylistDropOnDevice,
			onTagDropOnTrack: async (tagId: string, trackId: string) => {
				const trackIds = get(selectedTrackIds).has(trackId) ? Array.from(get(selectedTrackIds)) : [trackId]
				await tagsStore.assignTags(trackIds, [tagId])
				const playlistId = getSelectedPlaylistId()
				if (playlistId) {
					await libraryStore.loadPlaylistTracks(playlistId)
				} else {
					await libraryStore.loadTracks()
				}
			},
			onTagDropOnRelease: async (tagId: string, releaseId: string) => {
				const releaseIds = get(selectedReleaseIds).has(releaseId) ? Array.from(get(selectedReleaseIds)) : [releaseId]
				await discoveryStore.assignTags(releaseIds, [tagId])
			},
			onTagDropOnCategory: async (tagId: string, _sourceCategoryId: string, targetCategoryId: string) => {
				try {
					await tagsStore.moveTag(tagId, targetCategoryId)
					libraryStore.updateTagCategory(tagId, targetCategoryId)
					discoveryStore.updateTagCategory(tagId, targetCategoryId)
					discoveryPlaylistStore.updateTagCategory(tagId, targetCategoryId)
				} catch (error) {
					const message = error instanceof Error ? error.message : get(translate)('errors.tagNameConflict')
					toastStore.error(message)
				}
			},
		})
	}

	return {
		tagController,
		trackController,
		deviceController,
		exportController,
		playlistController,
		playNextTrack,
		playPreviousTrack,
		onMountSetup,
		setupDragDrop,
	}
}
