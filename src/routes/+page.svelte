<script lang="ts">
	import { onMount, onDestroy } from 'svelte'
	import { fade } from 'svelte/transition'
	import { get } from 'svelte/store'
	import { open } from '@tauri-apps/plugin-dialog'
	import { openPath } from '@tauri-apps/plugin-opener'

	import type {
		Track,
		TrackFilter,
		SortConfig,
		Playlist,
		TagCategory,
		Tag,
		TagSelectionState,
		UsbDevice,
		BreadcrumbItem,
		ExportRequest,
		SettingsPage,
	} from '$lib/types'
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
		selectedTrackIds,
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
		dragStore,
		isDragging,
		dragData,
		dragPosition,
		needsDropTargetRefresh,
		devToolsOpen,
		isDev,
		analysisStore,
	} from '$lib/stores'
	import { syncStore } from '$lib/stores/sync'
	import { findDropTargets, findDropTargetAtPoint, type DropTarget } from '$lib/utils/drag'
	import { toastStore } from '$lib/stores/toast'
	import { buildBreadcrumbItems, getPlaylistChildren } from '$lib/stores/playlists'
	import { findConflictingItem, getPlaylistById, hasChildren } from '$lib/utils'
	import { createTagController, createTrackController } from '$lib/controllers'
	import { useAppInitialization, useKeyboardShortcuts, useMenuActions } from '$lib/hooks'
	import { translate } from '$lib/i18n'

	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { LibraryView } from '$lib/components/library'
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
	import { TrackEditor } from '$lib/components/editor'
	import * as devicesApi from '$lib/api/devices'
	import * as exportApi from '$lib/api/export'
	import { openDevTools, closeDevTools } from '$lib/api/app'
	import { exportStore, isExporting } from '$lib/stores/export'

	// Splash screen state
	let showSplash = $state(true)
	let splashVersion = $state('0.0.0')

	// Local state
	let sortConfig = $state<SortConfig>({ field: 'date_added', direction: 'desc' })
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

	// Right sidebar resize state
	let isResizingRightSidebar = $state(false)

	// Internal drag state
	let dropTargets = $state<DropTarget[]>([])
	let rafId: number | null = null
	let cleanupOnMount: (() => void) | undefined

	// Handle global pointer events when dragging
	function handleGlobalPointerMove(e: PointerEvent) {
		if (!$isDragging) return

		// Use requestAnimationFrame to throttle updates
		if (rafId !== null) return

		rafId = requestAnimationFrame(() => {
			rafId = null
			dragStore.updatePosition(e.clientX, e.clientY)

			// Refresh drop targets if requested (e.g., after folder expand)
			if ($needsDropTargetRefresh) {
				dropTargets = findDropTargets()
				dragStore.clearDropTargetRefresh()
			}

			// Hit-test to find drop target under pointer
			const target = findDropTargetAtPoint(e.clientX, e.clientY, dropTargets)
			const targetId = target ? `${target.type}-${target.id}` : null
			dragStore.setHoveredDropTarget(targetId)
		})
	}

	// Check if targetId is a descendant of potentialAncestorId (prevents circular drops)
	function isDescendantOf(potentialAncestorId: string, targetId: string): boolean {
		let currentId: string | null = targetId
		while (currentId) {
			if (currentId === potentialAncestorId) return true
			const current = playlists.find((p) => p.id === currentId)
			currentId = current?.parent_id ?? null
		}
		return false
	}

	// Get all playlist IDs to export from a playlist/folder (single or recursive)
	function getExportPlaylistIds(playlistId: string, isFolder: boolean): string[] {
		if (!isFolder) {
			return [playlistId]
		}

		// For folders, recursively collect all non-folder playlist IDs
		const playlistIds: string[] = []
		function collectDescendants(parentId: string) {
			const children = playlists.filter((p) => p.parent_id === parentId)
			for (const child of children) {
				if (!child.is_folder) {
					playlistIds.push(child.id)
				} else {
					collectDescendants(child.id)
				}
			}
		}
		collectDescendants(playlistId)
		return playlistIds
	}

	// Handle dropping a playlist/folder onto a device
	async function handlePlaylistDropOnDevice(playlistId: string, isFolder: boolean, deviceId: string) {
		// Check if already exporting
		if ($isExporting) {
			toastStore.error(get(translate)('errors.exportInProgress'))
			return
		}

		// Find the device
		const device = devices.find((d) => d.id === deviceId)
		if (!device) {
			toastStore.error('Device not found')
			return
		}

		// Get playlist IDs to export
		const playlistIds = getExportPlaylistIds(playlistId, isFolder)

		// Check if folder has no playlists
		if (playlistIds.length === 0) {
			toastStore.error(get(translate)('export.noPlaylistsInFolder'))
			return
		}

		// Build export request
		const request: ExportRequest = {
			device_id: device.id,
			mount_point: device.mount_point,
			device_name: device.name,
			playlist_ids: playlistIds,
			enable_sync: true,
		}

		// Start export immediately
		await handleExportSubmit(request)
	}

	function handleGlobalPointerUp(e: PointerEvent) {
		if (!$isDragging) return

		const data = $dragData
		if (!data) {
			dragStore.endDrag()
			return
		}

		// Find drop target under pointer
		const target = findDropTargetAtPoint(e.clientX, e.clientY, dropTargets)

		if (target) {
			// Handle the drop based on what we're dragging and where
			if (data.type === 'tracks' && target.type === 'playlist') {
				// Dropping tracks on a playlist
				trackController.handleTracksDropOnPlaylist(target.id, data.trackIds)
			} else if (data.type === 'playlist' && target.type === 'folder') {
				// Validate: prevent dropping on self
				if (data.playlistId === target.id) {
					toastStore.error('Cannot drop a folder into itself')
					dragStore.endDrag()
					return
				}

				// Validate: prevent dropping folder into its own descendants
				if (data.isFolder && isDescendantOf(data.playlistId, target.id)) {
					toastStore.error('Cannot drop a folder into its own subfolder')
					dragStore.endDrag()
					return
				}

				// Dropping a playlist/folder on a folder
				handlePlaylistDragMove(data.playlistId, target.id)
			} else if (data.type === 'playlist' && target.type === 'device') {
				// Dropping a playlist/folder on a device - export immediately
				handlePlaylistDropOnDevice(data.playlistId, data.isFolder, target.id)
			}
		}

		dragStore.endDrag()
	}

	// Set up and tear down global drag listeners
	$effect(() => {
		if ($isDragging) {
			// Cache drop targets when drag starts
			dropTargets = findDropTargets()

			// Set grabbing cursor globally by adding class to html element
			document.documentElement.classList.add('is-dragging')

			// Add global listeners
			document.addEventListener('pointermove', handleGlobalPointerMove)
			document.addEventListener('pointerup', handleGlobalPointerUp)

			return () => {
				document.documentElement.classList.remove('is-dragging')
				document.removeEventListener('pointermove', handleGlobalPointerMove)
				document.removeEventListener('pointerup', handleGlobalPointerUp)
				if (rafId !== null) {
					cancelAnimationFrame(rafId)
					rafId = null
				}
			}
		}
	})

	// Orchestrator bindings
	let contextMenuOrchestrator: ReturnType<typeof ContextMenuOrchestrator>
	let modalOrchestrator: ReturnType<typeof ModalOrchestrator>

	// Context menu state for playlist tree hover styling
	let contextMenuPlaylistId = $state<string | null>(null)

	// Tag controller
	const tagController = createTagController({
		tagsStore,
		libraryStore,
		uiStore,
		getSelectedTagIds: () => selectedTagIds,
		getSelectedPlaylistId: () => selectedPlaylistId,
		getTagFilterMode: () => $tagFilterMode,
		getSelectedTrackIds: () => $selectedTrackIds,
		getRecentlyToggledMixedTags: () => $recentlyToggledMixedTags,
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

	// Subscribe to stores
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

	// Compute tag states when selection or tracks change
	$effect(() => {
		const result = computeTagStates(tagCategories, $displayedTracks, $selectedTrackIds)
		tagStates = result.states
		tagCounts = result.counts
	})

	// Clear recently toggled tags when selection changes
	let previousSelectedIds = $state<Set<string>>(new Set())
	$effect(() => {
		const currentIds = $selectedTrackIds
		// Only clear if selection actually changed
		if (currentIds.size !== previousSelectedIds.size || ![...currentIds].every((id) => previousSelectedIds.has(id))) {
			uiStore.clearAllRecentlyToggledTags()
			previousSelectedIds = new Set(currentIds)
		}
	})

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
			onToggleInspector: () => uiStore.toggleRightSidebar(),
			// New shortcuts
			onNewPlaylist: () => modalOrchestrator.openCreatePlaylistModal(selectedFolderId),
			onNewFolder: () => modalOrchestrator.openCreateFolderModal(selectedFolderId),
			onImport: () => trackController.handleImport(),
			onDeleteSelected: () => {
				const ids = [...$selectedTrackIds]
				if (ids.length > 0) {
					// Track deletion takes precedence
					if (selectedPlaylistId) {
						modalOrchestrator.openRemoveFromPlaylistModal(ids, selectedPlaylistId)
					} else {
						modalOrchestrator.openRemoveFromLibraryModal(ids)
					}
				} else if (selectedPlaylistId) {
					// Delete the selected playlist (no tracks selected)
					const playlist = playlists.find((p) => p.id === selectedPlaylistId)
					if (playlist) {
						handlePlaylistDelete(playlist)
					}
				} else if (selectedFolderId) {
					// Delete the selected folder (no tracks selected)
					const folder = playlists.find((p) => p.id === selectedFolderId)
					if (folder) {
						handlePlaylistDelete(folder)
					}
				}
			},
			onPlaySelected: () => {
				// Play the first selected track
				const selectedIds = $selectedTrackIds
				if (selectedIds.size > 0) {
					const firstSelectedId = [...selectedIds][0]
					const track = $displayedTracks.find((t) => t.id === firstSelectedId)
					if (track) {
						trackController.play(track)
					}
				}
			},
			onSeekBackward: () => playerStore.seekRelative(-5000),
			onSeekForward: () => playerStore.seekRelative(5000),
			onPreviousTrack: () => {
				// Play the previous track in the list
				const currentTrackId = $currentTrack?.id
				if (!currentTrackId) return
				const tracks = $displayedTracks
				const currentIndex = tracks.findIndex((t) => t.id === currentTrackId)
				if (currentIndex > 0) {
					trackController.play(tracks[currentIndex - 1])
				}
			},
			onNextTrack: () => {
				// Play the next track in the list
				const currentTrackId = $currentTrack?.id
				if (!currentTrackId) return
				const tracks = $displayedTracks
				const currentIndex = tracks.findIndex((t) => t.id === currentTrackId)
				if (currentIndex >= 0 && currentIndex < tracks.length - 1) {
					trackController.play(tracks[currentIndex + 1])
				}
			},
			onVolumeUp: () => playerStore.adjustVolume(0.1),
			onVolumeDown: () => playerStore.adjustVolume(-0.1),
			onToggleMute: () => playerStore.toggleMute(),
			onSelectPreviousTrack: () => {
				// Select the previous track in the list
				const tracks = $displayedTracks
				if (tracks.length === 0) return
				const selectedIds = $selectedTrackIds
				if (selectedIds.size === 0) {
					// Nothing selected, select the last track
					uiStore.selectTrack(tracks[tracks.length - 1].id)
				} else {
					// Find the first selected track and move selection up
					const firstSelectedId = [...selectedIds][0]
					const currentIndex = tracks.findIndex((t) => t.id === firstSelectedId)
					if (currentIndex > 0) {
						uiStore.selectTrack(tracks[currentIndex - 1].id)
					}
				}
			},
			onSelectNextTrack: () => {
				// Select the next track in the list
				const tracks = $displayedTracks
				if (tracks.length === 0) return
				const selectedIds = $selectedTrackIds
				if (selectedIds.size === 0) {
					// Nothing selected, select the first track
					uiStore.selectTrack(tracks[0].id)
				} else {
					// Find the last selected track and move selection down
					const lastSelectedId = [...selectedIds].pop()
					const currentIndex = tracks.findIndex((t) => t.id === lastSelectedId)
					if (currentIndex >= 0 && currentIndex < tracks.length - 1) {
						uiStore.selectTrack(tracks[currentIndex + 1].id)
					}
				}
			},
			onQuickExport: () => {
				// Only open if there are devices available
				if (devices.length > 0 && modalOrchestrator) {
					modalOrchestrator.openQuickExportModal()
				}
			},
			onJumpToPlayingTrack: () => {
				const track = $currentTrack
				if (!track) return

				// If viewing a playlist, switch to library view
				if (selectedPlaylistId) {
					handleLibraryClick()
				}

				// Select and scroll to track
				uiStore.selectTrack(track.id)
				// TODO: Implement scrollToTrack in LibraryView
			},
		})

		// Set up menu action listener
		const cleanupMenu = await useMenuActions({
			onImport: trackController.handleImport,
			onCreatePlaylist: handleCreatePlaylist,
			onCreateFolder: handleCreateFolder,
			onSelectAll: () => {
				const allIds = new Set($sortedTracks.map((t) => t.id))
				uiStore.setSelectedTracks(allIds)
			},
			onPlayPause: () => playerStore.togglePlayPause(),
			onStop: () => playerStore.stop(),
			onOpenSettings: (tab?: SettingsPage) => modalOrchestrator.openSettingsModal(tab),
			onQuickExport: () => {
				if (devices.length > 0 && modalOrchestrator) {
					modalOrchestrator.openQuickExportModal()
				}
			},
			onJumpToPlayingTrack: () => {
				const track = $currentTrack
				if (!track) return
				if (selectedPlaylistId) {
					handleLibraryClick()
				}
				uiStore.selectTrack(track.id)
			},
		})

		// Ensure minimum display time for splash screen
		const elapsed = Date.now() - splashStartTime
		if (elapsed < minDisplayTime) {
			await new Promise((r) => setTimeout(r, minDisplayTime - elapsed))
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

	// Initialize on mount
	onMount(() => {
		onMountHelper().then((cleanupFn) => {
			cleanupOnMount = cleanupFn
		})
	})

	onDestroy(() => {
		cleanupOnMount?.()
	})

	// Sort change
	function handleSortChange(config: SortConfig) {
		sortConfig = config
		libraryStore.setSort(config)
	}

	// Sidebar handlers
	async function handleLibraryClick() {
		uiStore.selectPlaylist(null)
		uiStore.selectFolder(null)
		libraryStore.clearPlaylistTracks()
		// Reload library with current tag filters (if any)
		if (selectedTagIds.length > 0) {
			await libraryStore.loadTracks({ tag_ids: selectedTagIds, tag_filter_mode: $tagFilterMode })
		} else {
			libraryStore.clearFilters()
			await libraryStore.loadTracks()
		}
	}

	function handleSidebarResize(delta: number) {
		uiStore.setSidebarWidth(sidebarWidth + delta)
	}

	function handleRightSidebarResize(delta: number) {
		uiStore.setRightSidebarWidth($rightSidebarWidth - delta)
	}

	// Selected tracks for the editor
	let selectedTracksArray = $derived($displayedTracks.filter((t) => $selectedTrackIds.has(t.id)))

	// Snapshot of selected tracks that persists during close transition
	let editorTracks = $state<Track[]>([])

	async function handlePlaylistSelect(playlist: Playlist) {
		// Clear track selection when selecting a folder or playlist
		uiStore.clearSelection()

		if (playlist.is_folder) {
			uiStore.selectFolder(playlist.id)
		} else {
			uiStore.selectPlaylist(playlist.id)
			// Apply existing tag filters to the playlist (if any)
			if (selectedTagIds.length > 0) {
				const filter: TrackFilter = {
					playlist_id: playlist.id,
					tag_ids: selectedTagIds,
					tag_filter_mode: $tagFilterMode,
				}
				await libraryStore.loadTracks(filter)
			} else {
				await libraryStore.loadPlaylistTracks(playlist.id)
			}
		}
	}

	function handleCreatePlaylist() {
		modalOrchestrator.openCreatePlaylistModal(selectedFolderId)
	}

	function handleCreateFolder() {
		modalOrchestrator.openCreateFolderModal(selectedFolderId)
	}

	function handleCreateCategory() {
		modalOrchestrator.openCreateCategoryModal()
	}

	function handleCreateTag(categoryId: string) {
		modalOrchestrator.openCreateTagModal(categoryId)
	}

	// Context menu handlers
	function handleTrackContextMenu(e: MouseEvent, track: Track) {
		// If the clicked track is in the selection, use the selection
		// Otherwise, use just the clicked track
		const currentSelection = $selectedTrackIds
		let tracks: Track[]
		if (currentSelection.has(track.id)) {
			tracks = $sortedTracks.filter((t) => currentSelection.has(t.id))
		} else {
			tracks = [track]
		}
		contextMenuOrchestrator.openTrackMenu(e, tracks)
	}

	// Playlist context menu handlers
	function handlePlaylistContextMenu(e: MouseEvent, playlist: Playlist) {
		contextMenuPlaylistId = playlist.id
		contextMenuOrchestrator.openPlaylistMenu(e, playlist, 'tree')
	}

	function handleFolderViewCardContextMenu(e: MouseEvent, playlist: Playlist) {
		contextMenuOrchestrator.openPlaylistMenu(e, playlist, 'folder')
	}

	// Playlist tree context menu handlers (right-click on whitespace)
	function handlePlaylistTreeContextMenu(e: MouseEvent) {
		contextMenuOrchestrator.openPlaylistTreeMenu(e)
	}

	function handlePlaylistTreeCreatePlaylist() {
		modalOrchestrator.openCreatePlaylistModal(null)
	}

	function handlePlaylistTreeCreateFolder() {
		modalOrchestrator.openCreateFolderModal(null)
	}

	// Tags sidebar context menu handlers (right-click on whitespace)
	function handleTagsWhitespaceContextMenu(e: MouseEvent) {
		contextMenuOrchestrator.openTagsSidebarMenu(e)
	}

	function handleTagsSidebarAddCategory() {
		modalOrchestrator.openCreateCategoryModal()
	}

	function handleTagsSidebarAddTag() {
		modalOrchestrator.openTagInputModal()
	}

	// Folder view context menu handlers (right-click on empty space)
	function handleFolderViewContextMenu(e: MouseEvent, folderId: string) {
		contextMenuOrchestrator.openFolderViewMenu(e, folderId)
	}

	function handleFolderViewCreatePlaylist(folderId: string | null) {
		modalOrchestrator.openCreatePlaylistModal(folderId)
	}

	function handleFolderViewCreateFolder(folderId: string | null) {
		modalOrchestrator.openCreateFolderModal(folderId)
	}

	// Library view context menu handlers (right-click on empty space)
	function handleLibraryViewContextMenu(e: MouseEvent) {
		contextMenuOrchestrator.openLibraryViewMenu(e)
	}

	// Playlist view context menu handlers (right-click on empty space)
	function handlePlaylistViewContextMenu(e: MouseEvent, playlist: Playlist) {
		contextMenuOrchestrator.openPlaylistViewMenu(e, playlist)
	}

	async function handlePlaylistViewImport(playlist: Playlist) {
		// Open file dialog
		const selected = await open({
			multiple: true,
			filters: [
				{
					name: 'Audio Files',
					extensions: ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac'],
				},
			],
		})

		if (selected && Array.isArray(selected)) {
			// Import tracks to collection
			const result = await libraryStore.importTracks(selected)

			// Add imported tracks to the playlist
			if (result.tracks.length > 0) {
				const trackIds = result.tracks.map((t) => t.id)
				await playlistsStore.addTracks(playlist.id, trackIds)

				// Reload playlist tracks to show the new additions
				await libraryStore.loadPlaylistTracks(playlist.id)

				// Show toast notification
				const count = trackIds.length
				toastStore.success(
					count === 1 ? `1 track added to ${playlist.name}` : `${count} tracks added to ${playlist.name}`
				)
			}
		}
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

	function handlePlaylistRename(playlist: Playlist) {
		modalOrchestrator.openRenamePlaylistModal(playlist)
	}

	function handlePlaylistDelete(playlist: Playlist) {
		modalOrchestrator.openDeletePlaylistModal(playlist, hasChildren(playlists, playlist.id))
	}

	async function handlePlaylistMove(playlist: Playlist, folderId: string | null) {
		// Check for conflict
		const conflict = findConflictingItem(playlists, playlist, folderId)

		if (conflict) {
			modalOrchestrator.openMoveConflictModal(playlist, conflict, folderId)
			return
		}

		// No conflict, proceed with move
		await playlistsStore.move(playlist.id, folderId)
	}

	// Handler for drag-drop playlist move
	async function handlePlaylistDragMove(playlistId: string, targetFolderId: string | null) {
		const playlist = getPlaylistById(playlists, playlistId)
		if (!playlist) return

		// Check for conflict
		const conflict = findConflictingItem(playlists, playlist, targetFolderId)

		if (conflict) {
			modalOrchestrator.openMoveConflictModal(playlist, conflict, targetFolderId)
			return
		}

		// No conflict, proceed with move
		const result = await playlistsStore.move(playlistId, targetFolderId)
		if (result) {
			toastStore.success('Moved successfully')
		}
	}

	// Helper to get folders for move menu
	const playlistFolders = $derived(playlists.filter((p) => p.is_folder))

	// Category colors map for track list
	const categoryColors = $derived(new Map(tagCategories.map((c) => [c.id, c.color])))

	// Category sort orders map for tag sorting in track rows
	const categorySortOrders = $derived(new Map(tagCategories.map((c) => [c.id, c.sort_order])))

	// Active filter tags for toolbar display
	const activeFilterTags = $derived.by(() => {
		if (selectedTagIds.length === 0) return []
		const tags: Tag[] = []
		for (const category of tagCategories) {
			for (const tag of category.tags) {
				if (selectedTagIds.includes(tag.id)) {
					tags.push(tag)
				}
			}
		}
		return tags
	})

	// Tag colors map for toolbar (maps tag.category_id to category.color)
	const tagColors = $derived(new Map(tagCategories.map((c) => [c.id, c.color])))

	// Breadcrumb items for navigation
	const currentFolderChildCount = $derived(
		selectedFolderId ? getPlaylistChildren(playlists, selectedFolderId).length : 0
	)

	const breadcrumbItems = $derived(
		buildBreadcrumbItems(
			playlists,
			selectedFolderId,
			selectedPlaylistId,
			selectedPlaylistId ? $displayedTracks.length : undefined,
			currentFolderChildCount
		)
	)

	const sidebarOpen = $derived($rightSidebarVisible && selectedTracksArray.length > 0)

	// Sync editor tracks with selection when sidebar is open
	$effect(() => {
		if (sidebarOpen && selectedTracksArray.length > 0) {
			editorTracks = selectedTracksArray
		}
	})

	// Breadcrumb navigation handler
	function handleBreadcrumbNavigate(item: BreadcrumbItem) {
		if (item.id === null) {
			// Navigate to Library root
			handleLibraryClick()
		} else if (item.playlist) {
			handlePlaylistSelect(item.playlist)
		}
	}

	// Breadcrumb context menu handler
	function handleBreadcrumbContextMenu(e: MouseEvent, item: BreadcrumbItem) {
		if (item.type === 'library') {
			// Library context menu - placeholder for now
			// TODO: Could show "New Playlist", "New Folder", "Import Tracks"
			return
		}

		// Reuse playlist context menu for folders/playlists
		if (item.playlist) {
			contextMenuOrchestrator.openPlaylistMenu(e, item.playlist, 'tree')
		}
	}

	// Tag context menu handlers
	function handleTagContextMenu(e: MouseEvent, tag: Tag, category: TagCategory) {
		contextMenuOrchestrator.openTagMenu(e, { type: 'tag', tag, category })
	}

	function handleCategoryContextMenu(e: MouseEvent, category: TagCategory) {
		contextMenuOrchestrator.openTagMenu(e, { type: 'category', category })
	}

	// Device context menu handlers
	function handleDeviceContextMenu(e: MouseEvent, device: UsbDevice) {
		contextMenuOrchestrator.openDeviceMenu(e, device)
	}

	async function handleEjectDevice(device: UsbDevice) {
		try {
			await devicesApi.ejectDevice(device.mount_point)
			toastStore.success(`${device.name} ejected`)
		} catch (error) {
			toastStore.error(`Failed to eject ${device.name}`)
			console.error('Eject error:', error)
		}
	}

	function handleViewDeviceInfo(device: UsbDevice) {
		modalOrchestrator.openDeviceInfoModal(device)
	}

	async function handleDeviceRevealInFinder(device: UsbDevice) {
		await openPath(device.mount_point)
	}

	function handleDeviceReformat(device: UsbDevice) {
		modalOrchestrator.openReformatDeviceModal(device)
	}

	function handleDeviceIgnore(device: UsbDevice) {
		settingsStore.ignoreDevice(device.id)
	}

	async function handleReformatDevice(device: UsbDevice, volumeName: string) {
		devicesStore.setReformattingDevice(device.id)
		try {
			await devicesApi.reformatDevice(device.mount_point, volumeName)
			toastStore.success(`Device reformatted as "${volumeName}"`)
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error)
			// Handle user cancellation gracefully - don't show error toast or log
			if (message.includes('cancelled') || message.includes('canceled')) {
				return
			}
			toastStore.error(`Failed to reformat: ${message}`)
			console.error('Reformat error:', error)
		} finally {
			devicesStore.clearReformattingDevice()
		}
	}

	// Export handlers
	function handleDeviceExport(device: UsbDevice) {
		modalOrchestrator.openExportToDeviceModal(device)
	}

	function handlePlaylistExport(playlist: Playlist) {
		modalOrchestrator.openExportPlaylistModal(playlist)
	}

	async function handleExportSubmit(request: ExportRequest) {
		exportStore.startExport(request.device_id, request.device_name, request.playlist_ids.length)

		try {
			const result = await exportApi.exportToDevice(request)
			exportStore.completeExport(result)

			if (result.success) {
				toastStore.success(
					get(translate)('toast.tracksExported', {
						values: {
							exported: result.tracks_copied,
							skipped: result.tracks_skipped,
							deviceName: request.device_name,
						},
					})
				)
			} else {
				// Export completed but with errors
				const errorMsg = result.errors.length > 0 ? result.errors[0] : 'Unknown error'
				modalOrchestrator.openExportFailureModal(errorMsg, request.device_id, request.mount_point, result.tracks_copied)
			}
		} catch (error) {
			const errorMsg = error instanceof Error ? error.message : 'Export failed'
			exportStore.failExport(errorMsg)
			modalOrchestrator.openExportFailureModal(errorMsg, request.device_id, request.mount_point, 0)
		}
	}

	async function handleQuickExportSubmit(requests: ExportRequest[]) {
		// Export to each device sequentially
		for (const request of requests) {
			await handleExportSubmit(request)
		}
	}

	async function handleExportCancel() {
		try {
			await exportApi.cancelExport()
			exportStore.reset()
		} catch (error) {
			console.error('Failed to cancel export:', error)
		}
	}

	function handleExportFailureKeep() {
		exportStore.reset()
	}

	async function handleExportFailureCleanup(deviceId: string, mountPoint: string) {
		try {
			await exportApi.cleanupFailedExport(deviceId, mountPoint)
			toastStore.success('Cleaned up partial export')
		} catch (error) {
			toastStore.error('Failed to clean up export')
			console.error('Cleanup error:', error)
		}
		exportStore.reset()
	}

	// Relocate track complete handler
	function handleRelocateComplete(updatedTrack: Track) {
		// Update the track in the library store
		libraryStore.loadTracks()
		toastStore.success(`Relocated "${updatedTrack.title || 'track'}"`)
	}

	// Toggle dev tools open/closed
	async function handleToggleDevTools() {
		if ($devToolsOpen) {
			await closeDevTools()
		} else {
			await openDevTools()
		}
		appStore.toggleDevTools()
	}
</script>

<!-- Splash Screen -->
<SplashScreen show={showSplash} version={splashVersion} />

<!-- Main App Content -->
{#if !showSplash}
	<div class="flex h-full flex-col" in:fade={{ duration: 300, delay: 100 }}>
		<!-- Unified Header: Logo + Toolbar -->
		<div class="flex rounded-br bg-surface-1">
			<!-- Logo section (matches sidebar width) -->
			<div class="flex flex-shrink-0 items-center justify-center gap-2" style="width: {sidebarWidth}px">
				<Icon name="logo" class="h-6 w-6 text-brand-primary" fill />
				<Text variant="header-1" as="span" weight="bold">Crate</Text>
				{#if $isDev}
					<span class="rounded bg-amber-500/20 px-1.5 py-0.5 text-xs font-medium text-amber-500">DEV</span>
				{/if}
			</div>
			<!-- Toolbar content -->
			<Toolbar
				{activeFilterTags}
				{tagColors}
				tagFilterMode={$tagFilterMode}
				onRemoveTagFilter={tagController.removeTagFilter}
				onClearAllTagFilters={tagController.clearTagFilters}
				onToggleTagFilterMode={tagController.toggleTagFilterMode}
				onImport={trackController.handleImport}
				onSettings={() => modalOrchestrator.openSettingsModal()}
				onDevTools={handleToggleDevTools}
			/>
		</div>

		<div class="relative flex flex-1 overflow-hidden">
			<!-- Left: Sidebar (without header) -->
			<div class="flex-shrink-0 rounded-tr-md" style="width: {sidebarWidth}px">
				<Sidebar
					{playlists}
					{tagCategories}
					{devices}
					{selectedPlaylistId}
					{selectedFolderId}
					{contextMenuPlaylistId}
					{selectedTagIds}
					selectedTrackIds={$selectedTrackIds}
					{tagStates}
					{tagCounts}
					trackCount={$trackCount}
					showHeader={false}
					onLibraryClick={handleLibraryClick}
					onPlaylistSelect={handlePlaylistSelect}
					onPlaylistContextMenu={handlePlaylistContextMenu}
					onPlaylistTreeContextMenu={handlePlaylistTreeContextMenu}
					onDeviceContextMenu={handleDeviceContextMenu}
					onCancelExport={handleExportCancel}
					onTagSelect={tagController.selectTag}
					onTagToggle={tagController.toggleTagOnTracks}
					onTagContextMenu={handleTagContextMenu}
					onCategoryContextMenu={handleCategoryContextMenu}
					onCreatePlaylist={handleCreatePlaylist}
					onCreateFolder={handleCreateFolder}
					onCreateCategory={handleCreateCategory}
					onCreateTag={handleCreateTag}
					onTagsWhitespaceContextMenu={handleTagsWhitespaceContextMenu}
					onTracksDrop={trackController.handleTracksDropOnPlaylist}
					onPlaylistMove={handlePlaylistDragMove}
				/>
			</div>

			<ResizeHandle onResize={handleSidebarResize} />

			<!-- Right: Main Content + Optional TrackEditor -->
			<div class="flex flex-1 overflow-hidden rounded-tl-md border-t border-l border-stroke">
				<!-- Main Content -->
				<div class="flex-1 overflow-hidden">
					{#if selectedFolderId}
						<FolderView
							folderId={selectedFolderId}
							{playlists}
							onSelect={handlePlaylistSelect}
							{breadcrumbItems}
							onBreadcrumbNavigate={handleBreadcrumbNavigate}
							onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
							onEmptySpaceContextMenu={handleFolderViewContextMenu}
							onCardContextMenu={handleFolderViewCardContextMenu}
						/>
					{:else if selectedPlaylistId}
						{@const playlist = playlists.find((p) => p.id === selectedPlaylistId)}
						{#if playlist}
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
								onEmptySpaceContextMenu={handlePlaylistViewContextMenu}
								onBreadcrumbNavigate={handleBreadcrumbNavigate}
								onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
								onTrackColorChange={trackController.setColor}
								onCancelAnalysis={handleCancelAnalysis}
							/>
						{/if}
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
							onEmptySpaceContextMenu={handleLibraryViewContextMenu}
							onTrackColorChange={trackController.setColor}
							onCancelAnalysis={handleCancelAnalysis}
						/>
					{/if}
				</div>

				<!-- Right Sidebar (Track Editor) -->
				<div
					class="flex h-full flex-shrink-0 overflow-hidden ease-out"
					class:transition-[width]={!isResizingRightSidebar}
					class:duration-250={!isResizingRightSidebar}
					class:animate-[fade-in_250ms_ease-out]={sidebarOpen}
					style="width: {sidebarOpen ? $rightSidebarWidth : 0}px"
					ontransitionend={(e) => {
						if (e.propertyName === 'width' && !sidebarOpen) {
							editorTracks = []
						}
					}}
				>
					<ResizeHandle
						onResize={handleRightSidebarResize}
						onResizeStart={() => (isResizingRightSidebar = true)}
						onResizeEnd={() => (isResizingRightSidebar = false)}
					/>
					<div style="width: {$rightSidebarWidth}px">
						<TrackEditor selectedTracks={editorTracks} />
					</div>
				</div>
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
	onPlaylistRename={handlePlaylistRename}
	onPlaylistDelete={handlePlaylistDelete}
	onPlaylistMove={handlePlaylistMove}
	onFolderViewCreatePlaylist={handleFolderViewCreatePlaylist}
	onFolderViewCreateFolder={handleFolderViewCreateFolder}
	onPlaylistTreeCreatePlaylist={handlePlaylistTreeCreatePlaylist}
	onPlaylistTreeCreateFolder={handlePlaylistTreeCreateFolder}
	onLibraryViewImport={trackController.handleImport}
	onPlaylistViewImport={handlePlaylistViewImport}
	onTagRename={(tag) => modalOrchestrator.openRenameTagModal(tag)}
	onTagDelete={(tag) => modalOrchestrator.openDeleteTagModal(tag)}
	onCategoryRename={(category) => modalOrchestrator.openRenameCategoryModal(category)}
	onCategoryDelete={(category) => modalOrchestrator.openDeleteCategoryModal(category)}
	onCategoryChangeColor={tagController.changeCategoryColor}
	onTagsSidebarAddCategory={handleTagsSidebarAddCategory}
	onTagsSidebarAddTag={handleTagsSidebarAddTag}
	onDeviceViewInfo={handleViewDeviceInfo}
	onDeviceRevealInFinder={handleDeviceRevealInFinder}
	onDeviceReformat={handleDeviceReformat}
	onDeviceEject={handleEjectDevice}
	onDeviceExport={handleDeviceExport}
	onDeviceIgnore={handleDeviceIgnore}
	onPlaylistExport={handlePlaylistExport}
	onClose={() => (contextMenuPlaylistId = null)}
/>

<!-- Modal Orchestrator -->
<ModalOrchestrator
	bind:this={modalOrchestrator}
	{playlists}
	{tagCategories}
	onCreatePlaylist={async (name, parentId) => {
		const playlist = await playlistsStore.createPlaylist(name, parentId ?? undefined)
		if (playlist) {
			uiStore.selectPlaylist(playlist.id)
			await libraryStore.loadPlaylistTracks(playlist.id)
		}
		return playlist
	}}
	onCreateFolder={async (name, parentId) => {
		const folder = await playlistsStore.createFolder(name, parentId ?? undefined)
		if (folder) {
			uiStore.selectFolder(folder.id)
		}
		return folder
	}}
	onCreateCategory={async (name) => {
		await tagsStore.createCategory(name)
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
	onDeletePlaylist={async (id, _deleteTracksToo) => {
		// Find playlist to get parent_id before deletion
		const playlist = playlists.find((p) => p.id === id)
		const parentId = playlist?.parent_id ?? null

		await playlistsStore.delete(id)

		// Navigate to parent folder or library
		if (parentId) {
			const parentFolder = playlists.find((p) => p.id === parentId)
			if (parentFolder) {
				uiStore.selectFolder(parentId)
			} else {
				handleLibraryClick()
			}
		} else {
			handleLibraryClick()
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
		// Always refresh playlists to update track counts (tracks may have been in playlists)
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
	onExport={handleExportSubmit}
	onQuickExport={handleQuickExportSubmit}
	onExportFailureKeep={handleExportFailureKeep}
	onExportFailureCleanup={handleExportFailureCleanup}
	onReformatDevice={handleReformatDevice}
/>

<!-- Drag Preview -->
{#if $isDragging && $dragPosition}
	<DragPreview data={$dragData} tracks={$libraryStore.tracks} {playlists} x={$dragPosition.x} y={$dragPosition.y} />
{/if}
