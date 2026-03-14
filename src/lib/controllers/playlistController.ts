import { open } from '@tauri-apps/plugin-dialog'
import type { ActiveView, DiscoveryFilter, Playlist, TrackFilter } from '$lib/types'
import { withNativeDialog } from '$lib/utils'
import type { playlistsStore as PlaylistsStoreType } from '$lib/stores/playlists'
import type { discoveryStore as DiscoveryStoreType } from '$lib/stores/discovery'
import type { libraryStore as LibraryStoreType } from '$lib/stores/library'
import type { uiStore as UIStoreType } from '$lib/stores/ui'
import type { toastStore as ToastStoreType } from '$lib/stores/toast'
import { findConflictingItem, getPlaylistById, hasChildren } from '$lib/utils'

// =============================================================================
// Types
// =============================================================================

export interface PlaylistControllerDeps {
	playlistsStore: typeof PlaylistsStoreType
	discoveryStore: typeof DiscoveryStoreType
	libraryStore: typeof LibraryStoreType
	uiStore: typeof UIStoreType
	toastStore: typeof ToastStoreType
	getPlaylists: () => Playlist[]
	getSelectedPlaylistId: () => string | null
	getSelectedFolderId: () => string | null
	getSelectedTagIds: () => string[]
	getTagFilterMode: () => 'and' | 'or'
	getActiveView: () => ActiveView
	onDiscoveryPlaylistSelected?: (playlistId: string) => Promise<void>
}

export interface PlaylistControllerModalActions {
	openCreatePlaylistModal: (parentId: string | null) => void
	openCreateFolderModal: (parentId: string | null) => void
	openCreateSmartPlaylistModal: (parentId: string | null, context: ActiveView) => void
	openEditSmartPlaylistModal: (playlist: Playlist) => void
	openRenamePlaylistModal: (playlist: Playlist) => void
	openDeletePlaylistModal: (playlist: Playlist, hasChildren: boolean) => void
	openMoveConflictModal: (playlist: Playlist, conflict: Playlist, targetId: string | null) => void
}

export interface PlaylistController {
	handleLibraryClick: () => Promise<void>
	handlePlaylistSelect: (playlist: Playlist) => Promise<void>
	handleCreatePlaylist: () => void
	handleCreateFolder: () => void
	handleCreateSmartPlaylist: (context: ActiveView) => void
	handleEditSmartPlaylist: (playlist: Playlist) => void
	handlePlaylistRename: (playlist: Playlist) => void
	handlePlaylistDelete: (playlist: Playlist) => void
	handlePlaylistMove: (playlist: Playlist, folderId: string | null) => Promise<void>
	handlePlaylistDragMove: (playlistId: string, targetFolderId: string | null) => Promise<void>
	handleBulkPlaylistMove: (playlistIds: string[], targetFolderId: string | null) => Promise<void>
	handlePlaylistViewImport: (playlist: Playlist) => Promise<void>
}

// =============================================================================
// Controller Factory
// =============================================================================

export function createPlaylistController(
	deps: PlaylistControllerDeps,
	modalActions: PlaylistControllerModalActions
): PlaylistController {
	const {
		playlistsStore,
		discoveryStore,
		libraryStore,
		uiStore,
		toastStore,
		getPlaylists,
		getSelectedPlaylistId,
		getSelectedFolderId,
		getSelectedTagIds,
		getTagFilterMode,
		getActiveView,
		onDiscoveryPlaylistSelected,
	} = deps

	/**
	 * Navigate to library view
	 */
	async function handleLibraryClick(): Promise<void> {
		uiStore.clearSelection()
		uiStore.selectPlaylist(null)
		uiStore.selectFolder(null)

		const selectedTagIds = getSelectedTagIds()
		const tagFilterMode = getTagFilterMode()

		if (getActiveView() === 'discovery') {
			const filter: DiscoveryFilter = {}
			if (selectedTagIds.length > 0) {
				filter.tag_ids = selectedTagIds
				filter.tag_filter_mode = tagFilterMode
			}
			await discoveryStore.loadReleases(Object.keys(filter).length > 0 ? filter : undefined)
		} else {
			libraryStore.clearPlaylistTracks()
			if (selectedTagIds.length > 0) {
				await libraryStore.loadTracks({ tag_ids: selectedTagIds, tag_filter_mode: tagFilterMode })
			} else {
				libraryStore.clearFilters()
				await libraryStore.loadTracks()
			}
		}
	}

	/**
	 * Select a playlist or folder
	 */
	async function handlePlaylistSelect(playlist: Playlist): Promise<void> {
		// Clear track selection when selecting a folder or playlist
		uiStore.clearSelection()

		if (playlist.is_folder) {
			uiStore.selectFolder(playlist.id)
		} else if (playlist.is_smart) {
			uiStore.selectPlaylist(playlist.id)
			if (playlist.context === 'discovery') {
				await onDiscoveryPlaylistSelected?.(playlist.id)
			} else {
				await libraryStore.loadSmartPlaylistTracks(playlist.id)
			}
		} else if (playlist.context === 'discovery') {
			uiStore.selectPlaylist(playlist.id)
			await onDiscoveryPlaylistSelected?.(playlist.id)
		} else {
			uiStore.selectPlaylist(playlist.id)
			// Apply existing tag filters to the playlist (if any)
			const selectedTagIds = getSelectedTagIds()
			if (selectedTagIds.length > 0) {
				const filter: TrackFilter = {
					playlist_id: playlist.id,
					tag_ids: selectedTagIds,
					tag_filter_mode: getTagFilterMode(),
				}
				await libraryStore.loadTracks(filter)
			} else {
				await libraryStore.loadPlaylistTracks(playlist.id)
			}
		}
	}

	/**
	 * Open create playlist modal
	 */
	function handleCreatePlaylist(): void {
		modalActions.openCreatePlaylistModal(getSelectedFolderId())
	}

	/**
	 * Open create folder modal
	 */
	function handleCreateFolder(): void {
		modalActions.openCreateFolderModal(getSelectedFolderId())
	}

	/**
	 * Open create smart playlist modal
	 */
	function handleCreateSmartPlaylist(context: ActiveView): void {
		modalActions.openCreateSmartPlaylistModal(getSelectedFolderId(), context)
	}

	/**
	 * Open edit smart playlist modal
	 */
	function handleEditSmartPlaylist(playlist: Playlist): void {
		modalActions.openEditSmartPlaylistModal(playlist)
	}

	/**
	 * Open rename playlist modal
	 */
	function handlePlaylistRename(playlist: Playlist): void {
		modalActions.openRenamePlaylistModal(playlist)
	}

	/**
	 * Open delete playlist confirmation modal
	 */
	function handlePlaylistDelete(playlist: Playlist): void {
		const playlists = getPlaylists()
		modalActions.openDeletePlaylistModal(playlist, hasChildren(playlists, playlist.id))
	}

	/**
	 * Move a playlist to a folder (handles conflict detection)
	 */
	async function handlePlaylistMove(playlist: Playlist, folderId: string | null): Promise<void> {
		const playlists = getPlaylists()

		// Check for conflict
		const conflict = findConflictingItem(playlists, playlist, folderId)

		if (conflict) {
			modalActions.openMoveConflictModal(playlist, conflict, folderId)
			return
		}

		// No conflict, proceed with move
		await playlistsStore.move(playlist.id, folderId)
	}

	/**
	 * Handle drag-drop playlist move (handles conflict detection)
	 */
	async function handlePlaylistDragMove(playlistId: string, targetFolderId: string | null): Promise<void> {
		const playlists = getPlaylists()
		const playlist = getPlaylistById(playlists, playlistId)
		if (!playlist) return

		// Check for conflict
		const conflict = findConflictingItem(playlists, playlist, targetFolderId)

		if (conflict) {
			modalActions.openMoveConflictModal(playlist, conflict, targetFolderId)
			return
		}

		// No conflict, proceed with move
		const result = await playlistsStore.move(playlistId, targetFolderId)
		if (result) {
			toastStore.success('Moved successfully')
		}
	}

	/**
	 * Move multiple playlists to a folder (handles conflict detection for each)
	 */
	async function handleBulkPlaylistMove(playlistIds: string[], targetFolderId: string | null): Promise<void> {
		const playlists = getPlaylists()
		for (const id of playlistIds) {
			const playlist = getPlaylistById(playlists, id)
			if (!playlist) continue
			// Skip if already in the target folder
			if (playlist.parent_id === targetFolderId) continue

			const conflict = findConflictingItem(playlists, playlist, targetFolderId)
			if (conflict) {
				modalActions.openMoveConflictModal(playlist, conflict, targetFolderId)
				return
			}

			await playlistsStore.move(id, targetFolderId)
		}
	}

	/**
	 * Import tracks directly into a playlist view
	 */
	async function handlePlaylistViewImport(playlist: Playlist): Promise<void> {
		if (playlist.is_smart) return
		// Open file dialog
		const selected = await withNativeDialog(() =>
			open({
				multiple: true,
				filters: [
					{
						name: 'Audio Files',
						extensions: ['mp3', 'wav', 'aiff', 'aif', 'flac', 'm4a', 'aac'],
					},
				],
			})
		)

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

	return {
		handleLibraryClick,
		handlePlaylistSelect,
		handleCreatePlaylist,
		handleCreateFolder,
		handleCreateSmartPlaylist,
		handleEditSmartPlaylist,
		handlePlaylistRename,
		handlePlaylistDelete,
		handlePlaylistMove,
		handlePlaylistDragMove,
		handleBulkPlaylistMove,
		handlePlaylistViewImport,
	}
}
