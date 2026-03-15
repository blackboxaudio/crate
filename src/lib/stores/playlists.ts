import { writable, derived } from 'svelte/store'
import type {
	ActiveView,
	DiscoveryRelease,
	Playlist,
	SmartRules,
	Track,
	BreadcrumbItem,
	MoveConflictResolution,
	MovePlaylistResult,
} from '$lib/types'
import * as playlistsApi from '$lib/api/playlists'
import { toastStore } from './toast'
import { syncStore } from './sync'

// =============================================================================
// State
// =============================================================================

interface PlaylistsState {
	playlists: Playlist[]
	loading: boolean
	error: string | null
}

const initialState: PlaylistsState = {
	playlists: [],
	loading: false,
	error: null,
}

// =============================================================================
// Store
// =============================================================================

function createPlaylistsStore() {
	const { subscribe, set, update } = writable<PlaylistsState>(initialState)

	return {
		subscribe,

		/**
		 * Load all playlists from both contexts
		 */
		async load() {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const [libraryPlaylists, discoveryPlaylists] = await Promise.all([
					playlistsApi.getPlaylists('library'),
					playlistsApi.getPlaylists('discovery'),
				])
				update((state) => ({
					...state,
					playlists: [...libraryPlaylists, ...discoveryPlaylists],
					loading: false,
				}))
			} catch (error) {
				const errorMessage = error instanceof Error ? error.message : 'Failed to load playlists'
				update((state) => ({
					...state,
					loading: false,
					error: errorMessage,
				}))
				toastStore.error(errorMessage)
			}
		},

		/**
		 * Create a new playlist
		 */
		async createPlaylist(name: string, parentId?: string, context: ActiveView = 'library') {
			try {
				const playlist = await playlistsApi.createPlaylist(name, parentId, context)
				update((state) => ({
					...state,
					playlists: [...state.playlists, playlist],
				}))
				return playlist
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to create playlist',
				}))
				return null
			}
		},

		/**
		 * Create a new folder
		 */
		async createFolder(name: string, parentId?: string, context: ActiveView = 'library') {
			try {
				const folder = await playlistsApi.createFolder(name, parentId, context)
				update((state) => ({
					...state,
					playlists: [...state.playlists, folder],
				}))
				return folder
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to create folder',
				}))
				return null
			}
		},

		/**
		 * Rename a playlist or folder
		 */
		async rename(id: string, name: string) {
			try {
				const updated = await playlistsApi.renamePlaylist(id, name)
				update((state) => ({
					...state,
					playlists: state.playlists.map((p) => (p.id === id ? updated : p)),
				}))

				// Notify sync store about playlist changes (for auto-sync)
				syncStore.notifyPlaylistChanges([id])

				return updated
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to rename',
				}))
				return null
			}
		},

		/**
		 * Delete a playlist or folder
		 */
		async delete(id: string, deleteTracksFromCollection: boolean = false) {
			try {
				await playlistsApi.deletePlaylist(id, deleteTracksFromCollection)
				// Collect all descendant IDs to remove (handles folder with children)
				update((state) => {
					const idsToRemove = new Set<string>()
					idsToRemove.add(id)
					const collectDescendants = (parentId: string) => {
						for (const p of state.playlists) {
							if (p.parent_id === parentId && !idsToRemove.has(p.id)) {
								idsToRemove.add(p.id)
								collectDescendants(p.id)
							}
						}
					}
					collectDescendants(id)
					return {
						...state,
						playlists: state.playlists.filter((p) => !idsToRemove.has(p.id)),
					}
				})
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to delete',
				}))
			}
		},

		/**
		 * Move a playlist to a different folder (simple move, will error on conflict)
		 */
		async move(id: string, parentId: string | null): Promise<MovePlaylistResult | null> {
			try {
				const result = await playlistsApi.movePlaylist(id, parentId)
				update((state) => ({
					...state,
					playlists: state.playlists.map((p) => (p.id === id ? result.playlist : p)),
				}))
				return result
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to move playlist',
				}))
				return null
			}
		},

		/**
		 * Move a playlist with conflict resolution (overwrite or merge)
		 */
		async moveWithResolution(
			id: string,
			parentId: string | null,
			resolution: MoveConflictResolution
		): Promise<MovePlaylistResult | null> {
			try {
				const result = await playlistsApi.movePlaylist(id, parentId, resolution)

				// Reload playlists to get accurate state after complex operations
				await this.load()

				return result
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to move playlist',
				}))
				return null
			}
		},

		/**
		 * Get tracks in a playlist
		 */
		async getPlaylistTracks(playlistId: string): Promise<Track[]> {
			try {
				return await playlistsApi.getPlaylistTracks(playlistId)
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to get playlist tracks',
				}))
				return []
			}
		},

		/**
		 * Add tracks to a playlist
		 */
		async addTracks(playlistId: string, trackIds: string[]) {
			try {
				const updatedPlaylist = await playlistsApi.addToPlaylist(playlistId, trackIds)
				// Update playlist with accurate data from backend
				update((state) => ({
					...state,
					playlists: state.playlists.map((p) => (p.id === playlistId ? updatedPlaylist : p)),
				}))

				// Notify sync store about playlist changes (for auto-sync)
				syncStore.notifyPlaylistChanges([playlistId])
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to add tracks',
				}))
			}
		},

		/**
		 * Remove tracks from a playlist
		 */
		async removeTracks(playlistId: string, trackIds: string[]) {
			try {
				const updatedPlaylist = await playlistsApi.removeFromPlaylist(playlistId, trackIds)
				// Update playlist with accurate data from backend
				update((state) => ({
					...state,
					playlists: state.playlists.map((p) => (p.id === playlistId ? updatedPlaylist : p)),
				}))

				// Notify sync store about playlist changes (for auto-sync)
				syncStore.notifyPlaylistChanges([playlistId])
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to remove tracks',
				}))
			}
		},

		/**
		 * Reorder tracks in a playlist
		 */
		async reorderTracks(playlistId: string, trackIds: string[]) {
			try {
				await playlistsApi.reorderPlaylist(playlistId, trackIds)

				// Notify sync store about playlist changes (for auto-sync)
				syncStore.notifyPlaylistChanges([playlistId])
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to reorder tracks',
				}))
			}
		},

		/**
		 * Add discovery releases to a playlist
		 */
		async addReleases(playlistId: string, releaseIds: string[]) {
			try {
				const updatedPlaylist = await playlistsApi.addReleasesToPlaylist(playlistId, releaseIds)
				update((state) => ({
					...state,
					playlists: state.playlists.map((p) => (p.id === playlistId ? updatedPlaylist : p)),
				}))
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to add releases',
				}))
			}
		},

		/**
		 * Remove discovery releases from a playlist
		 */
		async removeReleases(playlistId: string, releaseIds: string[]) {
			try {
				const updatedPlaylist = await playlistsApi.removeReleasesFromPlaylist(playlistId, releaseIds)
				update((state) => ({
					...state,
					playlists: state.playlists.map((p) => (p.id === playlistId ? updatedPlaylist : p)),
				}))
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to remove releases',
				}))
			}
		},

		/**
		 * Get discovery releases in a playlist
		 */
		async getPlaylistReleases(playlistId: string): Promise<DiscoveryRelease[]> {
			try {
				return await playlistsApi.getPlaylistReleases(playlistId)
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to get playlist releases',
				}))
				return []
			}
		},

		/**
		 * Create a new smart playlist
		 */
		async createSmartPlaylist(
			name: string,
			smartRules: SmartRules,
			parentId?: string,
			context: ActiveView = 'library'
		) {
			try {
				const playlist = await playlistsApi.createSmartPlaylist(name, smartRules, parentId, context)
				update((state) => ({
					...state,
					playlists: [...state.playlists, playlist],
				}))
				return playlist
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to create smart playlist',
				}))
				return null
			}
		},

		/**
		 * Update smart rules on an existing smart playlist
		 */
		async updateSmartRules(id: string, smartRules: SmartRules) {
			try {
				const updated = await playlistsApi.updateSmartRules(id, smartRules)
				update((state) => ({
					...state,
					playlists: state.playlists.map((p) => (p.id === id ? updated : p)),
				}))
				return updated
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to update smart rules',
				}))
				return null
			}
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const playlistsStore = createPlaylistsStore()

// =============================================================================
// Derived Stores
// =============================================================================

/**
 * Root level playlists (no parent)
 */
export const rootPlaylists = derived(playlistsStore, ($playlists) =>
	$playlists.playlists.filter((p) => p.parent_id === null)
)

/**
 * Get children of a playlist/folder
 */
export function getPlaylistChildren(playlists: Playlist[], parentId: string): Playlist[] {
	return playlists.filter((p) => p.parent_id === parentId)
}

/**
 * Build playlist tree structure
 */
export interface PlaylistTreeNode {
	playlist: Playlist
	children: PlaylistTreeNode[]
}

export function buildPlaylistTree(playlists: Playlist[]): PlaylistTreeNode[] {
	// Sort items: folders first, then alphabetically by name
	const sortItems = (items: Playlist[]) =>
		[...items].sort((a, b) => {
			if (a.is_folder !== b.is_folder) {
				return a.is_folder ? -1 : 1
			}
			return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
		})

	const rootItems = sortItems(playlists.filter((p) => p.parent_id === null))

	function buildChildren(parentId: string): PlaylistTreeNode[] {
		return sortItems(playlists.filter((p) => p.parent_id === parentId)).map((playlist) => ({
			playlist,
			children: buildChildren(playlist.id),
		}))
	}

	return rootItems.map((playlist) => ({
		playlist,
		children: buildChildren(playlist.id),
	}))
}

// =============================================================================
// Breadcrumb Helpers
// =============================================================================

/**
 * Get the full path from root to a playlist/folder by traversing parent_id chain
 * Returns array from root to target (inclusive)
 */
export function getPlaylistPath(playlists: Playlist[], targetId: string): Playlist[] {
	const path: Playlist[] = []
	let current = playlists.find((p) => p.id === targetId)

	while (current) {
		path.unshift(current)
		if (current.parent_id) {
			current = playlists.find((p) => p.id === current!.parent_id)
		} else {
			break
		}
	}

	return path
}

/**
 * Build breadcrumb items for the current navigation state
 */
export function buildBreadcrumbItems(
	playlists: Playlist[],
	selectedFolderId: string | null,
	selectedPlaylistId: string | null,
	trackCount?: number,
	childCount?: number,
	activeView: ActiveView = 'library',
	t: (key: string) => string = (key) => key
): BreadcrumbItem[] {
	const items: BreadcrumbItem[] = []

	// Determine the target ID
	const targetId = selectedPlaylistId || selectedFolderId

	if (!targetId) {
		// At root - no breadcrumbs needed
		return []
	}

	// Add root breadcrumb based on active view
	const rootLabel = activeView === 'discovery' ? t('nav.discovery') : t('nav.library')
	items.push({
		id: null,
		name: rootLabel,
		type: activeView === 'discovery' ? 'discovery' : 'library',
	})

	// Get path from root to target
	const path = getPlaylistPath(playlists, targetId)

	// Add each item in the path
	path.forEach((playlist, index) => {
		const isLast = index === path.length - 1
		const item: BreadcrumbItem = {
			id: playlist.id,
			name: playlist.name,
			type: playlist.is_folder ? 'folder' : playlist.is_smart ? 'smart_playlist' : 'playlist',
			playlist,
		}

		// Add count info for the last item
		if (isLast) {
			if (playlist.is_folder) {
				item.count = childCount
				item.countLabel = childCount === 1 ? t('library.item') : t('library.items')
			} else {
				const count = trackCount ?? playlist.track_count
				item.count = count
				if (activeView === 'discovery') {
					item.countLabel = count === 1 ? t('discovery.release') : t('discovery.releases')
				} else {
					item.countLabel = count === 1 ? t('library.track') : t('library.tracks')
				}
			}
		}

		items.push(item)
	})

	return items
}
