<script lang="ts">
	import { onMount, onDestroy, tick } from 'svelte'
	import { fade } from 'svelte/transition'
	import { get } from 'svelte/store'

	import type {
		ActiveView,
		Track,
		SortConfig,
		DiscoverySortConfig,
		DiscoveryReleaseCreate,
		DiscoveryFilter,
		DiscoveryRelease,
		ImportResultWithDuplicates,
		Playlist,
		TagCategory,
		Tag,
		TagSelectionState,
		TrackFilter,
		UsbDevice,
		BreadcrumbItem,
		SettingsPage,
	} from '$lib/types'
	import { pickTagCategoryColor } from '$lib/types'
	import {
		appStore,
		libraryStore,
		sortedTracks,
		displayedTracks,
		trackCount,
		playerStore,
		currentTrack,
		tagsStore,
		playlistsStore,
		uiStore,
		activeView,
		selectedTrackIds,
		selectedReleaseIds,
		recentlyToggledMixedTags,
		tagFilterMode,
		settingsStore,
		continuousPlayback,
		devicesStore,
		visibleDevices,
		computeTagStates,
		missingTracksStore,
		missingTrackIds,
		rightSidebarVisible,
		rightSidebarWidth,
		isDragging,
		dragData,
		dragPosition,
		devToolsOpen,
		isDev,
		analysisStore,
		discoveryStore,
		sortedReleases,
		releaseCount,
		previewInfo,
		updaterStore,
		updateAvailable,
		expandedReleaseIds,
	} from '$lib/stores'
	import { isPlaying } from '$lib/stores/player'
	import { syncStore } from '$lib/stores/sync'
	import { toastStore } from '$lib/stores/toast'
	import { buildBreadcrumbItems, getPlaylistChildren } from '$lib/stores/playlists'
	import {
		createTagController,
		createTrackController,
		createDeviceController,
		createExportController,
		createPlaylistController,
	} from '$lib/controllers'
	import {
		useAppInitialization,
		useKeyboardShortcuts,
		useMenuActions,
		useMediaKeys,
		useDragDropCoordination,
	} from '$lib/hooks'
	import { translate } from '$lib/i18n'

	import { Sidebar, Toolbar, RightSidebar } from '$lib/components/layout'
	import { LibraryView } from '$lib/components/library'
	import {
		DiscoveryView,
		AddReleaseModal,
		DiscoveryEditor,
		MergeReleasesModal,
		PurchaseReleaseModal,
	} from '$lib/components/discovery'
	import { TrackEditor } from '$lib/components/editor'
	import { openUrl } from '@tauri-apps/plugin-opener'
	import { Player } from '$lib/components/player'
	import {
		ResizeHandle,
		ContextMenuOrchestrator,
		ModalOrchestrator,
		DragPreview,
		Icon,
		Text,
		UpdateModal,
	} from '$lib/components/common'
	import { splashVisible, dismissSplash } from '$lib/stores/splash'
	import { PlaylistView, FolderView } from '$lib/components/playlists'
	import { openDevTools, closeDevTools, setMenuItemEnabled } from '$lib/api/app'
	import { updateNowPlaying, updatePlaybackState, clearNowPlaying } from '$lib/api/mediaControls'
	import { exportStore } from '$lib/stores/export'
	import * as discoveryApi from '$lib/api/discovery'
	import * as playlistsApi from '$lib/api/playlists'
	import { SvelteMap } from 'svelte/reactivity'

	// =============================================================================
	// State
	// =============================================================================

	// Local state
	let sortConfig = $state<SortConfig>({ field: 'date_added', direction: 'desc' })
	let discoverySortConfig = $state<DiscoverySortConfig>({ field: 'date_added', direction: 'desc' })
	let showAddReleaseModal = $state(false)
	let purchaseRelease = $state<DiscoveryRelease | null>(null)
	let mergeReleases = $state<DiscoveryRelease[] | null>(null)
	let playlists = $state<Playlist[]>([])
	let tagCategories = $state<TagCategory[]>([])
	let devices = $state<UsbDevice[]>([])
	let selectedPlaylistId = $state<string | null>(null)
	let selectedFolderId = $state<string | null>(null)
	let selectedTagIds = $state<string[]>([])
	let sidebarWidth = $state(240)

	// Tag toggle state
	let tagStates = $state<Map<string, TagSelectionState>>(new Map())
	let tagCounts = $state<Map<string, number>>(new Map())

	// Drag and drop state (for external file drops)
	let isDragOver = $state(false)

	// Context menu state for playlist tree hover styling
	let contextMenuPlaylistId = $state<string | null>(null)

	// Playlist tree multi-selection state
	let selectedTreeIds = $state<Set<string>>(new Set())

	// Clear tree multi-selection when navigation changes (breadcrumbs, folder cards, etc.)
	let prevNavPlaylistId: string | null = null
	let prevNavFolderId: string | null = null
	$effect(() => {
		const pId = selectedPlaylistId
		const fId = selectedFolderId
		if (pId !== prevNavPlaylistId || fId !== prevNavFolderId) {
			prevNavPlaylistId = pId
			prevNavFolderId = fId
			if (selectedTreeIds.size > 0) {
				selectedTreeIds = new Set()
			}
		}
	})

	// Cleanup function from onMount
	let cleanupOnMount: (() => void) | undefined

	// =============================================================================
	// Orchestrator Bindings
	// =============================================================================

	let contextMenuOrchestrator: ReturnType<typeof ContextMenuOrchestrator>
	let modalOrchestrator: ReturnType<typeof ModalOrchestrator>

	// =============================================================================
	// Controllers
	// =============================================================================

	// Tag controller
	const tagController = createTagController({
		tagsStore,
		libraryStore,
		discoveryStore,
		uiStore,
		getSelectedTagIds: () => selectedTagIds,
		getSelectedPlaylistId: () => selectedPlaylistId,
		getTagFilterMode: () => $tagFilterMode,
		getSelectedTrackIds: () => $selectedTrackIds,
		getSelectedReleaseIds: () => $selectedReleaseIds,
		getRecentlyToggledMixedTags: () => $recentlyToggledMixedTags,
		getActiveView: () => $activeView,
	})

	// Track controller
	const trackController = createTrackController(
		{
			playerStore,
			libraryStore,
			playlistsStore,
			missingTracksStore,
			uiStore,
			toastStore,
			getSelectedPlaylistId: () => selectedPlaylistId,
			getPlaylists: () => playlists,
			getMissingTrackIds: () => $missingTrackIds,
		},
		{
			openRelocateModal: (track) => modalOrchestrator.openRelocateModal(track),
			openRemoveFromPlaylistModal: (trackIds, playlistId) =>
				modalOrchestrator.openRemoveFromPlaylistModal(trackIds, playlistId),
			openRemoveFromLibraryModal: (trackIds) => modalOrchestrator.openRemoveFromLibraryModal(trackIds),
			openDuplicateTrackModal: (duplicates, onComplete) =>
				modalOrchestrator.openDuplicateTrackModal(duplicates, onComplete),
		}
	)

	// Device controller
	const deviceController = createDeviceController(
		{ devicesStore, settingsStore, toastStore },
		{
			openDeviceInfoModal: (device) => modalOrchestrator.openDeviceInfoModal(device),
			openReformatDeviceModal: (device) => modalOrchestrator.openReformatDeviceModal(device),
		}
	)

	// Export controller
	const exportController = createExportController(
		{
			exportStore,
			toastStore,
			getDevices: () => devices,
			getPlaylists: () => playlists,
		},
		{
			openExportToDeviceModal: (device) => modalOrchestrator.openExportToDeviceModal(device),
			openExportPlaylistModal: (playlist) => modalOrchestrator.openExportPlaylistModal(playlist),
			openQuickExportModal: () => modalOrchestrator.openQuickExportModal(),
			openExportFailureModal: (error, deviceId, mountPoint, filesCopied) =>
				modalOrchestrator.openExportFailureModal(error, deviceId, mountPoint, filesCopied),
		}
	)

	// Playlist controller
	const playlistController = createPlaylistController(
		{
			playlistsStore,
			libraryStore,
			uiStore,
			toastStore,
			getPlaylists: () => playlists,
			getSelectedPlaylistId: () => selectedPlaylistId,
			getSelectedFolderId: () => selectedFolderId,
			getSelectedTagIds: () => selectedTagIds,
			getTagFilterMode: () => $tagFilterMode,
			onDiscoveryPlaylistSelected: async (playlistId) => {
				const playlist = playlists.find((p) => p.id === playlistId)
				discoveryPlaylistReleases = playlist?.is_smart
					? await playlistsApi.getSmartPlaylistReleases(playlistId)
					: await playlistsStore.getPlaylistReleases(playlistId)
				discoveryPlaylistReleasesCache.set(playlistId, discoveryPlaylistReleases)
			},
		},
		{
			openCreatePlaylistModal: (parentId) => modalOrchestrator.openCreatePlaylistModal(parentId),
			openCreateFolderModal: (parentId) => modalOrchestrator.openCreateFolderModal(parentId),
			openCreateSmartPlaylistModal: (parentId, context) =>
				modalOrchestrator.openCreateSmartPlaylistModal(parentId, context),
			openEditSmartPlaylistModal: (playlist) => modalOrchestrator.openEditSmartPlaylistModal(playlist),
			openRenamePlaylistModal: (playlist) => modalOrchestrator.openRenamePlaylistModal(playlist),
			openDeletePlaylistModal: (playlist, hasChildren) =>
				modalOrchestrator.openDeletePlaylistModal(playlist, hasChildren),
			openMoveConflictModal: (playlist, conflict, targetId) =>
				modalOrchestrator.openMoveConflictModal(playlist, conflict, targetId),
		}
	)

	// =============================================================================
	// Store Subscriptions
	// =============================================================================

	$effect(() => {
		const unsubPlaylists = playlistsStore.subscribe((state) => {
			playlists = state.playlists
		})
		const unsubTags = tagsStore.subscribe((state) => {
			tagCategories = state.categories
		})
		const unsubUI = uiStore.subscribe((state) => {
			selectedPlaylistId = state.selectedPlaylistId
			selectedFolderId = state.selectedFolderId
			selectedTagIds = state.selectedTagIds
			sidebarWidth = state.sidebarWidth
		})
		const unsubDevices = visibleDevices.subscribe((visibleDevicesList) => {
			devices = visibleDevicesList
		})

		return () => {
			unsubPlaylists()
			unsubTags()
			unsubUI()
			unsubDevices()
		}
	})

	// Compute tag states when selection or tracks/releases change
	$effect(() => {
		if ($activeView === 'discovery') {
			// Compute tag states from selected releases
			const states = new SvelteMap<string, TagSelectionState>()
			const counts = new SvelteMap<string, number>()
			const selectedIds = $selectedReleaseIds
			if (selectedIds.size > 0) {
				const selectedReleases = $sortedReleases.filter((r) => selectedIds.has(r.id))
				const totalSelected = selectedReleases.length
				if (totalSelected > 0) {
					const tagCountMap = new SvelteMap<string, number>()
					for (const release of selectedReleases) {
						for (const tag of release.tags) {
							tagCountMap.set(tag.id, (tagCountMap.get(tag.id) || 0) + 1)
						}
					}
					const allTags = tagCategories.flatMap((c) => c.tags)
					for (const tag of allTags) {
						const count = tagCountMap.get(tag.id) || 0
						counts.set(tag.id, count)
						if (count === 0) states.set(tag.id, 'inactive')
						else if (count === totalSelected) states.set(tag.id, 'active')
						else states.set(tag.id, 'mixed')
					}
				}
			}
			tagStates = states
			tagCounts = counts
		} else {
			const result = computeTagStates(tagCategories, $displayedTracks, $selectedTrackIds)
			tagStates = result.states
			tagCounts = result.counts
		}
	})

	// Clear recently toggled tags when selection changes
	let previousSelectedIds = $state<Set<string>>(new Set())
	$effect(() => {
		const currentIds = $activeView === 'discovery' ? $selectedReleaseIds : $selectedTrackIds
		if (currentIds.size !== previousSelectedIds.size || ![...currentIds].every((id) => previousSelectedIds.has(id))) {
			uiStore.clearAllRecentlyToggledTags()
			previousSelectedIds = new Set(currentIds)
		}
	})

	// Clear discovery playlist releases when no playlist is selected
	$effect(() => {
		if (!selectedPlaylistId) {
			discoveryPlaylistReleases = []
		}
	})

	// Enable/disable Refresh Metadata menu item based on view and selection
	$effect(() => {
		setMenuItemEnabled('refresh_metadata', $activeView === 'discovery' && $selectedReleaseIds.size > 0)
	})

	// =============================================================================
	// Now Playing Sync
	// =============================================================================

	// Sync current track metadata to OS Now Playing (macOS Control Center, etc.)
	$effect(() => {
		const preview = $previewInfo
		const track = $currentTrack
		if (preview) {
			const previewTrack = preview.release.tracks[preview.trackIndex]
			updateNowPlaying(
				previewTrack?.name || null,
				preview.release.artist || null,
				preview.release.title || null,
				preview.release.artwork_url || preview.release.artwork_path || null,
				previewTrack?.duration_ms ?? null
			).catch(() => {})
		} else if (track) {
			updateNowPlaying(
				track.title || null,
				track.artist || null,
				track.album || null,
				track.artwork_path || null,
				track.duration_ms
			).catch(() => {})
		} else {
			clearNowPlaying().catch(() => {})
		}
	})

	// Sync playback state (playing/paused) to OS — separate from metadata to avoid
	// re-sending metadata on every play/pause toggle
	$effect(() => {
		const playing = $isPlaying
		updatePlaybackState(playing).catch(() => {})
	})

	// =============================================================================
	// Drag-Drop Coordination
	// =============================================================================

	let cleanupDragDrop: (() => void) | undefined

	$effect(() => {
		cleanupDragDrop = useDragDropCoordination({
			getPlaylists: () => playlists,
			getDevices: () => devices,
			onTracksDropOnPlaylist: trackController.handleTracksDropOnPlaylist,
			onReleasesDropOnPlaylist: async (playlistId: string, releaseIds: string[]) => {
				await playlistsStore.addReleases(playlistId, releaseIds)
			},
			onPlaylistMove: playlistController.handlePlaylistDragMove,
			onPlaylistExportToDevice: exportController.handlePlaylistDropOnDevice,
			onTagDropOnTrack: async (tagId: string, trackId: string) => {
				const trackIds = $selectedTrackIds.has(trackId) ? Array.from($selectedTrackIds) : [trackId]
				await tagsStore.assignTags(trackIds, [tagId])
				if (selectedPlaylistId) {
					await libraryStore.loadPlaylistTracks(selectedPlaylistId)
				} else {
					await libraryStore.loadTracks()
				}
			},
			onTagDropOnRelease: async (tagId: string, releaseId: string) => {
				const releaseIds = $selectedReleaseIds.has(releaseId) ? Array.from($selectedReleaseIds) : [releaseId]
				await discoveryStore.assignTags(releaseIds, [tagId])
			},
			onTagDropOnCategory: async (tagId: string, _sourceCategoryId: string, targetCategoryId: string) => {
				try {
					await tagsStore.moveTag(tagId, targetCategoryId)
					libraryStore.updateTagCategory(tagId, targetCategoryId)
					discoveryStore.updateTagCategory(tagId, targetCategoryId)
					discoveryPlaylistReleases = discoveryPlaylistReleases.map((r) => ({
						...r,
						tags: r.tags.map((tag) => (tag.id === tagId ? { ...tag, category_id: targetCategoryId } : tag)),
					}))
				} catch (error) {
					const message = error instanceof Error ? error.message : get(translate)('errors.tagNameConflict')
					toastStore.error(message)
				}
			},
		})

		return () => {
			cleanupDragDrop?.()
		}
	})

	// =============================================================================
	// Track Navigation
	// =============================================================================

	function playNextTrack() {
		const preview = $previewInfo
		if (preview) {
			const nextIndex = preview.trackIndex + 1
			if (nextIndex < preview.release.tracks.length) {
				playerStore.playPreview(preview.release, nextIndex)
			} else {
				playerStore.stop()
			}
			return
		}
		const id = $currentTrack?.id
		if (!id) return
		const tracks = $displayedTracks
		const idx = tracks.findIndex((t) => t.id === id)
		if (idx >= 0 && idx < tracks.length - 1) trackController.play(tracks[idx + 1])
	}

	function playPreviousTrack() {
		const preview = $previewInfo
		if (preview) {
			const prevIndex = preview.trackIndex - 1
			if (prevIndex >= 0) {
				playerStore.playPreview(preview.release, prevIndex)
			}
			return
		}
		const id = $currentTrack?.id
		if (!id) return
		const tracks = $displayedTracks
		const idx = tracks.findIndex((t) => t.id === id)
		if (idx > 0) trackController.play(tracks[idx - 1])
	}

	// =============================================================================
	// Initialization
	// =============================================================================

	async function onMountHelper(): Promise<() => void> {
		const splashStartTime = Date.now()
		const minDisplayTime = 700

		// Initialize export store event listening
		await exportStore.startListening()

		// Initialize app (stores, drag-drop, device listener)
		const cleanupApp = await useAppInitialization({
			stores: { appStore, libraryStore, tagsStore, playlistsStore, settingsStore, devicesStore, syncStore },
			toastStore,
			onExternalFileDrop: trackController.handleExternalFileDrop,
			onDragStateChange: (dragOver) => {
				isDragOver = dragOver
			},
		})

		// Set up keyboard shortcuts
		const cleanupKeyboard = useKeyboardShortcuts({
			isModalOpen: () => modalOrchestrator?.isModalOpen() ?? false,
			onPlayPause: () => playerStore.togglePlayPause(),
			onFocusSearch: () => {
				const searchInput = document.querySelector('input[type="search"]') as HTMLInputElement
				searchInput?.focus()
			},
			onClearSelection: () => uiStore.clearSelection(),
			onSelectAll: () => {
				if ($activeView === 'discovery') {
					uiStore.setSelectedReleases(new Set($sortedReleases.map((r) => r.id)))
				} else {
					const allIds = new Set($sortedTracks.map((t) => t.id))
					uiStore.setSelectedTracks(allIds)
				}
			},
			onOpenSettings: (tab?: SettingsPage) => modalOrchestrator?.openSettingsModal(tab),
			onNewPlaylist: () => playlistController.handleCreatePlaylist(),
			onNewFolder: () => playlistController.handleCreateFolder(),
			onImport: async () => {
				if ($activeView !== 'library') {
					handleViewChange('library')
					await tick()
				}
				trackController.handleImport()
			},
			onDeleteSelected: () => {
				// Bulk delete selected playlist tree items
				if (selectedTreeIds.size > 1) {
					const selected = playlists.filter((p) => selectedTreeIds.has(p.id))
					if (selected.length > 0) {
						modalOrchestrator.openDeletePlaylistBulkModal(selected)
					}
					return true
				}

				// Smart playlists are read-only — don't allow removing items
				const currentPlaylist = selectedPlaylistId ? playlists.find((p) => p.id === selectedPlaylistId) : null
				if (currentPlaylist?.is_smart) return false

				if ($activeView === 'discovery') {
					const releaseIds = $selectedReleaseIds
					if (releaseIds.size > 0) {
						if (selectedPlaylistId) {
							modalOrchestrator.openRemoveDiscoveryReleasesFromPlaylistModal(Array.from(releaseIds), selectedPlaylistId)
						} else {
							modalOrchestrator.openRemoveDiscoveryReleasesModal(Array.from(releaseIds))
						}
						return true
					}
				}
				const ids = [...$selectedTrackIds]
				if (ids.length > 0) {
					if (selectedPlaylistId) {
						modalOrchestrator.openRemoveFromPlaylistModal(ids, selectedPlaylistId)
					} else {
						modalOrchestrator.openRemoveFromLibraryModal(ids)
					}
				} else if (selectedPlaylistId) {
					if (currentPlaylist) playlistController.handlePlaylistDelete(currentPlaylist)
				} else if (selectedFolderId) {
					const folder = playlists.find((p) => p.id === selectedFolderId)
					if (folder) playlistController.handlePlaylistDelete(folder)
				}
				return true
			},
			onPlaySelected: () => {
				if ($activeView === 'discovery') {
					const releaseIds = $selectedReleaseIds
					if (releaseIds.size > 0) {
						const releases = $sortedReleases
						expandedReleaseIds.toggleSelection(
							[...releaseIds],
							(id) => (releases.find((r) => r.id === id)?.tracks.length ?? 0) > 0
						)
					}
					return
				}
				const selectedIds = $selectedTrackIds
				if (selectedIds.size > 0) {
					const firstSelectedId = [...selectedIds][0]
					const track = $displayedTracks.find((t) => t.id === firstSelectedId)
					if (track) trackController.play(track)
				}
			},
			onSeekBackward: () => playerStore.seekRelative(-10000),
			onSeekForward: () => playerStore.seekRelative(10000),
			onFineSeekBackward: () => playerStore.seekRelative(-1000),
			onFineSeekForward: () => playerStore.seekRelative(1000),
			onPreviousTrack: playPreviousTrack,
			onNextTrack: playNextTrack,
			onVolumeUp: () => playerStore.adjustVolume(0.1),
			onVolumeDown: () => playerStore.adjustVolume(-0.1),
			onToggleMute: () => playerStore.toggleMute(),
			onSelectPreviousTrack: () => {
				if ($activeView === 'discovery') {
					const releases = $sortedReleases
					if (releases.length === 0) return
					const selectedIds = $selectedReleaseIds
					if (selectedIds.size === 0) {
						uiStore.selectRelease(releases[releases.length - 1].id)
					} else {
						const firstSelectedId = [...selectedIds][0]
						const currentIndex = releases.findIndex((r) => r.id === firstSelectedId)
						if (currentIndex > 0) uiStore.selectRelease(releases[currentIndex - 1].id)
					}
					return
				}
				const tracks = $displayedTracks
				if (tracks.length === 0) return
				const selectedIds = $selectedTrackIds
				if (selectedIds.size === 0) {
					uiStore.selectTrack(tracks[tracks.length - 1].id)
				} else {
					const firstSelectedId = [...selectedIds][0]
					const currentIndex = tracks.findIndex((t) => t.id === firstSelectedId)
					if (currentIndex > 0) uiStore.selectTrack(tracks[currentIndex - 1].id)
				}
			},
			onSelectNextTrack: () => {
				if ($activeView === 'discovery') {
					const releases = $sortedReleases
					if (releases.length === 0) return
					const selectedIds = $selectedReleaseIds
					if (selectedIds.size === 0) {
						uiStore.selectRelease(releases[0].id)
					} else {
						const lastSelectedId = [...selectedIds].pop()
						const currentIndex = releases.findIndex((r) => r.id === lastSelectedId)
						if (currentIndex >= 0 && currentIndex < releases.length - 1)
							uiStore.selectRelease(releases[currentIndex + 1].id)
					}
					return
				}
				const tracks = $displayedTracks
				if (tracks.length === 0) return
				const selectedIds = $selectedTrackIds
				if (selectedIds.size === 0) {
					uiStore.selectTrack(tracks[0].id)
				} else {
					const lastSelectedId = [...selectedIds].pop()
					const currentIndex = tracks.findIndex((t) => t.id === lastSelectedId)
					if (currentIndex >= 0 && currentIndex < tracks.length - 1) uiStore.selectTrack(tracks[currentIndex + 1].id)
				}
			},
			onQuickExport: () => {
				if (devices.length > 0 && modalOrchestrator) modalOrchestrator.openQuickExportModal()
			},
			onJumpToPlayingTrack: () => {
				const track = $currentTrack
				if (!track) return
				if (selectedPlaylistId) playlistController.handleLibraryClick()
				uiStore.selectTrack(track.id)
			},
			onToggleView: () => {
				if (modalOrchestrator?.isModalOpen()) return
				const next = $activeView === 'library' ? 'discovery' : 'library'
				handleViewChange(next)
			},
			onAddRelease: async () => {
				if ($activeView !== 'discovery') {
					handleViewChange('discovery')
					await tick()
				}
				showAddReleaseModal = true
			},
			onRefreshMetadata: async () => {
				if ($activeView !== 'discovery') return
				const ids = [...$selectedReleaseIds]
				if (ids.length === 0) return
				await Promise.all(ids.map((id) => discoveryStore.refreshMetadata(id)))
			},
		})

		// Set up menu action listener
		const cleanupMenu = await useMenuActions({
			onImport: async () => {
				if ($activeView !== 'library') {
					handleViewChange('library')
					await tick()
				}
				await trackController.handleImport()
			},
			onAddRelease: async () => {
				if ($activeView !== 'discovery') {
					handleViewChange('discovery')
					await tick()
				}
				showAddReleaseModal = true
			},
			onCreatePlaylist: playlistController.handleCreatePlaylist,
			onCreateFolder: playlistController.handleCreateFolder,
			onSelectAll: () => {
				if ($activeView === 'discovery') {
					uiStore.setSelectedReleases(new Set($sortedReleases.map((r) => r.id)))
				} else {
					const allIds = new Set($sortedTracks.map((t) => t.id))
					uiStore.setSelectedTracks(allIds)
				}
			},
			onPlayPause: () => playerStore.togglePlayPause(),
			onStop: () => playerStore.stop(),
			onNextTrack: playNextTrack,
			onPreviousTrack: playPreviousTrack,
			onSeekForward: () => playerStore.seekRelative(10000),
			onSeekBackward: () => playerStore.seekRelative(-10000),
			onFineSeekForward: () => playerStore.seekRelative(1000),
			onFineSeekBackward: () => playerStore.seekRelative(-1000),
			onVolumeUp: () => playerStore.adjustVolume(0.1),
			onVolumeDown: () => playerStore.adjustVolume(-0.1),
			onToggleMute: () => playerStore.toggleMute(),
			onOpenSettings: (tab?: SettingsPage) => modalOrchestrator?.openSettingsModal(tab),
			onQuickExport: () => {
				if (devices.length > 0 && modalOrchestrator) modalOrchestrator.openQuickExportModal()
			},
			onJumpToPlayingTrack: () => {
				const track = $currentTrack
				if (!track) return
				if (selectedPlaylistId) playlistController.handleLibraryClick()
				uiStore.selectTrack(track.id)
			},
			onToggleView: () => {
				if (modalOrchestrator?.isModalOpen()) return
				const next = $activeView === 'library' ? 'discovery' : 'library'
				handleViewChange(next)
			},
			onToggleEditor: () => uiStore.toggleRightSidebar(),
			onExpandAllReleases: () => {
				const releases = $sortedReleases
				expandedReleaseIds.expandAll(releases.filter((r) => r.tracks.length > 0).map((r) => r.id))
			},
			onCollapseAllReleases: () => expandedReleaseIds.collapseAll(),
			onRefreshMetadata: async () => {
				const ids = [...$selectedReleaseIds]
				if (ids.length === 0) return
				await Promise.all(ids.map((id) => discoveryStore.refreshMetadata(id)))
			},
		})

		// Set up media key listeners (OS-level via souvlaki)
		const cleanupMediaKeys = await useMediaKeys({
			onPlayPause: () => playerStore.togglePlayPause(),
			onNextTrack: playNextTrack,
			onPreviousTrack: playPreviousTrack,
		})

		// Register track-end callback for continuous playback
		playerStore.onTrackEnd(() => {
			if (get(continuousPlayback)) {
				playNextTrack()
			}
		})

		// Ensure minimum display time for splash screen
		const elapsed = Date.now() - splashStartTime
		if (elapsed < minDisplayTime) {
			await new Promise((r) => setTimeout(r, minDisplayTime - elapsed))
		}

		// Load discovery releases if that's the persisted view
		if (get(activeView) === 'discovery') {
			discoveryStore.loadReleases()
		}

		// Check for updates (non-dev only, guarded inside store)
		updaterStore.check(true)

		// Hourly recheck
		const updateInterval = setInterval(() => updaterStore.check(true), 60 * 60 * 1000)

		// Dismiss splash screen
		dismissSplash()

		return () => {
			cleanupApp()
			cleanupKeyboard()
			cleanupMenu()
			cleanupMediaKeys()
			playerStore.onTrackEnd(null)
			exportStore.stopListening()
			clearInterval(updateInterval)
		}
	}

	onMount(() => {
		onMountHelper().then((cleanupFn) => {
			cleanupOnMount = cleanupFn
		})
	})

	onDestroy(() => {
		cleanupOnMount?.()
	})

	// =============================================================================
	// Simple Handlers
	// =============================================================================

	function handleSortChange(config: SortConfig) {
		sortConfig = config
		libraryStore.setSort(config)
	}

	function handleSidebarResize(delta: number) {
		uiStore.setSidebarWidth(sidebarWidth + delta)
	}

	function handleRightSidebarResize(delta: number) {
		uiStore.setRightSidebarWidth($rightSidebarWidth + delta)
	}

	async function handleToggleDevTools() {
		if ($devToolsOpen) {
			await closeDevTools()
		} else {
			await openDevTools()
		}
		appStore.toggleDevTools()
	}

	async function handleTrackAnalyze(tracks: Track[]) {
		const trackIds = tracks.map((t) => t.id)
		try {
			await analysisStore.analyzeTracks(trackIds)
		} catch (error) {
			console.error('Analysis failed:', error)
			toastStore.error(get(translate)('errors.analysisFailed'))
		}
	}

	async function handleCancelAnalysis(trackId: string) {
		await analysisStore.cancelTrackAnalysis(trackId)
	}

	// =============================================================================
	// Discovery Handlers
	// =============================================================================

	function handleViewChange(view: ActiveView) {
		// Cache discovery playlist releases before switching away
		if (selectedPlaylistId && discoveryPlaylistReleases.length > 0) {
			discoveryPlaylistReleasesCache.set(selectedPlaylistId, discoveryPlaylistReleases)
		}

		// setActiveView saves current navigation and restores the target view's cached state
		uiStore.setActiveView(view)

		// Read the restored navigation state
		const ui = get(uiStore)
		const restoredPlaylistId = ui.selectedPlaylistId
		const restoredFolderId = ui.selectedFolderId

		if (restoredPlaylistId) {
			// Restore data for the cached playlist
			const playlist = playlists.find((p) => p.id === restoredPlaylistId)
			if (playlist) {
				if (playlist.context === 'discovery') {
					const cached = discoveryPlaylistReleasesCache.get(restoredPlaylistId)
					if (cached) {
						discoveryPlaylistReleases = cached
					} else {
						const fetchReleases = playlist.is_smart
							? playlistsApi.getSmartPlaylistReleases(restoredPlaylistId)
							: playlistsStore.getPlaylistReleases(restoredPlaylistId)
						fetchReleases.then((releases) => {
							discoveryPlaylistReleases = releases
						})
					}
				} else if (playlist.is_smart) {
					libraryStore.loadSmartPlaylistTracks(restoredPlaylistId)
				} else {
					libraryStore.loadPlaylistTracks(restoredPlaylistId)
				}
			}
		} else if (!restoredFolderId) {
			// Base-level view: load main data
			discoveryPlaylistReleases = []
			if (view === 'discovery') {
				const filter: DiscoveryFilter = {}
				if (selectedTagIds.length > 0) {
					filter.tag_ids = selectedTagIds
					filter.tag_filter_mode = $tagFilterMode
				}
				discoveryStore.loadReleases(Object.keys(filter).length > 0 ? filter : undefined)
			} else {
				const filter: TrackFilter = {}
				if (selectedTagIds.length > 0) {
					filter.tag_ids = selectedTagIds
					filter.tag_filter_mode = $tagFilterMode
				}
				libraryStore.loadTracks(Object.keys(filter).length > 0 ? filter : undefined)
			}
		}
	}

	function handleDiscoverySortChange(config: DiscoverySortConfig) {
		discoverySortConfig = config
		discoveryStore.setSort(config)
	}

	const PREVIEWABLE_SOURCES = new Set(['bandcamp', 'soundcloud', 'youtube'])

	function releaseHasAnyPreviewableTrack(release: DiscoveryRelease): boolean {
		if (PREVIEWABLE_SOURCES.has(release.source_type)) return release.tracks.length > 0
		return release.tracks.some((t) => t.video_id !== null)
	}

	function handleReleaseOpen(release: DiscoveryRelease) {
		if (releaseHasAnyPreviewableTrack(release)) {
			const firstPlayable = release.source_type === 'discogs' ? release.tracks.findIndex((t) => t.video_id !== null) : 0
			if (firstPlayable >= 0) {
				playerStore.playPreview(release, firstPlayable)
				return
			}
		}
		openUrl(release.url)
	}

	function handleTrackPlayInRelease(release: DiscoveryRelease, trackIndex: number) {
		uiStore.clearReleaseSelection()
		const track = release.tracks[trackIndex]
		const canPlay =
			PREVIEWABLE_SOURCES.has(release.source_type) || (release.source_type === 'discogs' && track?.video_id !== null)
		if (canPlay && release.tracks.length > 0) {
			playerStore.playPreview(release, trackIndex)
		}
	}

	function handleReleaseSelectionChange(ids: Set<string>) {
		uiStore.setSelectedReleases(ids)
	}

	async function handleAddRelease(create: DiscoveryReleaseCreate) {
		const release = await discoveryStore.createRelease(create)
		if (release) {
			showAddReleaseModal = false
			if (release.tracks.length > 0) {
				expandedReleaseIds.expand(release.id)
			}
			// Refresh smart playlist counts and view contents
			const playlist = playlists.find((p) => p.id === selectedPlaylistId)
			if (playlist?.is_smart && playlist.context === 'discovery') {
				discoveryPlaylistReleases = await playlistsApi.getSmartPlaylistReleases(playlist.id)
				discoveryPlaylistReleasesCache.set(playlist.id, discoveryPlaylistReleases)
			}
			await playlistsStore.load()
		}
	}

	function handleRelocateComplete(updatedTrack: Track) {
		libraryStore.loadTracks()
		toastStore.success(`Relocated "${updatedTrack.title || 'track'}"`)
	}

	// =============================================================================
	// Context Menu Handlers
	// =============================================================================

	function handleTrackContextMenu(e: MouseEvent, track: Track) {
		const currentSelection = $selectedTrackIds
		let tracks: Track[]
		if (currentSelection.has(track.id)) {
			tracks = $sortedTracks.filter((t) => currentSelection.has(t.id))
		} else {
			tracks = [track]
		}
		contextMenuOrchestrator.openTrackMenu(e, tracks)
	}

	function handlePlaylistItemClick(playlist: Playlist, newSelectedIds: Set<string>, isModifierClick: boolean) {
		selectedTreeIds = newSelectedIds
		// Navigation is handled by PlaylistTree's onSelect (plain clicks only)
	}

	function handlePlaylistMultiContextMenu(e: MouseEvent, playlists: Playlist[]) {
		contextMenuPlaylistId = null
		contextMenuOrchestrator.openPlaylistMenu(e, playlists, 'tree')
	}

	function handlePlaylistContextMenu(e: MouseEvent, playlist: Playlist) {
		contextMenuPlaylistId = playlist.id
		contextMenuOrchestrator.openPlaylistMenu(e, playlist, 'tree')
	}

	function handleDeviceContextMenu(e: MouseEvent, device: UsbDevice) {
		contextMenuOrchestrator.openDeviceMenu(e, device)
	}

	function handleTagContextMenu(e: MouseEvent, tag: Tag, category: TagCategory) {
		contextMenuOrchestrator.openTagMenu(e, { type: 'tag', tag, category })
	}

	function handleCategoryContextMenu(e: MouseEvent, category: TagCategory) {
		contextMenuOrchestrator.openTagMenu(e, { type: 'category', category })
	}

	function handleReleaseContextMenu(e: MouseEvent, release: DiscoveryRelease) {
		const currentSelection = $selectedReleaseIds
		let releases: DiscoveryRelease[]
		if (currentSelection.has(release.id)) {
			releases = $sortedReleases.filter((r) => currentSelection.has(r.id))
		} else {
			releases = [release]
		}
		contextMenuOrchestrator.openDiscoveryReleaseMenu(e, releases)
	}

	function handleDiscoveryReleaseOpenInBrowser(release: DiscoveryRelease) {
		openUrl(release.url)
	}

	function handleDiscoveryReleaseImport(release: DiscoveryRelease) {
		purchaseRelease = release
	}

	async function handlePurchaseComplete(result: ImportResultWithDuplicates) {
		purchaseRelease = null
		uiStore.clearReleaseSelection()

		if (result.tracks.length > 0) {
			libraryStore.addTracksToState(result.tracks)

			if (get(settingsStore).autoAnalyzeOnImport) {
				analysisStore
					.analyzeTracks(result.tracks.map((t) => t.id))
					.catch((error) => console.error('Auto-analysis failed:', error))
			}
		}

		if (result.duplicates.length > 0) {
			modalOrchestrator.openDuplicateTrackModal(result.duplicates, () => {
				libraryStore.loadTracks()
			})
		}
	}

	async function handleDiscoveryReleaseRefreshMetadata(release: DiscoveryRelease) {
		await discoveryStore.refreshMetadata(release.id)
	}

	async function handleEditorSave() {
		const playlist = playlists.find((p) => p.id === selectedPlaylistId)
		if (playlist?.is_smart) {
			if (playlist.context === 'discovery') {
				discoveryPlaylistReleases = await playlistsApi.getSmartPlaylistReleases(playlist.id)
				discoveryPlaylistReleasesCache.set(playlist.id, discoveryPlaylistReleases)
			} else {
				await libraryStore.loadSmartPlaylistTracks(playlist.id)
			}
			await playlistsStore.load()
		}
	}

	function handleBreadcrumbNavigate(item: BreadcrumbItem) {
		if (item.id === null) {
			playlistController.handleLibraryClick()
		} else if (item.playlist) {
			playlistController.handlePlaylistSelect(item.playlist)
		}
	}

	function handleBreadcrumbContextMenu(e: MouseEvent, item: BreadcrumbItem) {
		if (item.type === 'library') return
		if (item.playlist) {
			contextMenuOrchestrator.openPlaylistMenu(e, item.playlist, 'tree')
		}
	}

	// =============================================================================
	// Derived State
	// =============================================================================

	let selectedTracksArray = $derived($displayedTracks.filter((t) => $selectedTrackIds.has(t.id)))
	let selectedReleasesArray = $derived($sortedReleases.filter((r) => $selectedReleaseIds.has(r.id)))
	const contextPlaylists = $derived(playlists.filter((p) => p.context === $activeView))
	const playlistFolders = $derived(contextPlaylists.filter((p) => p.is_folder))
	const categoryColors = $derived(new Map(tagCategories.map((c) => [c.id, c.color])))
	const categorySortOrders = $derived(new Map(tagCategories.map((c) => [c.id, c.sort_order])))

	const activeFilterTags = $derived.by(() => {
		if (selectedTagIds.length === 0) return []
		const tags: Tag[] = []
		for (const category of tagCategories) {
			for (const tag of category.tags) {
				if (selectedTagIds.includes(tag.id)) tags.push(tag)
			}
		}
		return tags
	})

	const tagColors = $derived(new Map(tagCategories.map((c) => [c.id, c.color])))

	const currentFolderChildCount = $derived(
		selectedFolderId ? getPlaylistChildren(contextPlaylists, selectedFolderId).length : 0
	)

	// Discovery playlist releases (loaded when a discovery playlist is selected)
	let discoveryPlaylistReleases = $state<DiscoveryRelease[]>([])

	// Cache discovery playlist releases per playlist ID so switching views doesn't require re-fetching
	const discoveryPlaylistReleasesCache = new SvelteMap<string, DiscoveryRelease[]>()

	// Combine all available releases for drag preview lookups
	const allAvailableReleases = $derived.by(() => {
		const sorted = $sortedReleases
		if (discoveryPlaylistReleases.length === 0) return sorted
		const idSet = new Set(sorted.map((r) => r.id))
		const extra = discoveryPlaylistReleases.filter((r) => !idSet.has(r.id))
		return extra.length > 0 ? [...sorted, ...extra] : sorted
	})

	const breadcrumbItems = $derived(
		buildBreadcrumbItems(
			contextPlaylists,
			selectedFolderId,
			selectedPlaylistId,
			selectedPlaylistId
				? $activeView === 'discovery'
					? discoveryPlaylistReleases.length
					: $displayedTracks.length
				: undefined,
			currentFolderChildCount,
			$activeView
		)
	)
</script>

<!-- Main App Content -->
{#if !$splashVisible}
	<div class="flex h-full flex-col" in:fade={{ duration: 300, delay: 100 }}>
		<!-- Unified Header: Logo + Toolbar -->
		<div class="flex rounded-br bg-surface-1">
			<div class="flex flex-shrink-0 items-center justify-center gap-2" style="width: {sidebarWidth}px">
				<Icon name="logo" class="h-6 w-6 text-brand-primary" fill />
				<Text variant="header-1" as="span" weight="bold">Crate</Text>
				{#if $isDev}
					<span class="rounded bg-amber-500/20 px-1.5 py-0.5 text-xs font-medium text-amber-500">DEV</span>
				{/if}
			</div>
			<Toolbar
				activeView={$activeView}
				{activeFilterTags}
				{tagColors}
				tagFilterMode={$tagFilterMode}
				onViewChange={handleViewChange}
				onRemoveTagFilter={tagController.removeTagFilter}
				onClearAllTagFilters={tagController.clearTagFilters}
				onToggleTagFilterMode={tagController.toggleTagFilterMode}
				onImport={$activeView === 'library' ? trackController.handleImport : undefined}
				onAddRelease={$activeView === 'discovery' ? () => (showAddReleaseModal = true) : undefined}
				onSettings={() => modalOrchestrator.openSettingsModal()}
				onDevTools={handleToggleDevTools}
			/>
		</div>

		<div class="relative flex flex-1 overflow-hidden bg-surface-1">
			<!-- Left: Sidebar -->
			<div class="flex-shrink-0" style="width: {sidebarWidth}px">
				<Sidebar
					playlists={contextPlaylists}
					{tagCategories}
					{devices}
					{selectedPlaylistId}
					{selectedFolderId}
					{contextMenuPlaylistId}
					{selectedTagIds}
					selectedTrackIds={$activeView === 'discovery' ? $selectedReleaseIds : $selectedTrackIds}
					{selectedTreeIds}
					{tagStates}
					{tagCounts}
					trackCount={$activeView === 'discovery' ? $releaseCount : $trackCount}
					showHeader={false}
					onLibraryClick={() => {
						selectedTreeIds = new Set()
						playlistController.handleLibraryClick()
					}}
					onPlaylistSelect={playlistController.handlePlaylistSelect}
					onPlaylistItemClick={handlePlaylistItemClick}
					onPlaylistContextMenu={handlePlaylistContextMenu}
					onPlaylistMultiContextMenu={handlePlaylistMultiContextMenu}
					onPlaylistTreeContextMenu={(e) => contextMenuOrchestrator.openPlaylistTreeMenu(e)}
					onDeviceContextMenu={handleDeviceContextMenu}
					onCancelExport={exportController.handleExportCancel}
					onTagSelect={tagController.selectTag}
					onTagToggle={tagController.toggleTagOnTracks}
					onTagContextMenu={handleTagContextMenu}
					onCategoryContextMenu={handleCategoryContextMenu}
					onCreatePlaylist={playlistController.handleCreatePlaylist}
					onCreateSmartPlaylist={() => playlistController.handleCreateSmartPlaylist($activeView)}
					onCreateFolder={playlistController.handleCreateFolder}
					onCreateCategory={() => modalOrchestrator.openCreateCategoryModal()}
					onCreateTag={(categoryId) => modalOrchestrator.openCreateTagModal(categoryId)}
					onTagsWhitespaceContextMenu={(e) => contextMenuOrchestrator.openTagsSidebarMenu(e)}
					onTracksDrop={trackController.handleTracksDropOnPlaylist}
					onPlaylistMove={playlistController.handlePlaylistDragMove}
				/>
			</div>

			<ResizeHandle onResize={handleSidebarResize} />

			<!-- Right: Main Content + Optional TrackEditor -->
			<div class="flex flex-1 overflow-hidden rounded-tl-md border-t border-l border-stroke">
				<div class="flex-1 overflow-hidden bg-surface-0">
					{#if selectedFolderId}
						<FolderView
							folderId={selectedFolderId}
							playlists={contextPlaylists}
							onSelect={playlistController.handlePlaylistSelect}
							{breadcrumbItems}
							onBreadcrumbNavigate={handleBreadcrumbNavigate}
							onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
							onEmptySpaceContextMenu={(e, folderId) => contextMenuOrchestrator.openFolderViewMenu(e, folderId)}
							onCardContextMenu={(e, playlist) => contextMenuOrchestrator.openPlaylistMenu(e, playlist, 'folder')}
						/>
					{:else if selectedPlaylistId}
						{@const playlist = contextPlaylists.find((p) => p.id === selectedPlaylistId)}
						{#if playlist}
							{#if playlist.context === 'discovery'}
								<PlaylistView
									{playlist}
									isDiscovery
									releases={discoveryPlaylistReleases}
									tracks={[]}
									selectedIds={$selectedReleaseIds}
									{sortConfig}
									{categoryColors}
									{categorySortOrders}
									{breadcrumbItems}
									editorVisible={$rightSidebarVisible}
									hasSelection={selectedReleasesArray.length > 0}
									onSelectionChange={handleReleaseSelectionChange}
									onDiscoveryTrackPlay={handleTrackPlayInRelease}
									onContextMenu={(e, item) => {
										handleReleaseContextMenu(e, item as unknown as DiscoveryRelease)
									}}
									onBreadcrumbNavigate={handleBreadcrumbNavigate}
									onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
									onToggleEditor={() => uiStore.toggleRightSidebar()}
								/>
							{:else}
								<PlaylistView
									{playlist}
									tracks={$displayedTracks}
									selectedIds={$selectedTrackIds}
									playingTrackId={$currentTrack?.id ?? null}
									{sortConfig}
									{isDragOver}
									{categoryColors}
									{categorySortOrders}
									{breadcrumbItems}
									editorVisible={$rightSidebarVisible}
									hasSelection={selectedTracksArray.length > 0}
									onSelectionChange={trackController.handleSelectionChange}
									onTrackPlay={trackController.play}
									onSortChange={handleSortChange}
									onContextMenu={handleTrackContextMenu}
									onEmptySpaceContextMenu={(e, pl) => contextMenuOrchestrator.openPlaylistViewMenu(e, pl)}
									onBreadcrumbNavigate={handleBreadcrumbNavigate}
									onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
									onTrackColorChange={trackController.setColor}
									onCancelAnalysis={handleCancelAnalysis}
									onToggleEditor={() => uiStore.toggleRightSidebar()}
								/>
							{/if}
						{/if}
					{:else if $activeView === 'discovery'}
						<DiscoveryView
							releases={$sortedReleases}
							releaseCount={$releaseCount}
							selectedIds={$selectedReleaseIds}
							sortConfig={discoverySortConfig}
							{categoryColors}
							{categorySortOrders}
							editorVisible={$rightSidebarVisible}
							hasSelection={selectedReleasesArray.length > 0}
							onSelectionChange={handleReleaseSelectionChange}
							onReleaseOpen={handleReleaseOpen}
							onReleaseOpenUrl={handleDiscoveryReleaseOpenInBrowser}
							onReleaseImport={handleDiscoveryReleaseImport}
							onTrackPlay={handleTrackPlayInRelease}
							onSortChange={handleDiscoverySortChange}
							onContextMenu={handleReleaseContextMenu}
							onEmptySpaceContextMenu={(e) => contextMenuOrchestrator.openDiscoveryViewMenu(e)}
							onUrlDrop={async (url) => {
								await handleAddRelease({ url })
							}}
							onToggleEditor={() => uiStore.toggleRightSidebar()}
						/>
					{:else}
						<LibraryView
							tracks={$displayedTracks}
							trackCount={$trackCount}
							selectedIds={$selectedTrackIds}
							playingTrackId={$currentTrack?.id ?? null}
							{sortConfig}
							{isDragOver}
							{categoryColors}
							{categorySortOrders}
							editorVisible={$rightSidebarVisible}
							hasSelection={selectedTracksArray.length > 0}
							onSelectionChange={trackController.handleSelectionChange}
							onTrackPlay={trackController.play}
							onSortChange={handleSortChange}
							onContextMenu={handleTrackContextMenu}
							onEmptySpaceContextMenu={(e) => contextMenuOrchestrator.openLibraryViewMenu(e)}
							onTrackColorChange={trackController.setColor}
							onCancelAnalysis={handleCancelAnalysis}
							onToggleEditor={() => uiStore.toggleRightSidebar()}
						/>
					{/if}
				</div>

				<RightSidebar
					hasContent={$activeView === 'discovery' ? selectedReleasesArray.length > 0 : selectedTracksArray.length > 0}
					isVisible={$rightSidebarVisible}
					width={$rightSidebarWidth}
					onResize={handleRightSidebarResize}
				>
					{#if $activeView === 'discovery'}
						<div class="h-full" in:fade={{ duration: 150 }}>
							<DiscoveryEditor
								selectedReleases={selectedReleasesArray}
								onImport={handleDiscoveryReleaseImport}
								onSave={handleEditorSave}
							/>
						</div>
					{:else}
						<div class="h-full" in:fade={{ duration: 150 }}>
							<TrackEditor selectedTracks={selectedTracksArray} onSave={handleEditorSave} />
						</div>
					{/if}
				</RightSidebar>
			</div>
		</div>

		<Player onNext={playNextTrack} onPrevious={playPreviousTrack} />
	</div>
{/if}

<!-- Context Menu Orchestrator -->
<ContextMenuOrchestrator
	bind:this={contextMenuOrchestrator}
	{playlists}
	currentPlaylistId={selectedPlaylistId}
	{playlistFolders}
	categoryCount={tagCategories.length}
	onTrackAddToPlaylist={trackController.addToPlaylist}
	onTrackRevealInExplorer={trackController.revealInExplorer}
	onTrackRemoveFromPlaylist={trackController.removeFromPlaylistClick}
	onTrackRemoveFromLibrary={trackController.removeFromLibraryClick}
	onTrackRelocate={(track) => modalOrchestrator.openRelocateModal(track)}
	onTrackSetColor={trackController.setColorFromContextMenu}
	onTrackAnalyze={handleTrackAnalyze}
	onPlaylistCreatePlaylist={(p) => modalOrchestrator.openCreatePlaylistModal(p.id)}
	onPlaylistCreateSmartPlaylist={(p) => modalOrchestrator.openCreateSmartPlaylistModal(p.id, p.context)}
	onPlaylistCreateFolder={(p) => modalOrchestrator.openCreateFolderModal(p.id)}
	onPlaylistEditSmartPlaylist={(p) => modalOrchestrator.openEditSmartPlaylistModal(p)}
	onPlaylistRename={playlistController.handlePlaylistRename}
	onPlaylistDelete={playlistController.handlePlaylistDelete}
	onPlaylistBulkDelete={(playlists) => modalOrchestrator.openDeletePlaylistBulkModal(playlists)}
	onPlaylistMove={playlistController.handlePlaylistMove}
	onFolderViewCreatePlaylist={(folderId) => modalOrchestrator.openCreatePlaylistModal(folderId)}
	onFolderViewCreateSmartPlaylist={(folderId) => modalOrchestrator.openCreateSmartPlaylistModal(folderId, $activeView)}
	onFolderViewCreateFolder={(folderId) => modalOrchestrator.openCreateFolderModal(folderId)}
	onPlaylistTreeCreatePlaylist={() => modalOrchestrator.openCreatePlaylistModal(null)}
	onPlaylistTreeCreateSmartPlaylist={() => modalOrchestrator.openCreateSmartPlaylistModal(null, $activeView)}
	onPlaylistTreeCreateFolder={() => modalOrchestrator.openCreateFolderModal(null)}
	onLibraryViewImport={trackController.handleImport}
	onDiscoveryViewAddRelease={() => (showAddReleaseModal = true)}
	onPlaylistViewImport={playlistController.handlePlaylistViewImport}
	{tagCategories}
	onTagAddTag={(categoryId) => modalOrchestrator.openCreateTagModal(categoryId)}
	onTagRename={(tag) => modalOrchestrator.openRenameTagModal(tag)}
	onTagDelete={(tag) => modalOrchestrator.openDeleteTagModal(tag)}
	onTagMove={async (tag, targetCategoryId) => {
		try {
			await tagsStore.moveTag(tag.id, targetCategoryId)
			libraryStore.updateTagCategory(tag.id, targetCategoryId)
			discoveryStore.updateTagCategory(tag.id, targetCategoryId)
			discoveryPlaylistReleases = discoveryPlaylistReleases.map((r) => ({
				...r,
				tags: r.tags.map((t) => (t.id === tag.id ? { ...t, category_id: targetCategoryId } : t)),
			}))
		} catch (error) {
			const message = error instanceof Error ? error.message : get(translate)('errors.tagNameConflict')
			toastStore.error(message)
		}
	}}
	onCategoryRename={(category) => modalOrchestrator.openRenameCategoryModal(category)}
	onCategoryDelete={(category) => modalOrchestrator.openDeleteCategoryModal(category)}
	onCategoryChangeColor={tagController.changeCategoryColor}
	onTagsSidebarAddCategory={() => modalOrchestrator.openCreateCategoryModal()}
	onDeviceViewInfo={deviceController.handleViewDeviceInfo}
	onDeviceRevealInFinder={deviceController.handleDeviceRevealInFinder}
	onDeviceReformat={deviceController.handleDeviceReformat}
	onDeviceEject={deviceController.handleEjectDevice}
	onDeviceExport={exportController.handleDeviceExport}
	onDeviceIgnore={deviceController.handleDeviceIgnore}
	onPlaylistExport={exportController.handlePlaylistExport}
	onDiscoveryReleaseOpenInBrowser={handleDiscoveryReleaseOpenInBrowser}
	onDiscoveryReleaseRefreshMetadata={handleDiscoveryReleaseRefreshMetadata}
	onDiscoveryReleaseImport={handleDiscoveryReleaseImport}
	onDiscoveryReleaseDelete={(releaseIds) => modalOrchestrator.openRemoveDiscoveryReleasesModal(releaseIds)}
	onDiscoveryReleaseRemoveFromPlaylist={(playlistId, releaseIds) =>
		modalOrchestrator.openRemoveDiscoveryReleasesFromPlaylistModal(releaseIds, playlistId)}
	onDiscoveryReleaseMerge={(releases) => (mergeReleases = releases)}
	onDiscoveryReleaseAddToPlaylist={async (playlistId, releases) => {
		const releaseIds = releases.map((r) => r.id)
		await playlistsStore.addReleases(playlistId, releaseIds)
	}}
	onClose={() => {
		contextMenuPlaylistId = null
	}}
/>

<!-- Modal Orchestrator -->
<ModalOrchestrator
	bind:this={modalOrchestrator}
	{playlists}
	{tagCategories}
	onModalOpenChange={(isOpen) => {
		setMenuItemEnabled('toggle_view', !isOpen)
	}}
	onCreatePlaylist={async (name, parentId) => {
		const context = $activeView
		const playlist = await playlistsStore.createPlaylist(name, parentId ?? undefined, context)
		if (playlist) {
			uiStore.selectPlaylist(playlist.id)
			if (context === 'library') {
				await libraryStore.loadPlaylistTracks(playlist.id)
			}
		}
		return playlist
	}}
	onCreateFolder={async (name, parentId) => {
		const context = $activeView
		const folder = await playlistsStore.createFolder(name, parentId ?? undefined, context)
		if (folder) {
			uiStore.selectFolder(folder.id)
		}
		return folder
	}}
	onCreateCategory={async (name) => {
		const categories = get(tagsStore).categories
		const accent = get(settingsStore).accentColor
		const color = pickTagCategoryColor(categories, accent)
		await tagsStore.createCategory(name, color)
	}}
	onCreateTag={async (categoryId, name) => {
		await tagsStore.createTag(categoryId, name)
	}}
	onRenamePlaylist={async (id, name) => {
		await playlistsStore.rename(id, name)
	}}
	onRenameTag={async (id, name) => {
		await tagsStore.updateTag(id, name)
	}}
	onRenameCategory={async (id, name) => {
		await tagsStore.updateCategory(id, name)
	}}
	onDeletePlaylist={async (id, deleteTracksToo) => {
		const playlist = playlists.find((p) => p.id === id)
		const parentId = playlist?.parent_id ?? null
		const context = playlist?.context
		discoveryPlaylistReleasesCache.delete(id)
		await playlistsStore.delete(id, deleteTracksToo)
		if (deleteTracksToo) {
			if (context === 'discovery') {
				await discoveryStore.loadReleases()
			} else {
				await libraryStore.loadTracks()
			}
			await playlistsStore.load()
		}
		if (parentId) {
			const parentFolder = playlists.find((p) => p.id === parentId)
			if (parentFolder) {
				uiStore.selectFolder(parentId)
			} else {
				playlistController.handleLibraryClick()
			}
		} else {
			playlistController.handleLibraryClick()
		}
	}}
	onDeletePlaylistBulk={async (ids, deleteTracksToo) => {
		// Smart ordering: skip children whose ancestors are also in the set
		const idSet = new Set(ids)
		const topLevel = ids.filter((id) => {
			let current = playlists.find((p) => p.id === id)
			while (current?.parent_id) {
				if (idSet.has(current.parent_id)) return false
				current = playlists.find((p) => p.id === current!.parent_id)
			}
			return true
		})

		for (const id of topLevel) {
			discoveryPlaylistReleasesCache.delete(id)
			await playlistsStore.delete(id, deleteTracksToo)
		}

		if (deleteTracksToo) {
			await libraryStore.loadTracks()
			await discoveryStore.loadReleases()
			await playlistsStore.load()
		}

		selectedTreeIds = new Set()
		playlistController.handleLibraryClick()
	}}
	onDeleteTag={async (id) => {
		await tagsStore.deleteTag(id)
		await libraryStore.loadTracks()
	}}
	onDeleteCategory={async (id) => {
		await tagsStore.deleteCategory(id)
		await libraryStore.loadTracks()
	}}
	onRemoveFromPlaylist={async (trackIds, playlistId, deleteFromCollection) => {
		await playlistsStore.removeTracks(playlistId, trackIds)
		if (deleteFromCollection) {
			await libraryStore.deleteTracks(trackIds)
			await playlistsStore.load()
		} else {
			await libraryStore.loadPlaylistTracks(playlistId)
		}
		uiStore.clearSelection()
		const count = trackIds.length
		toastStore.success(count === 1 ? '1 track removed from playlist' : `${count} tracks removed from playlist`)
	}}
	onRemoveDiscoveryReleases={async (releaseIds) => {
		await discoveryStore.deleteReleases(releaseIds)
		uiStore.clearReleaseSelection()
		await playlistsStore.load()
	}}
	onRemoveDiscoveryReleasesFromPlaylist={async (releaseIds, playlistId, deleteFromCollection) => {
		await playlistsStore.removeReleases(playlistId, releaseIds)
		discoveryPlaylistReleases = discoveryPlaylistReleases.filter((r) => !releaseIds.includes(r.id))
		discoveryPlaylistReleasesCache.set(playlistId, discoveryPlaylistReleases)
		if (deleteFromCollection) {
			await discoveryStore.deleteReleases(releaseIds)
		}
		uiStore.clearReleaseSelection()
	}}
	onRemoveFromLibrary={async (trackIds) => {
		await libraryStore.deleteTracks(trackIds)
		uiStore.clearSelection()
		if (selectedPlaylistId) {
			await libraryStore.loadPlaylistTracks(selectedPlaylistId)
		}
		await playlistsStore.load()
		const count = trackIds.length
		toastStore.success(count === 1 ? '1 track removed from library' : `${count} tracks removed from library`)
	}}
	onMoveConflictOverwrite={async (movingItemId, targetParentId) => {
		const result = await playlistsStore.moveWithResolution(movingItemId, targetParentId, 'overwrite')
		return !!result
	}}
	onMoveConflictMerge={async (movingItemId, targetParentId) => {
		const result = await playlistsStore.moveWithResolution(movingItemId, targetParentId, 'merge')
		return {
			success: !!result,
			nestedConflicts: result?.nestedConflicts ?? [],
		}
	}}
	onTagInputSubmit={async (categoryId, tagName) => {
		await tagsStore.createTag(categoryId, tagName)
	}}
	onCreateSmartPlaylist={async (name, smartRules, parentId, context) => {
		const playlist = await playlistsStore.createSmartPlaylist(name, smartRules, parentId ?? undefined, context)
		if (playlist) {
			playlistController.handlePlaylistSelect(playlist)
		}
		return playlist
	}}
	onUpdateSmartRules={async (id, smartRules) => {
		await playlistsStore.updateSmartRules(id, smartRules)
		// Reload the smart playlist tracks to reflect new rules
		const playlist = playlists.find((p) => p.id === id)
		if (playlist && selectedPlaylistId === id) {
			if (playlist.context === 'discovery') {
				discoveryPlaylistReleases = await playlistsApi.getSmartPlaylistReleases(id)
			} else {
				await libraryStore.loadSmartPlaylistTracks(id)
			}
		}
		// Reload playlists to update counts
		await playlistsStore.load()
	}}
	onRelocateComplete={handleRelocateComplete}
	{devices}
	onExport={exportController.handleExportSubmit}
	onQuickExport={exportController.handleQuickExportSubmit}
	onExportFailureKeep={exportController.handleExportFailureKeep}
	onExportFailureCleanup={exportController.handleExportFailureCleanup}
	onReformatDevice={deviceController.handleReformatDevice}
/>

<!-- Add Release Modal -->
{#if showAddReleaseModal}
	<AddReleaseModal
		open={true}
		onClose={() => (showAddReleaseModal = false)}
		onSubmit={handleAddRelease}
		onAddToExisting={async (releaseId, tracks) => {
			const release = await discoveryApi.addTracksToRelease(releaseId, tracks)
			if (release) {
				discoveryStore.updateRelease(releaseId, {})
				showAddReleaseModal = false
				await discoveryStore.loadReleases()
			}
		}}
	/>
{/if}

<!-- Merge Releases Modal -->
{#if mergeReleases && mergeReleases.length >= 2}
	<MergeReleasesModal
		open={true}
		releases={mergeReleases}
		onClose={() => (mergeReleases = null)}
		onMerge={async (targetId, sourceIds) => {
			const merged = await discoveryStore.mergeReleases(targetId, sourceIds)
			if (merged) {
				uiStore.setSelectedReleases(new Set([merged.id]))
				mergeReleases = null
			}
		}}
	/>
{/if}

<!-- Purchase Release Modal -->
{#if purchaseRelease}
	<PurchaseReleaseModal
		open={true}
		release={purchaseRelease}
		onClose={() => (purchaseRelease = null)}
		onComplete={handlePurchaseComplete}
	/>
{/if}

<!-- Update Modal -->
{#if $updateAvailable}
	<UpdateModal open={true} onClose={() => updaterStore.dismiss()} />
{/if}

<!-- Drag Preview -->
{#if $isDragging && $dragPosition}
	<DragPreview
		data={$dragData}
		tracks={$libraryStore.tracks}
		releases={allAvailableReleases}
		{playlists}
		{tagCategories}
		x={$dragPosition.x}
		y={$dragPosition.y}
	/>
{/if}
