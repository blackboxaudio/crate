<script lang="ts">
	import { onMount } from 'svelte'
	import { open } from '@tauri-apps/plugin-dialog'

	import type { Track, SortConfig, Playlist, TagCategory } from '$lib/types'
	import {
		libraryStore,
		sortedTracks,
		trackCount,
		playerStore,
		currentTrack,
		tagsStore,
		playlistsStore,
		uiStore,
		selectedTrackIds,
	} from '$lib/stores'

	import { Sidebar, Toolbar } from '$lib/components/layout'
	import { TrackList } from '$lib/components/library'
	import { Player } from '$lib/components/player'

	// Local state
	let sortConfig = $state<SortConfig>({ field: 'date_added', direction: 'desc' })
	let playlists = $state<Playlist[]>([])
	let tagCategories = $state<TagCategory[]>([])
	let selectedPlaylistId = $state<string | null>(null)
	let selectedTagId = $state<string | null>(null)

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
		await Promise.all([libraryStore.loadTracks(), tagsStore.load(), playlistsStore.load()])

		// Set up keyboard shortcuts
		window.addEventListener('keydown', handleKeydown)

		// Set up drag and drop
		window.addEventListener('dragover', handleDragOver)
		window.addEventListener('drop', handleDrop)

		return () => {
			window.removeEventListener('keydown', handleKeydown)
			window.removeEventListener('dragover', handleDragOver)
			window.removeEventListener('drop', handleDrop)
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

	// Drag and drop
	function handleDragOver(e: DragEvent) {
		e.preventDefault()
		if (e.dataTransfer) {
			e.dataTransfer.dropEffect = 'copy'
		}
	}

	async function handleDrop(e: DragEvent) {
		e.preventDefault()
		const files = e.dataTransfer?.files
		if (!files || files.length === 0) return

		const paths: string[] = []
		for (const file of files) {
			// In Tauri, we can get the path from the file
			if ('path' in file && typeof file.path === 'string') {
				paths.push(file.path)
			}
		}

		if (paths.length > 0) {
			await libraryStore.importTracks(paths)
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
		uiStore.selectTag(null)
		libraryStore.clearFilters()
	}

	function handlePlaylistSelect(playlist: Playlist) {
		uiStore.selectPlaylist(playlist.id)
	}

	function handleTagSelect(tagId: string) {
		uiStore.selectTag(tagId)
		libraryStore.setFilter({ tag_ids: [tagId] })
	}

	async function handleCreatePlaylist() {
		const name = prompt('Playlist name:')
		if (name) {
			await playlistsStore.createPlaylist(name)
		}
	}

	async function handleCreateCategory() {
		const name = prompt('Category name:')
		if (name) {
			await tagsStore.createCategory(name)
		}
	}
</script>

<div class="flex h-full flex-col">
	<Toolbar onImport={handleImport} />

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
				onTagSelect={handleTagSelect}
				onCreatePlaylist={handleCreatePlaylist}
				onCreateCategory={handleCreateCategory}
			/>
		</div>

		<div class="flex-1 overflow-hidden">
			<TrackList
				tracks={$sortedTracks}
				selectedIds={$selectedTrackIds}
				playingTrackId={$currentTrack?.id ?? null}
				{sortConfig}
				onSelectionChange={handleSelectionChange}
				onTrackPlay={handleTrackPlay}
				onSortChange={handleSortChange}
			/>
		</div>
	</div>

	<Player />
</div>
