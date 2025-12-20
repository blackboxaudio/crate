<script lang="ts">
	import { onMount } from 'svelte'
	import { open } from '@tauri-apps/plugin-dialog'

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
	import { createTagController, createTrackController } from '$lib/controllers'
	import { useAppInitialization, useKeyboardShortcuts, useMenuActions } from '$lib/hooks'

	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { LibraryView } from '$lib/components/library'
	import { Player } from '$lib/components/player'
	import { ResizeHandle, ContextMenuOrchestrator, ModalOrchestrator } from '$lib/components/common'
	import { PlaylistView, FolderView } from '$lib/components/playlists'
	import * as devicesApi from '$lib/api/devices'
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
		// Initialize app (stores, drag-drop, device listener)
		const cleanupApp = await useAppInitialization({
			stores: { appStore, libraryStore, tagsStore, playlistsStore, settingsStore, devicesStore },
			toastStore,
			onExternalFileDrop: trackController.handleExternalFileDrop,
			onDragStateChange: (dragOver) => {
				isDragOver = dragOver
			},
		})

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
			onOpenSettings: () => modalOrchestrator.openSettingsModal(),
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
			onDeselectAll: () => uiStore.clearSelection(),
			onPlayPause: () => playerStore.togglePlayPause(),
			onStop: () => playerStore.stop(),
			onOpenSettings: () => modalOrchestrator.openSettingsModal(),
		})

		return () => {
			cleanupApp()
			cleanupKeyboard()
			cleanupMenu()
		}
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

	// Relocate track complete handler
	function handleRelocateComplete(updatedTrack: Track) {
		// Update the track in the library store
		libraryStore.loadTracks()
		toastStore.success(`Relocated "${updatedTrack.title || 'track'}"`)
	}
</script>

<div class="flex h-full flex-col">
	<Toolbar
		{activeFilterTags}
		{tagColors}
		tagFilterMode={$tagFilterMode}
		onRemoveTagFilter={tagController.removeTagFilter}
		onClearAllTagFilters={tagController.clearTagFilters}
		onToggleTagFilterMode={tagController.toggleTagFilterMode}
		onImport={trackController.handleImport}
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
						onSelectionChange={trackController.handleSelectionChange}
						onTrackPlay={trackController.play}
						onSortChange={handleSortChange}
						onContextMenu={handleTrackContextMenu}
						onEmptySpaceContextMenu={handlePlaylistViewContextMenu}
						onBreadcrumbNavigate={handleBreadcrumbNavigate}
						onBreadcrumbContextMenu={handleBreadcrumbContextMenu}
						onTrackColorChange={trackController.setColor}
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
					onSelectionChange={trackController.handleSelectionChange}
					onTrackPlay={trackController.play}
					onSortChange={handleSortChange}
					onContextMenu={handleTrackContextMenu}
					onEmptySpaceContextMenu={handleLibraryViewContextMenu}
					onTrackColorChange={trackController.setColor}
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
	onTrackAddToPlaylist={trackController.addToPlaylist}
	onTrackRevealInExplorer={trackController.revealInExplorer}
	onTrackRemoveFromPlaylist={trackController.removeFromPlaylistClick}
	onTrackRemoveFromLibrary={trackController.removeFromLibraryClick}
	onTrackRelocate={(track) => modalOrchestrator.openRelocateModal(track)}
	onTrackSetColor={trackController.setColorFromContextMenu}
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
