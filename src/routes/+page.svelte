<script lang="ts">
	import { onMount } from 'svelte'
	import { open } from '@tauri-apps/plugin-dialog'
	import { listen, type UnlistenFn } from '@tauri-apps/api/event'
	import { getCurrentWindow } from '@tauri-apps/api/window'

	import type { Track, SortConfig, Playlist, TagCategory, Tag } from '$lib/types'
	import {
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
		settingsStore,
	} from '$lib/stores'
	import { toastStore } from '$lib/stores/toast'

	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { TrackList, TrackContextMenu } from '$lib/components/library'
	import { Player } from '$lib/components/player'
	import { InputModal, ConfirmModal, ColorPicker } from '$lib/components/common'
	import { PlaylistContextMenu, FolderView } from '$lib/components/playlists'
	import { TagContextMenu } from '$lib/components/tags'
	import { SettingsModal } from '$lib/components/settings'

	// Local state
	let sortConfig = $state<SortConfig>({ field: 'date_added', direction: 'desc' })
	let playlists = $state<Playlist[]>([])
	let tagCategories = $state<TagCategory[]>([])
	let selectedPlaylistId = $state<string | null>(null)
	let selectedFolderId = $state<string | null>(null)
	let selectedTagId = $state<string | null>(null)

	// Modal state
	let showPlaylistModal = $state(false)
	let showFolderModal = $state(false)
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

	// Playlist modal states
	let showRenamePlaylistModal = $state(false)
	let renamePlaylistId = $state<string | null>(null)
	let renamePlaylistValue = $state('')
	let showDeletePlaylistConfirm = $state(false)
	let deletePlaylistId = $state<string | null>(null)
	let deletePlaylistIsFolder = $state(false)
	let deletePlaylistHasChildren = $state(false)
	let deleteTracksFromCollection = $state(false)

	// Tag context menu state
	let tagContextMenuOpen = $state(false)
	let tagContextMenuPosition = $state({ x: 0, y: 0 })
	let tagContextMenuTarget = $state<
		{ type: 'tag'; tag: Tag; category: TagCategory } | { type: 'category'; category: TagCategory } | null
	>(null)

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
	let showColorPicker = $state(false)
	let colorPickerCategoryId = $state<string | null>(null)
	let colorPickerCurrentColor = $state<string | null>(null)

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
			selectedTagId = state.selectedTagId
		})

		return () => {
			unsubPlaylists()
			unsubTags()
			unsubUI()
		}
	})

	// Initialize on mount
	onMount(async () => {
		await Promise.all([libraryStore.loadTracks(), tagsStore.load(), playlistsStore.load(), settingsStore.load()])

		// Set up keyboard shortcuts
		window.addEventListener('keydown', handleKeydown)

		// Set up Tauri drag and drop events
		let unlistenDrop: UnlistenFn | undefined
		let unlistenDragOver: UnlistenFn | undefined
		let unlistenDragLeave: UnlistenFn | undefined

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
							await libraryStore.importTracks(audioPaths)
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

		setupDragDrop()

		return () => {
			window.removeEventListener('keydown', handleKeydown)
			unlistenDrop?.()
			unlistenDragOver?.()
			unlistenDragLeave?.()
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
	}

	function isInputFocused() {
		const active = document.activeElement
		return active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement
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
		playerStore.play(track)
	}

	// Selection change
	function handleSelectionChange(ids: Set<string>) {
		uiStore.setSelectedTracks(ids)
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
		uiStore.selectTag(null)
		libraryStore.clearFilters()
		libraryStore.clearPlaylistTracks()
	}

	async function handlePlaylistSelect(playlist: Playlist) {
		if (playlist.is_folder) {
			uiStore.selectFolder(playlist.id)
		} else {
			uiStore.selectPlaylist(playlist.id)
			await libraryStore.loadPlaylistTracks(playlist.id)
		}
	}

	function handleTagSelect(tagId: string) {
		uiStore.selectTag(tagId)
		libraryStore.setFilter({ tag_ids: [tagId] })
	}

	function handleCreatePlaylist() {
		showPlaylistModal = true
	}

	async function handlePlaylistModalSubmit(name: string) {
		showPlaylistModal = false
		await playlistsStore.createPlaylist(name)
	}

	function handleCreateFolder() {
		showFolderModal = true
	}

	async function handleFolderModalSubmit(name: string) {
		showFolderModal = false
		await playlistsStore.createFolder(name)
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
	function handleTrackContextMenu(e: MouseEvent, track: Track) {
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

	async function handleAssignTag(tagId: string) {
		contextMenuOpen = false
		const trackIds = contextMenuTracks.map((t) => t.id)
		await tagsStore.assignTags(trackIds, [tagId])
		// Reload tracks to reflect tag changes
		await libraryStore.loadTracks()
	}

	async function handleRemoveTag(tagId: string) {
		contextMenuOpen = false
		const trackIds = contextMenuTracks.map((t) => t.id)
		await tagsStore.removeTags(trackIds, [tagId])
		// Reload tracks to reflect tag changes
		await libraryStore.loadTracks()
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
		e.preventDefault()
		playlistContextMenuPosition = { x: e.clientX, y: e.clientY }
		playlistContextMenuPlaylist = playlist
		playlistContextMenuOpen = true
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

	async function handlePlaylistMove(playlist: Playlist, folderId: string | null) {
		playlistContextMenuOpen = false
		await playlistsStore.move(playlist.id, folderId)
	}

	// Helper to get folders for move menu
	const playlistFolders = $derived(playlists.filter((p) => p.is_folder))

	// Category colors map for track list
	const categoryColors = $derived(new Map(tagCategories.map((c) => [c.id, c.color])))

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
		e.preventDefault()
		tagContextMenuPosition = { x: e.clientX, y: e.clientY }
		tagContextMenuTarget = { type: 'tag', tag, category }
		tagContextMenuOpen = true
	}

	function handleCategoryContextMenu(e: MouseEvent, category: TagCategory) {
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

	function handleChangeCategoryColor(category: TagCategory) {
		tagContextMenuOpen = false
		colorPickerCategoryId = category.id
		colorPickerCurrentColor = category.color
		showColorPicker = true
	}

	async function handleColorPickerSelect(color: string) {
		showColorPicker = false
		if (colorPickerCategoryId) {
			await tagsStore.updateCategory(colorPickerCategoryId, undefined, color)
			colorPickerCategoryId = null
			colorPickerCurrentColor = null
		}
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
</script>

<div class="flex h-full flex-col">
	<Toolbar onImport={handleImport} onSettings={() => (showSettings = true)} />

	<div class="flex flex-1 overflow-hidden">
		<div class="w-60 flex-shrink-0">
			<Sidebar
				{playlists}
				{tagCategories}
				{selectedPlaylistId}
				{selectedTagId}
				trackCount={$trackCount}
				onLibraryClick={handleLibraryClick}
				onPlaylistSelect={handlePlaylistSelect}
				onPlaylistContextMenu={handlePlaylistContextMenu}
				onTagSelect={handleTagSelect}
				onTagContextMenu={handleTagContextMenu}
				onCategoryContextMenu={handleCategoryContextMenu}
				onCreatePlaylist={handleCreatePlaylist}
				onCreateFolder={handleCreateFolder}
				onCreateCategory={handleCreateCategory}
				onCreateTag={handleCreateTag}
				onTracksDrop={handleTracksDropOnPlaylist}
			/>
		</div>

		<div class="flex-1 overflow-hidden">
			{#if selectedFolderId}
				<FolderView folderId={selectedFolderId} {playlists} onSelect={handlePlaylistSelect} />
			{:else}
				<TrackList
					tracks={$displayedTracks}
					selectedIds={$selectedTrackIds}
					playingTrackId={$currentTrack?.id ?? null}
					{sortConfig}
					{isDragOver}
					{categoryColors}
					onSelectionChange={handleSelectionChange}
					onTrackPlay={handleTrackPlay}
					onSortChange={handleSortChange}
					onContextMenu={handleTrackContextMenu}
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
	{tagCategories}
	onClose={() => (contextMenuOpen = false)}
	onAddToPlaylist={handleAddToPlaylist}
	onAssignTag={handleAssignTag}
	onRemoveTag={handleRemoveTag}
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

<!-- Category Color Picker -->
<ColorPicker
	open={showColorPicker}
	title="Category Color"
	selectedColor={colorPickerCurrentColor}
	onSelect={handleColorPickerSelect}
	onCancel={() => {
		showColorPicker = false
		colorPickerCategoryId = null
		colorPickerCurrentColor = null
	}}
/>

<!-- Settings Modal -->
<SettingsModal open={showSettings} onClose={() => (showSettings = false)} />
