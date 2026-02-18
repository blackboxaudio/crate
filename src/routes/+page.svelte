<script lang="ts">
	import { onMount, onDestroy } from 'svelte'
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
	} from '$lib/stores'
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
	import { useAppInitialization, useKeyboardShortcuts, useMenuActions, useDragDropCoordination } from '$lib/hooks'
	import { translate } from '$lib/i18n'

	import { Sidebar, Toolbar, RightSidebar } from '$lib/components/layout'
	import { LibraryView } from '$lib/components/library'
	import { DiscoveryView, AddReleaseModal, DiscoveryEditor, PurchaseReleaseModal } from '$lib/components/discovery'
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
		SplashScreen,
	} from '$lib/components/common'
	import { PlaylistView, FolderView } from '$lib/components/playlists'
	import { openDevTools, closeDevTools, setMenuItemEnabled } from '$lib/api/app'
	import { exportStore } from '$lib/stores/export'
	import { SvelteMap } from 'svelte/reactivity'

	// =============================================================================
	// State
	// =============================================================================

	// Splash screen state
	let showSplash = $state(true)
	let splashVersion = $state('0.0.0')

	// Local state
	let sortConfig = $state<SortConfig>({ field: 'date_added', direction: 'desc' })
	let discoverySortConfig = $state<DiscoverySortConfig>({ field: 'date_added', direction: 'desc' })
	let showAddReleaseModal = $state(false)
	let purchaseRelease = $state<DiscoveryRelease | null>(null)
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
		},
		{
			openCreatePlaylistModal: (parentId) => modalOrchestrator.openCreatePlaylistModal(parentId),
			openCreateFolderModal: (parentId) => modalOrchestrator.openCreateFolderModal(parentId),
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

	// Load discovery playlist releases when a discovery playlist is selected
	let prevDiscoveryPlaylistId = $state<string | null>(null)
	$effect(() => {
		const playlistId = selectedPlaylistId
		const view = $activeView
		if (view === 'discovery' && playlistId && playlistId !== prevDiscoveryPlaylistId) {
			const playlist = playlists.find((p) => p.id === playlistId)
			if (playlist && playlist.context === 'discovery' && !playlist.is_folder) {
				prevDiscoveryPlaylistId = playlistId
				playlistsStore.getPlaylistReleases(playlistId).then((releases) => {
					discoveryPlaylistReleases = releases
				})
			}
		} else if (!playlistId) {
			prevDiscoveryPlaylistId = null
			discoveryPlaylistReleases = []
		}
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
			onTagDropOnCategory: async (tagId: string, _sourceCategoryId: string, targetCategoryId: string) => {
				try {
					await tagsStore.moveTag(tagId, targetCategoryId)
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

		// Get version after stores load
		splashVersion = get(appStore).info?.version ?? '0.0.0'

		// Set up keyboard shortcuts
		const cleanupKeyboard = useKeyboardShortcuts({
			onPlayPause: () => playerStore.togglePlayPause(),
			onFocusSearch: () => {
				const searchInput = document.querySelector('input[type="search"]') as HTMLInputElement
				searchInput?.focus()
			},
			onClearSelection: () => uiStore.clearSelection(),
			onSelectAll: () => {
				const allIds = new Set($sortedTracks.map((t) => t.id))
				uiStore.setSelectedTracks(allIds)
			},
			onOpenSettings: (tab?: SettingsPage) => modalOrchestrator.openSettingsModal(tab),
			onNewPlaylist: () => playlistController.handleCreatePlaylist(),
			onNewFolder: () => playlistController.handleCreateFolder(),
			onImport: () => trackController.handleImport(),
			onDeleteSelected: () => {
				const ids = [...$selectedTrackIds]
				if (ids.length > 0) {
					if (selectedPlaylistId) {
						modalOrchestrator.openRemoveFromPlaylistModal(ids, selectedPlaylistId)
					} else {
						modalOrchestrator.openRemoveFromLibraryModal(ids)
					}
				} else if (selectedPlaylistId) {
					const playlist = playlists.find((p) => p.id === selectedPlaylistId)
					if (playlist) playlistController.handlePlaylistDelete(playlist)
				} else if (selectedFolderId) {
					const folder = playlists.find((p) => p.id === selectedFolderId)
					if (folder) playlistController.handlePlaylistDelete(folder)
				}
			},
			onPlaySelected: () => {
				const selectedIds = $selectedTrackIds
				if (selectedIds.size > 0) {
					const firstSelectedId = [...selectedIds][0]
					const track = $displayedTracks.find((t) => t.id === firstSelectedId)
					if (track) trackController.play(track)
				}
			},
			onSeekBackward: () => playerStore.seekRelative(-5000),
			onSeekForward: () => playerStore.seekRelative(5000),
			onPreviousTrack: () => {
				const currentTrackId = $currentTrack?.id
				if (!currentTrackId) return
				const tracks = $displayedTracks
				const currentIndex = tracks.findIndex((t) => t.id === currentTrackId)
				if (currentIndex > 0) trackController.play(tracks[currentIndex - 1])
			},
			onNextTrack: () => {
				const currentTrackId = $currentTrack?.id
				if (!currentTrackId) return
				const tracks = $displayedTracks
				const currentIndex = tracks.findIndex((t) => t.id === currentTrackId)
				if (currentIndex >= 0 && currentIndex < tracks.length - 1) trackController.play(tracks[currentIndex + 1])
			},
			onVolumeUp: () => playerStore.adjustVolume(0.1),
			onVolumeDown: () => playerStore.adjustVolume(-0.1),
			onToggleMute: () => playerStore.toggleMute(),
			onSelectPreviousTrack: () => {
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
		})

		// Set up menu action listener
		const cleanupMenu = await useMenuActions({
			onImport: trackController.handleImport,
			onCreatePlaylist: playlistController.handleCreatePlaylist,
			onCreateFolder: playlistController.handleCreateFolder,
			onSelectAll: () => {
				const allIds = new Set($sortedTracks.map((t) => t.id))
				uiStore.setSelectedTracks(allIds)
			},
			onPlayPause: () => playerStore.togglePlayPause(),
			onStop: () => playerStore.stop(),
			onOpenSettings: (tab?: SettingsPage) => modalOrchestrator.openSettingsModal(tab),
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

		// Dismiss splash screen
		showSplash = false

		return () => {
			cleanupApp()
			cleanupKeyboard()
			cleanupMenu()
			exportStore.stopListening()
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
		// Clear playlist/folder selection when switching views
		uiStore.selectPlaylist(null)
		uiStore.selectFolder(null)
		discoveryPlaylistReleases = []

		uiStore.setActiveView(view)
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

	function handleDiscoverySortChange(config: DiscoverySortConfig) {
		discoverySortConfig = config
		discoveryStore.setSort(config)
	}

	function handleReleaseOpen(release: { url: string }) {
		openUrl(release.url)
	}

	function handleReleaseSelectionChange(ids: Set<string>) {
		uiStore.setSelectedReleases(ids)
	}

	async function handleAddRelease(create: DiscoveryReleaseCreate) {
		const release = await discoveryStore.createRelease(create)
		if (release) {
			showAddReleaseModal = false
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

	async function handleDiscoveryReleaseDelete(releases: DiscoveryRelease[]) {
		const ids = releases.map((r) => r.id)
		await discoveryStore.deleteReleases(ids)
		uiStore.clearReleaseSelection()
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

<!-- Splash Screen -->
<SplashScreen show={showSplash} version={splashVersion} />

<!-- Main App Content -->
{#if !showSplash}
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
					{tagStates}
					{tagCounts}
					trackCount={$activeView === 'discovery' ? $releaseCount : $trackCount}
					showHeader={false}
					onLibraryClick={playlistController.handleLibraryClick}
					onPlaylistSelect={playlistController.handlePlaylistSelect}
					onPlaylistContextMenu={handlePlaylistContextMenu}
					onPlaylistTreeContextMenu={(e) => contextMenuOrchestrator.openPlaylistTreeMenu(e)}
					onDeviceContextMenu={handleDeviceContextMenu}
					onCancelExport={exportController.handleExportCancel}
					onTagSelect={tagController.selectTag}
					onTagToggle={tagController.toggleTagOnTracks}
					onTagContextMenu={handleTagContextMenu}
					onCategoryContextMenu={handleCategoryContextMenu}
					onCreatePlaylist={playlistController.handleCreatePlaylist}
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
				<div class="flex-1 overflow-hidden">
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
									onSelectionChange={handleReleaseSelectionChange}
									onContextMenu={(e, item) => {
										handleReleaseContextMenu(e, item as unknown as DiscoveryRelease)
									}}
									onBreadcrumbNavigate={handleBreadcrumbNavigate}
									onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
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
									onSelectionChange={trackController.handleSelectionChange}
									onTrackPlay={trackController.play}
									onSortChange={handleSortChange}
									onContextMenu={handleTrackContextMenu}
									onEmptySpaceContextMenu={(e, pl) => contextMenuOrchestrator.openPlaylistViewMenu(e, pl)}
									onBreadcrumbNavigate={handleBreadcrumbNavigate}
									onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
									onTrackColorChange={trackController.setColor}
									onCancelAnalysis={handleCancelAnalysis}
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
							onSelectionChange={handleReleaseSelectionChange}
							onReleaseOpen={handleReleaseOpen}
							onReleaseImport={handleDiscoveryReleaseImport}
							onSortChange={handleDiscoverySortChange}
							onContextMenu={handleReleaseContextMenu}
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
							onSelectionChange={trackController.handleSelectionChange}
							onTrackPlay={trackController.play}
							onSortChange={handleSortChange}
							onContextMenu={handleTrackContextMenu}
							onEmptySpaceContextMenu={(e) => contextMenuOrchestrator.openLibraryViewMenu(e)}
							onTrackColorChange={trackController.setColor}
							onCancelAnalysis={handleCancelAnalysis}
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
						<DiscoveryEditor selectedReleases={selectedReleasesArray} onImport={handleDiscoveryReleaseImport} />
					{:else}
						<TrackEditor selectedTracks={selectedTracksArray} />
					{/if}
				</RightSidebar>
			</div>
		</div>

		<Player />
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
	onPlaylistRename={playlistController.handlePlaylistRename}
	onPlaylistDelete={playlistController.handlePlaylistDelete}
	onPlaylistMove={playlistController.handlePlaylistMove}
	onFolderViewCreatePlaylist={(folderId) => modalOrchestrator.openCreatePlaylistModal(folderId)}
	onFolderViewCreateFolder={(folderId) => modalOrchestrator.openCreateFolderModal(folderId)}
	onPlaylistTreeCreatePlaylist={() => modalOrchestrator.openCreatePlaylistModal(null)}
	onPlaylistTreeCreateFolder={() => modalOrchestrator.openCreateFolderModal(null)}
	onLibraryViewImport={trackController.handleImport}
	onPlaylistViewImport={playlistController.handlePlaylistViewImport}
	{tagCategories}
	onTagAddTag={(categoryId) => modalOrchestrator.openCreateTagModal(categoryId)}
	onTagRename={(tag) => modalOrchestrator.openRenameTagModal(tag)}
	onTagDelete={(tag) => modalOrchestrator.openDeleteTagModal(tag)}
	onTagMove={async (tag, targetCategoryId) => {
		try {
			await tagsStore.moveTag(tag.id, targetCategoryId)
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
	onDiscoveryReleaseDelete={handleDiscoveryReleaseDelete}
	onDiscoveryReleaseAddToPlaylist={async (playlistId, releases) => {
		const releaseIds = releases.map((r) => r.id)
		await playlistsStore.addReleases(playlistId, releaseIds)
	}}
	onClose={() => (contextMenuPlaylistId = null)}
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
	onDeleteTag={async (id) => {
		await tagsStore.deleteTag(id)
		await libraryStore.loadTracks()
	}}
	onDeleteCategory={async (id) => {
		await tagsStore.deleteCategory(id)
		await libraryStore.loadTracks()
	}}
	onRemoveFromPlaylist={async (trackIds, playlistId) => {
		await playlistsStore.removeTracks(playlistId, trackIds)
		await libraryStore.loadPlaylistTracks(playlistId)
		uiStore.clearSelection()
		const count = trackIds.length
		toastStore.success(count === 1 ? '1 track removed from playlist' : `${count} tracks removed from playlist`)
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
	<AddReleaseModal open={true} onClose={() => (showAddReleaseModal = false)} onSubmit={handleAddRelease} />
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
