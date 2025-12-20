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
		MoveConflict,
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

	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { LibraryView, TrackContextMenu, RelocateTrackModal } from '$lib/components/library'
	import { Player } from '$lib/components/player'
	import { InputModal, ConfirmModal, MoveConflictModal, ResizeHandle, ContextMenu } from '$lib/components/common'
	import { PlaylistContextMenu, PlaylistView, FolderView } from '$lib/components/playlists'
	import { TagContextMenu, TagsSidebarContextMenu, TagInputModal } from '$lib/components/tags'
	import { DeviceContextMenu, DeviceInfoModal } from '$lib/components/devices'
	import { SettingsModal } from '$lib/components/settings'
	import * as devicesApi from '$lib/api/devices'
	import * as libraryApi from '$lib/api/library'
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

	// Modal state
	let showPlaylistModal = $state(false)
	let playlistModalParentId = $state<string | null>(null)
	let showFolderModal = $state(false)
	let folderModalParentId = $state<string | null>(null)
	let showCategoryModal = $state(false)
	let showTagModal = $state(false)
	let tagModalCategoryId = $state<string | null>(null)
	let showSettings = $state(false)

	// Drag and drop state
	let isDragOver = $state(false)

	// Track context menu state
	let contextMenuOpen = $state(false)
	let contextMenuPosition = $state({ x: 0, y: 0 })
	let contextMenuTracks = $state<Track[]>([])

	// Playlist context menu state
	let playlistContextMenuOpen = $state(false)
	let playlistContextMenuPosition = $state({ x: 0, y: 0 })
	let playlistContextMenuPlaylist = $state<Playlist | null>(null)
	let playlistContextMenuSource = $state<'tree' | 'folder' | null>(null)

	// Folder view context menu state (for empty space right-click)
	let folderViewContextMenuOpen = $state(false)
	let folderViewContextMenuPosition = $state({ x: 0, y: 0 })
	let folderViewContextMenuFolderId = $state<string | null>(null)

	// Playlist tree context menu state (for whitespace right-click)
	let playlistTreeContextMenuOpen = $state(false)
	let playlistTreeContextMenuPosition = $state({ x: 0, y: 0 })

	// Library view context menu state (for empty space right-click)
	let libraryViewContextMenuOpen = $state(false)
	let libraryViewContextMenuPosition = $state({ x: 0, y: 0 })

	// Playlist view context menu state (for empty space right-click)
	let playlistViewContextMenuOpen = $state(false)
	let playlistViewContextMenuPosition = $state({ x: 0, y: 0 })
	let playlistViewContextMenuPlaylist = $state<Playlist | null>(null)

	// Playlist modal states
	let showRenamePlaylistModal = $state(false)
	let renamePlaylistId = $state<string | null>(null)
	let renamePlaylistValue = $state('')
	let showDeletePlaylistConfirm = $state(false)
	let deletePlaylistId = $state<string | null>(null)
	let deletePlaylistIsFolder = $state(false)
	let deletePlaylistHasChildren = $state(false)
	let deleteTracksFromCollection = $state(false)

	// Track removal confirmation state
	let showRemoveFromPlaylistConfirm = $state(false)
	let showRemoveFromLibraryConfirm = $state(false)
	let removeTrackIds = $state<string[]>([])
	let removePlaylistId = $state<string | null>(null)

	// Tag context menu state
	let tagContextMenuOpen = $state(false)
	let tagContextMenuPosition = $state({ x: 0, y: 0 })
	let tagContextMenuTarget = $state<
		{ type: 'tag'; tag: Tag; category: TagCategory } | { type: 'category'; category: TagCategory } | null
	>(null)

	// Tags sidebar context menu state (whitespace right-click)
	let tagsSidebarContextMenuOpen = $state(false)
	let tagsSidebarContextMenuPosition = $state({ x: 0, y: 0 })

	// Tag input modal state (for adding tag from context menu)
	let showTagInputModal = $state(false)

	// Device context menu state
	let deviceContextMenuOpen = $state(false)
	let deviceContextMenuPosition = $state({ x: 0, y: 0 })
	let deviceContextMenuDevice = $state<UsbDevice | null>(null)

	// Device info modal state
	let showDeviceInfoModal = $state(false)
	let deviceInfoDevice = $state<UsbDevice | null>(null)

	// Relocate track modal state
	let showRelocateModal = $state(false)
	let relocateTrack = $state<Track | null>(null)

	// Move conflict modal state
	let showMoveConflictModal = $state(false)
	let moveConflictMovingItem = $state<Playlist | null>(null)
	let moveConflictExistingItem = $state<Playlist | null>(null)
	let moveConflictTargetParentId = $state<string | null>(null)
	let pendingMergeConflicts = $state<MoveConflict[]>([])

	// Tag modal states
	let showRenameTagModal = $state(false)
	let renameTagId = $state<string | null>(null)
	let renameTagValue = $state('')
	let showRenameCategoryModal = $state(false)
	let renameCategoryId = $state<string | null>(null)
	let renameCategoryValue = $state('')
	let showDeleteTagConfirm = $state(false)
	let deleteTagId = $state<string | null>(null)
	let showDeleteCategoryConfirm = $state(false)
	let deleteCategoryId = $state<string | null>(null)

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

		setupDragDrop()
		setupDeviceListener()

		return () => {
			window.removeEventListener('keydown', handleKeydown)
			unlistenDrop?.()
			unlistenDragOver?.()
			unlistenDragLeave?.()
			unlistenDevices?.()
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

		// Cmd/Ctrl+,: toggle settings
		if ((e.metaKey || e.ctrlKey) && e.key === ',') {
			e.preventDefault()
			showSettings = !showSettings
		}
	}

	function isInputFocused() {
		const active = document.activeElement
		return active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
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
	function handleLibraryClick() {
		uiStore.selectPlaylist(null)
		uiStore.selectFolder(null)
		uiStore.clearTagFilters()
		libraryStore.clearFilters()
		libraryStore.clearPlaylistTracks()
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
			await libraryStore.loadPlaylistTracks(playlist.id)
		}
	}

	async function handleTagSelect(tagId: string) {
		uiStore.toggleTagFilter(tagId)
		// Get updated selectedTagIds after toggle
		const updatedTagIds = selectedTagIds.includes(tagId)
			? selectedTagIds.filter((id) => id !== tagId)
			: [...selectedTagIds, tagId]
		if (updatedTagIds.length > 0) {
			await libraryStore.loadTracks({ tag_ids: updatedTagIds, tag_filter_mode: $tagFilterMode })
		} else {
			await libraryStore.loadTracks()
		}
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
	function handleClearTagFilter() {
		uiStore.clearTagFilters()
		libraryStore.clearFilters()
		libraryStore.loadTracks()
	}

	// Remove a single tag from filter
	async function handleRemoveTagFilter(tagId: string) {
		uiStore.removeTagFilter(tagId)
		const updatedTagIds = selectedTagIds.filter((id) => id !== tagId)
		if (updatedTagIds.length > 0) {
			await libraryStore.loadTracks({ tag_ids: updatedTagIds, tag_filter_mode: $tagFilterMode })
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
			const newMode = $tagFilterMode === 'or' ? 'and' : 'or'
			await libraryStore.loadTracks({ tag_ids: selectedTagIds, tag_filter_mode: newMode })
		}
	}

	function handleCreatePlaylist() {
		playlistModalParentId = selectedFolderId
		showPlaylistModal = true
	}

	async function handlePlaylistModalSubmit(name: string) {
		showPlaylistModal = false
		const playlist = await playlistsStore.createPlaylist(name, playlistModalParentId ?? undefined)
		playlistModalParentId = null
		if (playlist) {
			uiStore.selectPlaylist(playlist.id)
			await libraryStore.loadPlaylistTracks(playlist.id)
		}
	}

	function handleCreateFolder() {
		folderModalParentId = selectedFolderId
		showFolderModal = true
	}

	async function handleFolderModalSubmit(name: string) {
		showFolderModal = false
		const folder = await playlistsStore.createFolder(name, folderModalParentId ?? undefined)
		folderModalParentId = null
		if (folder) {
			uiStore.selectFolder(folder.id)
		}
	}

	function handleCreateCategory() {
		showCategoryModal = true
	}

	async function handleCategoryModalSubmit(name: string) {
		showCategoryModal = false
		await tagsStore.createCategory(name)
	}

	function handleCreateTag(categoryId: string) {
		tagModalCategoryId = categoryId
		showTagModal = true
	}

	async function handleTagModalSubmit(name: string) {
		showTagModal = false
		if (tagModalCategoryId) {
			await tagsStore.createTag(tagModalCategoryId, name)
			tagModalCategoryId = null
		}
	}

	// Context menu handlers
	function closeAllContextMenus() {
		contextMenuOpen = false
		playlistContextMenuOpen = false
		playlistContextMenuSource = null
		playlistTreeContextMenuOpen = false
		folderViewContextMenuOpen = false
		libraryViewContextMenuOpen = false
		playlistViewContextMenuOpen = false
		tagContextMenuOpen = false
		tagsSidebarContextMenuOpen = false
		deviceContextMenuOpen = false
	}

	function handleTrackContextMenu(e: MouseEvent, track: Track) {
		closeAllContextMenus()
		e.preventDefault()
		contextMenuPosition = { x: e.clientX, y: e.clientY }

		// If the clicked track is in the selection, use the selection
		// Otherwise, use just the clicked track
		const currentSelection = $selectedTrackIds
		if (currentSelection.has(track.id)) {
			contextMenuTracks = $sortedTracks.filter((t) => currentSelection.has(t.id))
		} else {
			contextMenuTracks = [track]
		}

		contextMenuOpen = true
	}

	async function handleAddToPlaylist(playlistId: string) {
		contextMenuOpen = false
		const trackIds = contextMenuTracks.map((t) => t.id)
		await playlistsStore.addTracks(playlistId, trackIds)
	}

	async function handleRevealInExplorer() {
		contextMenuOpen = false
		if (contextMenuTracks.length === 1) {
			await revealItemInDir(contextMenuTracks[0].file_path)
		}
	}

	// Track removal handlers
	function handleRemoveFromPlaylistClick() {
		contextMenuOpen = false
		removeTrackIds = contextMenuTracks.map((t) => t.id)
		removePlaylistId = selectedPlaylistId
		showRemoveFromPlaylistConfirm = true
	}

	async function handleRemoveFromPlaylistConfirm() {
		showRemoveFromPlaylistConfirm = false
		if (removePlaylistId && removeTrackIds.length > 0) {
			await playlistsStore.removeTracks(removePlaylistId, removeTrackIds)
			await libraryStore.loadPlaylistTracks(removePlaylistId)
			uiStore.clearSelection()
			const count = removeTrackIds.length
			toastStore.success(count === 1 ? '1 track removed from playlist' : `${count} tracks removed from playlist`)
		}
		removeTrackIds = []
		removePlaylistId = null
	}

	function handleRemoveFromLibraryClick() {
		contextMenuOpen = false
		removeTrackIds = contextMenuTracks.map((t) => t.id)
		showRemoveFromLibraryConfirm = true
	}

	async function handleRemoveFromLibraryConfirm() {
		showRemoveFromLibraryConfirm = false
		if (removeTrackIds.length > 0) {
			await libraryStore.deleteTracks(removeTrackIds)
			uiStore.clearSelection()
			if (selectedPlaylistId) {
				await libraryStore.loadPlaylistTracks(selectedPlaylistId)
				await playlistsStore.load()
			}
			const count = removeTrackIds.length
			toastStore.success(count === 1 ? '1 track removed from library' : `${count} tracks removed from library`)
		}
		removeTrackIds = []
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
		closeAllContextMenus()
		e.preventDefault()
		playlistContextMenuPosition = { x: e.clientX, y: e.clientY }
		playlistContextMenuPlaylist = playlist
		playlistContextMenuSource = 'tree'
		playlistContextMenuOpen = true
	}

	function handleFolderViewCardContextMenu(e: MouseEvent, playlist: Playlist) {
		closeAllContextMenus()
		e.preventDefault()
		playlistContextMenuPosition = { x: e.clientX, y: e.clientY }
		playlistContextMenuPlaylist = playlist
		playlistContextMenuSource = 'folder'
		playlistContextMenuOpen = true
	}

	// Playlist tree context menu handlers (right-click on whitespace)
	function handlePlaylistTreeContextMenu(e: MouseEvent) {
		closeAllContextMenus()
		e.preventDefault()
		playlistTreeContextMenuPosition = { x: e.clientX, y: e.clientY }
		playlistTreeContextMenuOpen = true
	}

	function handlePlaylistTreeCreatePlaylist() {
		playlistTreeContextMenuOpen = false
		playlistModalParentId = null
		showPlaylistModal = true
	}

	function handlePlaylistTreeCreateFolder() {
		playlistTreeContextMenuOpen = false
		folderModalParentId = null
		showFolderModal = true
	}

	// Tags sidebar context menu handlers (right-click on whitespace)
	function handleTagsWhitespaceContextMenu(e: MouseEvent) {
		closeAllContextMenus()
		e.preventDefault()
		tagsSidebarContextMenuPosition = { x: e.clientX, y: e.clientY }
		tagsSidebarContextMenuOpen = true
	}

	function handleTagsSidebarAddCategory() {
		tagsSidebarContextMenuOpen = false
		showCategoryModal = true
	}

	function handleTagsSidebarAddTag() {
		tagsSidebarContextMenuOpen = false
		showTagInputModal = true
	}

	async function handleTagInputModalSubmit(categoryId: string, tagName: string) {
		showTagInputModal = false
		await tagsStore.createTag(categoryId, tagName)
	}

	// Folder view context menu handlers (right-click on empty space)
	function handleFolderViewContextMenu(e: MouseEvent, folderId: string) {
		closeAllContextMenus()
		folderViewContextMenuPosition = { x: e.clientX, y: e.clientY }
		folderViewContextMenuFolderId = folderId
		folderViewContextMenuOpen = true
	}

	function handleFolderViewCreatePlaylist() {
		folderViewContextMenuOpen = false
		playlistModalParentId = folderViewContextMenuFolderId
		showPlaylistModal = true
	}

	function handleFolderViewCreateFolder() {
		folderViewContextMenuOpen = false
		folderModalParentId = folderViewContextMenuFolderId
		showFolderModal = true
	}

	// Library view context menu handlers (right-click on empty space)
	function handleLibraryViewContextMenu(e: MouseEvent) {
		closeAllContextMenus()
		e.preventDefault()
		libraryViewContextMenuPosition = { x: e.clientX, y: e.clientY }
		libraryViewContextMenuOpen = true
	}

	async function handleLibraryViewImport() {
		libraryViewContextMenuOpen = false
		await handleImport()
	}

	// Playlist view context menu handlers (right-click on empty space)
	function handlePlaylistViewContextMenu(e: MouseEvent, playlist: Playlist) {
		closeAllContextMenus()
		e.preventDefault()
		playlistViewContextMenuPosition = { x: e.clientX, y: e.clientY }
		playlistViewContextMenuPlaylist = playlist
		playlistViewContextMenuOpen = true
	}

	async function handlePlaylistViewImport() {
		playlistViewContextMenuOpen = false
		const playlist = playlistViewContextMenuPlaylist
		if (!playlist) return

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
		playlistContextMenuOpen = false
		renamePlaylistId = playlist.id
		renamePlaylistValue = playlist.name
		showRenamePlaylistModal = true
	}

	async function handleRenamePlaylistSubmit(name: string) {
		showRenamePlaylistModal = false
		if (renamePlaylistId) {
			await playlistsStore.rename(renamePlaylistId, name)
			renamePlaylistId = null
			renamePlaylistValue = ''
		}
	}

	function handlePlaylistDelete(playlist: Playlist) {
		playlistContextMenuOpen = false
		deletePlaylistId = playlist.id
		deletePlaylistIsFolder = playlist.is_folder
		// Check if folder has children
		deletePlaylistHasChildren = playlists.some((p) => p.parent_id === playlist.id)
		deleteTracksFromCollection = false
		showDeletePlaylistConfirm = true
	}

	async function handleDeletePlaylistConfirm(deleteTracksToo: boolean) {
		showDeletePlaylistConfirm = false
		if (deletePlaylistId) {
			// TODO: If deleteTracksToo is true, delete tracks from collection first
			await playlistsStore.delete(deletePlaylistId)
			deletePlaylistId = null
			deletePlaylistIsFolder = false
			deletePlaylistHasChildren = false
		}
	}

	// Helper to find a conflicting item in the target folder
	function findConflictingItem(movingItem: Playlist, targetParentId: string | null): Playlist | null {
		return (
			playlists.find((p) => p.parent_id === targetParentId && p.name === movingItem.name && p.id !== movingItem.id) ??
			null
		)
	}

	// Helper to get a playlist by ID
	function getPlaylistById(id: string): Playlist | null {
		return playlists.find((p) => p.id === id) ?? null
	}

	async function handlePlaylistMove(playlist: Playlist, folderId: string | null) {
		playlistContextMenuOpen = false

		// Check for conflict
		const conflict = findConflictingItem(playlist, folderId)

		if (conflict) {
			// Store context and show modal
			moveConflictMovingItem = playlist
			moveConflictExistingItem = conflict
			moveConflictTargetParentId = folderId
			showMoveConflictModal = true
			return
		}

		// No conflict, proceed with move
		await playlistsStore.move(playlist.id, folderId)
	}

	// Handler for drag-drop playlist move
	async function handlePlaylistDragMove(playlistId: string, targetFolderId: string | null) {
		const playlist = getPlaylistById(playlistId)
		if (!playlist) return

		// Check for conflict
		const conflict = findConflictingItem(playlist, targetFolderId)

		if (conflict) {
			// Store context and show modal
			moveConflictMovingItem = playlist
			moveConflictExistingItem = conflict
			moveConflictTargetParentId = targetFolderId
			showMoveConflictModal = true
			return
		}

		// No conflict, proceed with move
		const result = await playlistsStore.move(playlistId, targetFolderId)
		if (result) {
			toastStore.success('Moved successfully')
		}
	}

	// Move conflict resolution handlers
	function resetMoveConflictState() {
		moveConflictMovingItem = null
		moveConflictExistingItem = null
		moveConflictTargetParentId = null
	}

	function handleMoveConflictCancel() {
		showMoveConflictModal = false
		resetMoveConflictState()
		pendingMergeConflicts = []
	}

	async function handleMoveConflictOverwrite() {
		showMoveConflictModal = false

		if (moveConflictMovingItem && moveConflictTargetParentId !== undefined) {
			const result = await playlistsStore.moveWithResolution(
				moveConflictMovingItem.id,
				moveConflictTargetParentId,
				'overwrite'
			)
			if (result) {
				toastStore.success('Replaced existing item')
			}
		}

		resetMoveConflictState()
		pendingMergeConflicts = []
	}

	async function handleMoveConflictMerge() {
		showMoveConflictModal = false

		if (moveConflictMovingItem && moveConflictTargetParentId !== undefined) {
			const result = await playlistsStore.moveWithResolution(
				moveConflictMovingItem.id,
				moveConflictTargetParentId,
				'merge'
			)

			if (result) {
				// Check for nested conflicts from merge
				if (result.nestedConflicts.length > 0) {
					// Queue them for sequential resolution
					pendingMergeConflicts = result.nestedConflicts
					processNextMergeConflict()
					// Don't reset state - processNextMergeConflict set it up for the next conflict
					return
				} else {
					toastStore.success('Merged successfully')
				}
			}
		}

		resetMoveConflictState()
	}

	function processNextMergeConflict() {
		if (pendingMergeConflicts.length === 0) {
			toastStore.success('Merge completed')
			return
		}

		const next = pendingMergeConflicts[0]
		pendingMergeConflicts = pendingMergeConflicts.slice(1)

		moveConflictMovingItem = next.movingItem
		moveConflictExistingItem = next.existingItem
		// The target is the existing item's parent (which is the folder we're merging into)
		moveConflictTargetParentId = next.existingItem.parent_id
		showMoveConflictModal = true
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
		closeAllContextMenus()
		e.preventDefault()

		if (item.type === 'library') {
			// Library context menu - placeholder for now
			// TODO: Could show "New Playlist", "New Folder", "Import Tracks"
			return
		}

		// Reuse playlist context menu for folders/playlists
		if (item.playlist) {
			playlistContextMenuPosition = { x: e.clientX, y: e.clientY }
			playlistContextMenuPlaylist = item.playlist
			playlistContextMenuOpen = true
		}
	}

	// Helper to generate delete warnings
	function getDeleteWarnings(): string[] {
		const warnings: string[] = []
		if (deletePlaylistIsFolder && deletePlaylistHasChildren) {
			warnings.push('This folder contains playlists that will also be deleted.')
		}
		return warnings
	}

	// Tag context menu handlers
	function handleTagContextMenu(e: MouseEvent, tag: Tag, category: TagCategory) {
		closeAllContextMenus()
		e.preventDefault()
		tagContextMenuPosition = { x: e.clientX, y: e.clientY }
		tagContextMenuTarget = { type: 'tag', tag, category }
		tagContextMenuOpen = true
	}

	function handleCategoryContextMenu(e: MouseEvent, category: TagCategory) {
		closeAllContextMenus()
		e.preventDefault()
		tagContextMenuPosition = { x: e.clientX, y: e.clientY }
		tagContextMenuTarget = { type: 'category', category }
		tagContextMenuOpen = true
	}

	function handleRenameTag(tag: Tag) {
		tagContextMenuOpen = false
		renameTagId = tag.id
		renameTagValue = tag.name
		showRenameTagModal = true
	}

	async function handleRenameTagSubmit(name: string) {
		showRenameTagModal = false
		if (renameTagId) {
			await tagsStore.updateTag(renameTagId, name)
			renameTagId = null
			renameTagValue = ''
		}
	}

	function handleDeleteTag(tag: Tag) {
		tagContextMenuOpen = false
		deleteTagId = tag.id
		showDeleteTagConfirm = true
	}

	async function handleDeleteTagConfirm() {
		showDeleteTagConfirm = false
		if (deleteTagId) {
			await tagsStore.deleteTag(deleteTagId)
			deleteTagId = null
			// Reload tracks to reflect tag changes
			await libraryStore.loadTracks()
		}
	}

	function handleRenameCategory(category: TagCategory) {
		tagContextMenuOpen = false
		renameCategoryId = category.id
		renameCategoryValue = category.name
		showRenameCategoryModal = true
	}

	async function handleRenameCategorySubmit(name: string) {
		showRenameCategoryModal = false
		if (renameCategoryId) {
			await tagsStore.updateCategory(renameCategoryId, name)
			renameCategoryId = null
			renameCategoryValue = ''
		}
	}

	function handleDeleteCategory(category: TagCategory) {
		tagContextMenuOpen = false
		deleteCategoryId = category.id
		showDeleteCategoryConfirm = true
	}

	async function handleChangeCategoryColor(category: TagCategory, color: string | null) {
		tagContextMenuOpen = false
		await tagsStore.updateCategory(category.id, undefined, color ?? undefined)
	}

	async function handleDeleteCategoryConfirm() {
		showDeleteCategoryConfirm = false
		if (deleteCategoryId) {
			await tagsStore.deleteCategory(deleteCategoryId)
			deleteCategoryId = null
			// Reload tracks to reflect tag changes
			await libraryStore.loadTracks()
		}
	}

	// Device context menu handlers
	function handleDeviceContextMenu(e: MouseEvent, device: UsbDevice) {
		closeAllContextMenus()
		e.preventDefault()
		deviceContextMenuPosition = { x: e.clientX, y: e.clientY }
		deviceContextMenuDevice = device
		deviceContextMenuOpen = true
	}

	async function handleEjectDevice(device: UsbDevice) {
		deviceContextMenuOpen = false
		try {
			await devicesApi.ejectDevice(device.mount_point)
			toastStore.success(`${device.name} ejected`)
		} catch (error) {
			toastStore.error(`Failed to eject ${device.name}`)
			console.error('Eject error:', error)
		}
	}

	function handleViewDeviceInfo(device: UsbDevice) {
		deviceContextMenuOpen = false
		deviceInfoDevice = device
		showDeviceInfoModal = true
	}

	// Relocate track handlers
	function handleOpenRelocateModal(track: Track) {
		relocateTrack = track
		showRelocateModal = true
	}

	function handleRelocateComplete(updatedTrack: Track) {
		// Update the track in the library store
		libraryStore.loadTracks()
		toastStore.success(`Relocated "${updatedTrack.title || 'track'}"`)
	}

	function handleCloseRelocateModal() {
		showRelocateModal = false
		relocateTrack = null
	}

	// Track color change handler
	async function handleTrackColorChange(trackIds: string[], color: TrackColor | null) {
		await libraryStore.setTrackColors(trackIds, color)
	}

	// Context menu color handler
	async function handleContextMenuSetColor(color: TrackColor | null) {
		contextMenuOpen = false
		const trackIds = contextMenuTracks.map((t) => t.id)
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
		onSettings={() => (showSettings = true)}
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
				contextMenuPlaylistId={playlistContextMenuOpen && playlistContextMenuSource === 'tree'
					? (playlistContextMenuPlaylist?.id ?? null)
					: null}
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

<!-- Modals -->
<InputModal
	open={showPlaylistModal}
	title="New Playlist"
	placeholder="Playlist name"
	submitLabel="Create"
	onSubmit={handlePlaylistModalSubmit}
	onCancel={() => (showPlaylistModal = false)}
/>

<InputModal
	open={showFolderModal}
	title="New Folder"
	placeholder="Folder name"
	submitLabel="Create"
	onSubmit={handleFolderModalSubmit}
	onCancel={() => (showFolderModal = false)}
/>

<InputModal
	open={showCategoryModal}
	title="New Tag Category"
	placeholder="Category name"
	submitLabel="Create"
	onSubmit={handleCategoryModalSubmit}
	onCancel={() => (showCategoryModal = false)}
/>

<InputModal
	open={showTagModal}
	title="New Tag"
	placeholder="Tag name"
	submitLabel="Create"
	onSubmit={handleTagModalSubmit}
	onCancel={() => {
		showTagModal = false
		tagModalCategoryId = null
	}}
/>

<!-- Track Context Menu -->
<TrackContextMenu
	open={contextMenuOpen}
	x={contextMenuPosition.x}
	y={contextMenuPosition.y}
	selectedTracks={contextMenuTracks}
	{playlists}
	currentPlaylistId={selectedPlaylistId}
	onClose={() => (contextMenuOpen = false)}
	onRevealInExplorer={handleRevealInExplorer}
	onAddToPlaylist={handleAddToPlaylist}
	onRemoveFromPlaylist={handleRemoveFromPlaylistClick}
	onRemoveFromLibrary={handleRemoveFromLibraryClick}
	onRelocate={handleOpenRelocateModal}
	onSetColor={handleContextMenuSetColor}
/>

<!-- Playlist Context Menu -->
<PlaylistContextMenu
	open={playlistContextMenuOpen}
	x={playlistContextMenuPosition.x}
	y={playlistContextMenuPosition.y}
	playlist={playlistContextMenuPlaylist}
	folders={playlistFolders}
	onClose={() => (playlistContextMenuOpen = false)}
	onRename={handlePlaylistRename}
	onDelete={handlePlaylistDelete}
	onMove={handlePlaylistMove}
/>

<!-- Playlist Tree Context Menu (whitespace right-click) -->
<ContextMenu
	open={playlistTreeContextMenuOpen}
	x={playlistTreeContextMenuPosition.x}
	y={playlistTreeContextMenuPosition.y}
	items={[
		{ id: 'add-folder', label: 'New Folder', icon: 'folder', action: handlePlaylistTreeCreateFolder },
		{ id: 'add-playlist', label: 'New Playlist', icon: 'playlist', action: handlePlaylistTreeCreatePlaylist },
	]}
	onClose={() => (playlistTreeContextMenuOpen = false)}
/>

<!-- Folder View Context Menu (empty space right-click) -->
<ContextMenu
	open={folderViewContextMenuOpen}
	x={folderViewContextMenuPosition.x}
	y={folderViewContextMenuPosition.y}
	items={[
		{ id: 'add-folder', label: 'New Folder', icon: 'folder', action: handleFolderViewCreateFolder },
		{ id: 'add-playlist', label: 'New Playlist', icon: 'playlist', action: handleFolderViewCreatePlaylist },
	]}
	onClose={() => (folderViewContextMenuOpen = false)}
/>

<!-- Library View Context Menu (empty space right-click) -->
<ContextMenu
	open={libraryViewContextMenuOpen}
	x={libraryViewContextMenuPosition.x}
	y={libraryViewContextMenuPosition.y}
	items={[{ id: 'import', label: 'Import track', icon: 'upload', action: handleLibraryViewImport }]}
	onClose={() => (libraryViewContextMenuOpen = false)}
/>

<!-- Playlist View Context Menu (empty space right-click) -->
<ContextMenu
	open={playlistViewContextMenuOpen}
	x={playlistViewContextMenuPosition.x}
	y={playlistViewContextMenuPosition.y}
	items={[{ id: 'import', label: 'Import track', icon: 'upload', action: handlePlaylistViewImport }]}
	onClose={() => (playlistViewContextMenuOpen = false)}
/>

<!-- Rename Playlist Modal -->
<InputModal
	open={showRenamePlaylistModal}
	title="Rename"
	placeholder="Name"
	submitLabel="Save"
	initialValue={renamePlaylistValue}
	onSubmit={handleRenamePlaylistSubmit}
	onCancel={() => {
		showRenamePlaylistModal = false
		renamePlaylistId = null
		renamePlaylistValue = ''
	}}
/>

<!-- Delete Playlist Confirmation -->
<ConfirmModal
	open={showDeletePlaylistConfirm}
	title={deletePlaylistIsFolder ? 'Delete Folder' : 'Delete Playlist'}
	message={deletePlaylistIsFolder
		? 'Are you sure you want to delete this folder?'
		: 'Are you sure you want to delete this playlist?'}
	warnings={getDeleteWarnings()}
	checkboxLabel="Also delete tracks from my collection"
	bind:checkboxChecked={deleteTracksFromCollection}
	confirmLabel="Delete"
	destructive={true}
	onConfirm={handleDeletePlaylistConfirm}
	onCancel={() => {
		showDeletePlaylistConfirm = false
		deletePlaylistId = null
		deletePlaylistIsFolder = false
		deletePlaylistHasChildren = false
	}}
/>

<!-- Remove from Playlist Confirmation -->
<ConfirmModal
	open={showRemoveFromPlaylistConfirm}
	title="Remove from Playlist"
	message={removeTrackIds.length === 1
		? 'Are you sure you want to remove this track from the playlist?'
		: `Are you sure you want to remove ${removeTrackIds.length} tracks from the playlist?`}
	confirmLabel="Remove"
	destructive={true}
	onConfirm={handleRemoveFromPlaylistConfirm}
	onCancel={() => {
		showRemoveFromPlaylistConfirm = false
		removeTrackIds = []
		removePlaylistId = null
	}}
/>

<!-- Remove from Library Confirmation -->
<ConfirmModal
	open={showRemoveFromLibraryConfirm}
	title="Remove from Library"
	message={removeTrackIds.length === 1
		? 'Are you sure you want to remove this track from your library?'
		: `Are you sure you want to remove ${removeTrackIds.length} tracks from your library?`}
	warnings={['This action cannot be undone. Tracks will be removed from all playlists.']}
	confirmLabel="Remove"
	destructive={true}
	onConfirm={handleRemoveFromLibraryConfirm}
	onCancel={() => {
		showRemoveFromLibraryConfirm = false
		removeTrackIds = []
	}}
/>

<!-- Move Conflict Modal -->
<MoveConflictModal
	open={showMoveConflictModal}
	movingItem={moveConflictMovingItem}
	conflictingItem={moveConflictExistingItem}
	pendingCount={pendingMergeConflicts.length}
	onCancel={handleMoveConflictCancel}
	onOverwrite={handleMoveConflictOverwrite}
	onMerge={handleMoveConflictMerge}
/>

<!-- Tag Context Menu -->
<TagContextMenu
	open={tagContextMenuOpen}
	x={tagContextMenuPosition.x}
	y={tagContextMenuPosition.y}
	target={tagContextMenuTarget}
	onClose={() => (tagContextMenuOpen = false)}
	onRenameTag={handleRenameTag}
	onDeleteTag={handleDeleteTag}
	onRenameCategory={handleRenameCategory}
	onDeleteCategory={handleDeleteCategory}
	onChangeColor={handleChangeCategoryColor}
/>

<!-- Tags Sidebar Context Menu (whitespace right-click) -->
<TagsSidebarContextMenu
	open={tagsSidebarContextMenuOpen}
	x={tagsSidebarContextMenuPosition.x}
	y={tagsSidebarContextMenuPosition.y}
	categoryCount={tagCategories.length}
	onClose={() => (tagsSidebarContextMenuOpen = false)}
	onAddCategory={handleTagsSidebarAddCategory}
	onAddTag={handleTagsSidebarAddTag}
/>

<!-- Device Context Menu -->
<DeviceContextMenu
	open={deviceContextMenuOpen}
	x={deviceContextMenuPosition.x}
	y={deviceContextMenuPosition.y}
	device={deviceContextMenuDevice}
	onClose={() => (deviceContextMenuOpen = false)}
	onViewInfo={handleViewDeviceInfo}
	onEject={handleEjectDevice}
/>

<!-- Device Info Modal -->
<DeviceInfoModal
	open={showDeviceInfoModal}
	device={deviceInfoDevice}
	onClose={() => {
		showDeviceInfoModal = false
		deviceInfoDevice = null
	}}
/>

<!-- Tag Input Modal (for adding tag from context menu) -->
<TagInputModal
	open={showTagInputModal}
	categories={tagCategories}
	onSubmit={handleTagInputModalSubmit}
	onCancel={() => (showTagInputModal = false)}
/>

<!-- Rename Tag Modal -->
<InputModal
	open={showRenameTagModal}
	title="Rename Tag"
	placeholder="Tag name"
	submitLabel="Save"
	initialValue={renameTagValue}
	onSubmit={handleRenameTagSubmit}
	onCancel={() => {
		showRenameTagModal = false
		renameTagId = null
		renameTagValue = ''
	}}
/>

<!-- Delete Tag Confirmation -->
<ConfirmModal
	open={showDeleteTagConfirm}
	title="Delete Tag"
	message="Are you sure you want to delete this tag? It will be removed from all tracks."
	confirmLabel="Delete"
	destructive={true}
	onConfirm={handleDeleteTagConfirm}
	onCancel={() => {
		showDeleteTagConfirm = false
		deleteTagId = null
	}}
/>

<!-- Rename Category Modal -->
<InputModal
	open={showRenameCategoryModal}
	title="Rename Category"
	placeholder="Category name"
	submitLabel="Save"
	initialValue={renameCategoryValue}
	onSubmit={handleRenameCategorySubmit}
	onCancel={() => {
		showRenameCategoryModal = false
		renameCategoryId = null
		renameCategoryValue = ''
	}}
/>

<!-- Delete Category Confirmation -->
<ConfirmModal
	open={showDeleteCategoryConfirm}
	title="Delete Category"
	message="Are you sure you want to delete this category? All tags in this category will be removed from all tracks."
	confirmLabel="Delete"
	destructive={true}
	onConfirm={handleDeleteCategoryConfirm}
	onCancel={() => {
		showDeleteCategoryConfirm = false
		deleteCategoryId = null
	}}
/>

<!-- Settings Modal -->
<SettingsModal open={showSettings} onClose={() => (showSettings = false)} />

<!-- Relocate Track Modal -->
<RelocateTrackModal
	open={showRelocateModal}
	track={relocateTrack}
	onClose={handleCloseRelocateModal}
	onRelocate={handleRelocateComplete}
/>
