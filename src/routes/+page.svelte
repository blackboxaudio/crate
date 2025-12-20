<script lang="ts">
	import { onMount } from 'svelte'
	import { open } from '@tauri-apps/plugin-dialog'
	import { revealItemInDir } from '@tauri-apps/plugin-opener'
	import { listen, type UnlistenFn } from '@tauri-apps/api/event'
	import { getCurrentWindow } from '@tauri-apps/api/window'

	import type {
		Track,
		TrackColor,
		SortConfig,
		Playlist,
		TagCategory,
		Tag,
		TagSelectionState,
		UsbDevice,
		BreadcrumbItem,
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
		computeTagStates,
		missingTracksStore,
		missingTrackIds,
	} from '$lib/stores'
	import { toastStore } from '$lib/stores/toast'
	import { buildBreadcrumbItems, getPlaylistChildren } from '$lib/stores/playlists'
	import { findConflictingItem, getPlaylistById, hasChildren } from '$lib/utils'

	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { LibraryView } from '$lib/components/library'
	import { Player } from '$lib/components/player'
	import { ResizeHandle, ContextMenuOrchestrator, ModalOrchestrator } from '$lib/components/common'
	import { PlaylistView, FolderView } from '$lib/components/playlists'
	import * as devicesApi from '$lib/api/devices'
	import * as libraryApi from '$lib/api/library'
	import { onMenuAction, type MenuAction } from '$lib/api/menu'
	import { openDevTools } from '$lib/api/app'

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

	// Drag and drop state
	let isDragOver = $state(false)

	// Orchestrator bindings
	let contextMenuOrchestrator: ReturnType<typeof ContextMenuOrchestrator>
	let modalOrchestrator: ReturnType<typeof ModalOrchestrator>

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
		const unsubDevices = devicesStore.subscribe((state) => {
			devices = state.devices
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

	// Initialize on mount
	onMount(async () => {
		await Promise.all([
			appStore.load(),
			libraryStore.loadTracks(),
			tagsStore.load(),
			playlistsStore.load(),
			settingsStore.load(),
			devicesStore.loadDevices(),
		])

		// Set up keyboard shortcuts
		window.addEventListener('keydown', handleKeydown)

		// Set up Tauri drag and drop events
		let unlistenDrop: UnlistenFn | undefined
		let unlistenDragOver: UnlistenFn | undefined
		let unlistenDragLeave: UnlistenFn | undefined
		let unlistenDevices: UnlistenFn | undefined
		let unlistenMenu: UnlistenFn | undefined

		const setupDragDrop = async () => {
			const appWindow = getCurrentWindow()

			// Listen for file drop from OS file explorer
			// Note: Tauri's onDragDropEvent only fires for external OS file drags,
			// not for internal HTML5 drags (like dragging tracks to playlists)
			unlistenDrop = await appWindow.onDragDropEvent(async (event) => {
				if (event.payload.type === 'drop') {
					isDragOver = false
					const paths = event.payload.paths
					if (paths && paths.length > 0) {
						// Filter for audio files
						const audioExtensions = ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac']
						const audioPaths = paths.filter((p) => {
							const ext = p.split('.').pop()?.toLowerCase()
							return ext && audioExtensions.includes(ext)
						})
						if (audioPaths.length > 0) {
							await handleExternalFileDrop(audioPaths)
						}
					}
				} else if (event.payload.type === 'enter') {
					// 'enter' event fires when external files are dragged into the window
					// and includes the file paths. 'over' events don't include paths.
					if (event.payload.paths && event.payload.paths.length > 0) {
						isDragOver = true
					}
				} else if (event.payload.type === 'leave' || event.payload.type === 'cancel') {
					isDragOver = false
				}
			})
		}

		// Set up device change listener
		const setupDeviceListener = async () => {
			unlistenDevices = await listen<UsbDevice[]>('devices-changed', (event) => {
				const previousDevices = devicesStore.getDevices()
				const newDevices = event.payload

				// Detect new devices (connected)
				const prevIds = new Set(previousDevices.map((d) => d.id))
				for (const device of newDevices) {
					if (!prevIds.has(device.id)) {
						toastStore.info(`${device.name} connected`)
					}
				}

				// Detect removed devices (disconnected)
				const newIds = new Set(newDevices.map((d) => d.id))
				for (const device of previousDevices) {
					if (!newIds.has(device.id)) {
						toastStore.info(`${device.name} disconnected`)
					}
				}

				devicesStore.setDevices(newDevices)
			})
		}

		// Set up menu action listener
		const setupMenuListener = async () => {
			unlistenMenu = await onMenuAction(handleMenuAction)
		}

		setupDragDrop()
		setupDeviceListener()
		setupMenuListener()

		return () => {
			window.removeEventListener('keydown', handleKeydown)
			unlistenDrop?.()
			unlistenDragOver?.()
			unlistenDragLeave?.()
			unlistenDevices?.()
			unlistenMenu?.()
		}
	})

	// Keyboard shortcuts
	function handleKeydown(e: KeyboardEvent) {
		// Space: toggle play/pause
		if (e.code === 'Space' && !isInputFocused()) {
			e.preventDefault()
			playerStore.togglePlayPause()
		}

		// Cmd/Ctrl+F: focus search
		if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
			e.preventDefault()
			const searchInput = document.querySelector('input[type="search"]') as HTMLInputElement
			searchInput?.focus()
		}

		// Escape: clear selection
		if (e.key === 'Escape') {
			uiStore.clearSelection()
		}

		// Cmd/Ctrl+A: select all
		if ((e.metaKey || e.ctrlKey) && e.key === 'a' && !isInputFocused()) {
			e.preventDefault()
			const allIds = new Set($sortedTracks.map((t) => t.id))
			uiStore.setSelectedTracks(allIds)
		}

		// Cmd/Ctrl+,: open settings
		if ((e.metaKey || e.ctrlKey) && e.key === ',') {
			e.preventDefault()
			modalOrchestrator.openSettingsModal()
		}
	}

	function isInputFocused() {
		const active = document.activeElement
		return active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
	}

	// Menu action handler
	function handleMenuAction(action: MenuAction) {
		switch (action) {
			case 'import_tracks':
				handleImport()
				break
			case 'new_playlist':
				handleCreatePlaylist()
				break
			case 'new_folder':
				handleCreateFolder()
				break
			case 'select_all':
				if (!isInputFocused()) {
					const allIds = new Set($sortedTracks.map((t) => t.id))
					uiStore.setSelectedTracks(allIds)
				}
				break
			case 'deselect_all':
				uiStore.clearSelection()
				break
			case 'play_pause':
				if (!isInputFocused()) {
					playerStore.togglePlayPause()
				}
				break
			case 'stop':
				playerStore.stop()
				break
			case 'toggle_sidebar':
				// TODO: Implement sidebar toggle
				break
			case 'settings':
				modalOrchestrator.openSettingsModal()
				break
			case 'documentation':
				// TODO: Open documentation
				break
			case 'report_issue':
				// TODO: Open issue reporting
				break
		}
	}

	// External file drop handler (from OS file explorer)
	async function handleExternalFileDrop(audioPaths: string[]) {
		if (selectedPlaylistId) {
			// Import and add to playlist with combined toast
			const result = await libraryApi.importTracks(audioPaths)

			// Update library store state
			libraryStore.addTracksToState(result.tracks)

			if (result.tracks.length > 0) {
				const trackIds = result.tracks.map((t) => t.id)
				const playlist = playlists.find((p) => p.id === selectedPlaylistId)
				const playlistName = playlist?.name || 'playlist'

				try {
					await playlistsStore.addTracks(selectedPlaylistId, trackIds)
					const count = result.tracks.length
					const trackWord = count === 1 ? 'track' : 'tracks'
					if (result.failed_count > 0) {
						toastStore.warning(
							`${count} ${trackWord} imported and added to ${playlistName}, ${result.failed_count} failed`
						)
					} else {
						toastStore.success(`${count} ${trackWord} imported and added to ${playlistName}`)
					}
				} catch {
					toastStore.warning(`Tracks imported but failed to add to ${playlistName}`)
				}
			} else if (result.failed_count > 0) {
				toastStore.error(`Failed to import tracks: ${result.errors[0] || 'Unknown error'}`)
			}
		} else {
			// Library/folder view - use standard import with its own toast
			await libraryStore.importTracks(audioPaths)
		}
	}

	// Import handler
	async function handleImport() {
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
			await libraryStore.importTracks(selected)
		}
	}

	// Track playback
	function handleTrackPlay(track: Track) {
		// If track is missing, open relocate modal instead of trying to play
		if ($missingTrackIds.has(track.id)) {
			handleOpenRelocateModal(track)
			return
		}
		playerStore.play(track)
	}

	// Selection change
	function handleSelectionChange(ids: Set<string>) {
		uiStore.setSelectedTracks(ids)

		// Check file existence for newly selected tracks (lazy load)
		// Only check when selecting a single track to avoid excessive checks
		if (ids.size === 1) {
			const trackId = [...ids][0]
			// Don't check if already known to be missing or currently checking
			if (!$missingTrackIds.has(trackId) && !missingTracksStore.isChecking(trackId)) {
				missingTracksStore.checkTrack(trackId)
			}
		}
	}

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

	async function handleTagSelect(tagId: string) {
		// Capture current state BEFORE toggling (subscription updates synchronously)
		const wasSelected = selectedTagIds.includes(tagId)
		const updatedTagIds = wasSelected ? selectedTagIds.filter((id) => id !== tagId) : [...selectedTagIds, tagId]

		uiStore.toggleTagFilter(tagId)

		const filter: TrackFilter = {}
		if (updatedTagIds.length > 0) {
			filter.tag_ids = updatedTagIds
			filter.tag_filter_mode = $tagFilterMode
		}
		if (selectedPlaylistId) {
			filter.playlist_id = selectedPlaylistId
		}
		await libraryStore.loadTracks(Object.keys(filter).length > 0 ? filter : undefined)
	}

	// Tag toggle handler for when tracks are selected
	async function handleTagToggle(tagId: string, currentState: TagSelectionState) {
		const trackIds = Array.from($selectedTrackIds)

		if (currentState === 'active') {
			// Remove from all selected tracks
			await tagsStore.removeTags(trackIds, [tagId])
		} else if (currentState === 'inactive') {
			// Add to all selected tracks
			await tagsStore.assignTags(trackIds, [tagId])
		} else if (currentState === 'mixed') {
			// Check if this tag was recently toggled
			const wasRecentlyToggled = $recentlyToggledMixedTags.has(tagId)

			if (wasRecentlyToggled) {
				// Second click on mixed = add to all
				await tagsStore.assignTags(trackIds, [tagId])
				uiStore.clearRecentlyToggledTag(tagId)
			} else {
				// First click on mixed = remove from all
				await tagsStore.removeTags(trackIds, [tagId])
				uiStore.markTagAsRecentlyToggled(tagId)
			}
		}

		// Reload tracks to reflect tag changes
		await libraryStore.loadTracks()
	}

	// Clear tag filter
	async function handleClearTagFilter() {
		uiStore.clearTagFilters()
		libraryStore.clearFilters()
		if (selectedPlaylistId) {
			await libraryStore.loadPlaylistTracks(selectedPlaylistId)
		} else {
			await libraryStore.loadTracks()
		}
	}

	// Remove a single tag from filter
	async function handleRemoveTagFilter(tagId: string) {
		uiStore.removeTagFilter(tagId)
		const updatedTagIds = selectedTagIds.filter((id) => id !== tagId)

		const filter: TrackFilter = {}
		if (updatedTagIds.length > 0) {
			filter.tag_ids = updatedTagIds
			filter.tag_filter_mode = $tagFilterMode
		}
		if (selectedPlaylistId) {
			filter.playlist_id = selectedPlaylistId
		}

		if (Object.keys(filter).length > 0) {
			await libraryStore.loadTracks(filter)
		} else {
			libraryStore.clearFilters()
			await libraryStore.loadTracks()
		}
	}

	// Toggle tag filter mode (AND/OR)
	async function handleToggleTagFilterMode() {
		uiStore.toggleTagFilterMode()
		// Reload tracks with the new mode if tags are selected
		if (selectedTagIds.length > 0) {
			const filter: TrackFilter = {
				tag_ids: selectedTagIds,
				tag_filter_mode: $tagFilterMode,
			}
			if (selectedPlaylistId) {
				filter.playlist_id = selectedPlaylistId
			}
			await libraryStore.loadTracks(filter)
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

	async function handleAddToPlaylist(playlistId: string, tracks: Track[]) {
		const trackIds = tracks.map((t) => t.id)
		await playlistsStore.addTracks(playlistId, trackIds)
	}

	async function handleRevealInExplorer(track: Track) {
		await revealItemInDir(track.file_path)
	}

	// Track removal handlers
	function handleRemoveFromPlaylistClick(tracks: Track[]) {
		if (selectedPlaylistId) {
			const trackIds = tracks.map((t) => t.id)
			modalOrchestrator.openRemoveFromPlaylistModal(trackIds, selectedPlaylistId)
		}
	}

	function handleRemoveFromLibraryClick(tracks: Track[]) {
		const trackIds = tracks.map((t) => t.id)
		modalOrchestrator.openRemoveFromLibraryModal(trackIds)
	}

	// Drag and drop handlers
	async function handleTracksDropOnPlaylist(playlistId: string, trackIds: string[]) {
		try {
			await playlistsStore.addTracks(playlistId, trackIds)
			// Find playlist name for the toast message
			const playlist = playlists.find((p) => p.id === playlistId)
			const playlistName = playlist?.name || 'playlist'
			const count = trackIds.length
			toastStore.success(count === 1 ? `1 track added to ${playlistName}` : `${count} tracks added to ${playlistName}`)
		} catch (error) {
			toastStore.error('Failed to add tracks to playlist')
		}
	}

	// Playlist context menu handlers
	function handlePlaylistContextMenu(e: MouseEvent, playlist: Playlist) {
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

	async function handleLibraryViewImport() {
		await handleImport()
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

	function handleRenameTag(tag: Tag) {
		modalOrchestrator.openRenameTagModal(tag)
	}

	function handleDeleteTag(tag: Tag) {
		modalOrchestrator.openDeleteTagModal(tag)
	}

	function handleRenameCategory(category: TagCategory) {
		modalOrchestrator.openRenameCategoryModal(category)
	}

	function handleDeleteCategory(category: TagCategory) {
		modalOrchestrator.openDeleteCategoryModal(category)
	}

	async function handleChangeCategoryColor(category: TagCategory, color: string | null) {
		await tagsStore.updateCategory(category.id, undefined, color ?? undefined)
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

	// Relocate track handlers
	function handleOpenRelocateModal(track: Track) {
		modalOrchestrator.openRelocateModal(track)
	}

	function handleRelocateComplete(updatedTrack: Track) {
		// Update the track in the library store
		libraryStore.loadTracks()
		toastStore.success(`Relocated "${updatedTrack.title || 'track'}"`)
	}

	// Track color change handler
	async function handleTrackColorChange(trackIds: string[], color: TrackColor | null) {
		await libraryStore.setTrackColors(trackIds, color)
	}

	// Context menu color handler
	async function handleContextMenuSetColor(color: TrackColor | null, tracks: Track[]) {
		const trackIds = tracks.map((t) => t.id)
		await libraryStore.setTrackColors(trackIds, color)
	}
</script>

<div class="flex h-full flex-col">
	<Toolbar
		{activeFilterTags}
		{tagColors}
		tagFilterMode={$tagFilterMode}
		onRemoveTagFilter={handleRemoveTagFilter}
		onClearAllTagFilters={handleClearTagFilter}
		onToggleTagFilterMode={handleToggleTagFilterMode}
		onImport={handleImport}
		onSettings={() => modalOrchestrator.openSettingsModal()}
		onDevTools={openDevTools}
	/>

	<div class="flex flex-1 overflow-hidden">
		<div class="flex-shrink-0" style="width: {sidebarWidth}px">
			<Sidebar
				{playlists}
				{tagCategories}
				{devices}
				{selectedPlaylistId}
				{selectedFolderId}
				contextMenuPlaylistId={null}
				{selectedTagIds}
				selectedTrackIds={$selectedTrackIds}
				{tagStates}
				{tagCounts}
				trackCount={$trackCount}
				onLibraryClick={handleLibraryClick}
				onPlaylistSelect={handlePlaylistSelect}
				onPlaylistContextMenu={handlePlaylistContextMenu}
				onPlaylistTreeContextMenu={handlePlaylistTreeContextMenu}
				onDeviceContextMenu={handleDeviceContextMenu}
				onTagSelect={handleTagSelect}
				onTagToggle={handleTagToggle}
				onTagContextMenu={handleTagContextMenu}
				onCategoryContextMenu={handleCategoryContextMenu}
				onCreatePlaylist={handleCreatePlaylist}
				onCreateFolder={handleCreateFolder}
				onCreateCategory={handleCreateCategory}
				onCreateTag={handleCreateTag}
				onTagsWhitespaceContextMenu={handleTagsWhitespaceContextMenu}
				onTracksDrop={handleTracksDropOnPlaylist}
				onPlaylistMove={handlePlaylistDragMove}
			/>
		</div>

		<ResizeHandle onResize={handleSidebarResize} />

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
						{breadcrumbItems}
						onSelectionChange={handleSelectionChange}
						onTrackPlay={handleTrackPlay}
						onSortChange={handleSortChange}
						onContextMenu={handleTrackContextMenu}
						onEmptySpaceContextMenu={handlePlaylistViewContextMenu}
						onBreadcrumbNavigate={handleBreadcrumbNavigate}
						onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
						onTrackColorChange={handleTrackColorChange}
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
					onSelectionChange={handleSelectionChange}
					onTrackPlay={handleTrackPlay}
					onSortChange={handleSortChange}
					onContextMenu={handleTrackContextMenu}
					onEmptySpaceContextMenu={handleLibraryViewContextMenu}
					onTrackColorChange={handleTrackColorChange}
				/>
			{/if}
		</div>
	</div>

	<Player />
</div>

<!-- Context Menu Orchestrator -->
<ContextMenuOrchestrator
	bind:this={contextMenuOrchestrator}
	{playlists}
	currentPlaylistId={selectedPlaylistId}
	{playlistFolders}
	categoryCount={tagCategories.length}
	onTrackAddToPlaylist={handleAddToPlaylist}
	onTrackRevealInExplorer={handleRevealInExplorer}
	onTrackRemoveFromPlaylist={handleRemoveFromPlaylistClick}
	onTrackRemoveFromLibrary={handleRemoveFromLibraryClick}
	onTrackRelocate={handleOpenRelocateModal}
	onTrackSetColor={handleContextMenuSetColor}
	onPlaylistRename={handlePlaylistRename}
	onPlaylistDelete={handlePlaylistDelete}
	onPlaylistMove={handlePlaylistMove}
	onFolderViewCreatePlaylist={handleFolderViewCreatePlaylist}
	onFolderViewCreateFolder={handleFolderViewCreateFolder}
	onPlaylistTreeCreatePlaylist={handlePlaylistTreeCreatePlaylist}
	onPlaylistTreeCreateFolder={handlePlaylistTreeCreateFolder}
	onLibraryViewImport={handleLibraryViewImport}
	onPlaylistViewImport={handlePlaylistViewImport}
	onTagRename={handleRenameTag}
	onTagDelete={handleDeleteTag}
	onCategoryRename={handleRenameCategory}
	onCategoryDelete={handleDeleteCategory}
	onCategoryChangeColor={handleChangeCategoryColor}
	onTagsSidebarAddCategory={handleTagsSidebarAddCategory}
	onTagsSidebarAddTag={handleTagsSidebarAddTag}
	onDeviceViewInfo={handleViewDeviceInfo}
	onDeviceEject={handleEjectDevice}
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
		await playlistsStore.delete(id)
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
			await playlistsStore.load()
		}
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
/>
