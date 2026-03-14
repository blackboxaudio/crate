import { writable, derived, get } from 'svelte/store'
import type { Track, TrackColor, TrackFilter, SortConfig, ImportResultWithDuplicates } from '$lib/types'
import { sortTracks } from '$lib/utils/sorting'
import * as libraryApi from '$lib/api/library'
import * as playlistsApi from '$lib/api/playlists'
import { toastStore } from './toast'
import { autoAnalyzeOnImport } from './settings'
import { analysisStore } from './analysis'
import { syncStore } from './sync'

// =============================================================================
// State
// =============================================================================

interface LibraryState {
	tracks: Track[]
	loading: boolean
	error: string | null
	filter: TrackFilter
	sort: SortConfig
	playlistTracks: Track[]
	selectedPlaylistId: string | null
}

const initialState: LibraryState = {
	tracks: [],
	loading: false,
	error: null,
	filter: {},
	sort: {
		field: 'date_added',
		direction: 'desc',
	},
	playlistTracks: [],
	selectedPlaylistId: null,
}

// =============================================================================
// Store
// =============================================================================

function createLibraryStore() {
	const { subscribe, set, update } = writable<LibraryState>(initialState)

	return {
		subscribe,

		/**
		 * Load all tracks from the backend
		 */
		async loadTracks(filter?: TrackFilter) {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const tracks = await libraryApi.getTracks(filter)
				update((state) => ({
					...state,
					tracks,
					loading: false,
					filter: filter ?? {},
				}))
			} catch (error) {
				const errorMessage = error instanceof Error ? error.message : 'Failed to load tracks'
				update((state) => ({
					...state,
					loading: false,
					error: errorMessage,
				}))
				toastStore.error(errorMessage)
			}
		},

		/**
		 * Import tracks from file paths with duplicate detection
		 * Returns duplicates for the caller to handle via modal
		 */
		async importTracks(paths: string[]): Promise<ImportResultWithDuplicates> {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const result = await libraryApi.importTracksWithDuplicates(paths)

				// Add successfully imported tracks to state immediately
				if (result.tracks.length > 0) {
					update((state) => ({
						...state,
						tracks: [...result.tracks, ...state.tracks],
					}))

					// Auto-analyze imported tracks if enabled
					if (get(autoAnalyzeOnImport)) {
						const trackIds = result.tracks.map((t) => t.id)
						// Run analysis asynchronously, don't await to avoid blocking import UI
						analysisStore.analyzeTracks(trackIds).catch((error) => {
							console.error('Auto-analysis failed:', error)
						})
					}
				}

				update((state) => ({ ...state, loading: false }))

				// Show toast for non-duplicate results only if no duplicates
				// (duplicates will be handled by the modal, toast shown after resolution)
				if (result.duplicates.length === 0) {
					const successCount = result.tracks.length
					const failedCount = result.failed_count

					if (successCount > 0 && failedCount === 0) {
						toastStore.success(successCount === 1 ? '1 track imported' : `${successCount} tracks imported`)
					} else if (successCount > 0 && failedCount > 0) {
						toastStore.warning(`${successCount} track${successCount !== 1 ? 's' : ''} imported, ${failedCount} failed`)
					} else if (successCount === 0 && failedCount > 0) {
						const firstError = result.errors[0] || 'Unknown error'
						toastStore.error(`Failed to import tracks: ${firstError}`)
					}
				}

				return result
			} catch (error) {
				const errorMessage = error instanceof Error ? error.message : 'Failed to import tracks'
				update((state) => ({
					...state,
					loading: false,
					error: errorMessage,
				}))
				toastStore.error(errorMessage)
				return { tracks: [], failed_count: paths.length, errors: [errorMessage], duplicates: [] }
			}
		},

		/**
		 * Delete tracks by IDs
		 */
		async deleteTracks(ids: string[]) {
			try {
				await libraryApi.deleteTracks(ids)
				update((state) => ({
					...state,
					tracks: state.tracks.filter((t) => !ids.includes(t.id)),
					playlistTracks: state.playlistTracks.filter((t) => !ids.includes(t.id)),
				}))
			} catch (error) {
				update((state) => ({
					...state,
					error: error instanceof Error ? error.message : 'Failed to delete tracks',
				}))
			}
		},

		/**
		 * Update sort configuration
		 */
		setSort(sort: SortConfig) {
			update((state) => ({ ...state, sort }))
		},

		/**
		 * Update filter
		 */
		setFilter(filter: TrackFilter) {
			update((state) => ({ ...state, filter }))
		},

		/**
		 * Update search query
		 */
		setSearch(search: string) {
			update((state) => ({
				...state,
				filter: { ...state.filter, search: search || undefined },
			}))
		},

		/**
		 * Clear all filters
		 */
		clearFilters() {
			update((state) => ({ ...state, filter: {} }))
		},

		/**
		 * Set color for tracks
		 */
		async setTrackColors(trackIds: string[], color: TrackColor | null) {
			try {
				await libraryApi.setTrackColors(trackIds, color)

				// Update local state
				update((state) => ({
					...state,
					tracks: state.tracks.map((t) => (trackIds.includes(t.id) ? { ...t, color } : t)),
					playlistTracks: state.playlistTracks.map((t) => (trackIds.includes(t.id) ? { ...t, color } : t)),
				}))

				// Notify sync store about track changes (for auto-sync)
				syncStore.notifyTrackChanges(trackIds)
			} catch (error) {
				toastStore.error('Failed to set track color')
			}
		},

		/**
		 * Load tracks for a specific playlist
		 */
		async loadPlaylistTracks(playlistId: string) {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const tracks = await playlistsApi.getPlaylistTracks(playlistId)
				update((state) => ({
					...state,
					playlistTracks: tracks,
					selectedPlaylistId: playlistId,
					loading: false,
				}))
			} catch (error) {
				update((state) => ({
					...state,
					loading: false,
					error: error instanceof Error ? error.message : 'Failed to load playlist tracks',
				}))
			}
		},

		/**
		 * Load tracks for a smart playlist (evaluates rules dynamically)
		 */
		async loadSmartPlaylistTracks(playlistId: string) {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const tracks = await playlistsApi.getSmartPlaylistTracks(playlistId)
				update((state) => ({
					...state,
					playlistTracks: tracks,
					selectedPlaylistId: playlistId,
					loading: false,
				}))
			} catch (error) {
				update((state) => ({
					...state,
					loading: false,
					error: error instanceof Error ? error.message : 'Failed to load smart playlist tracks',
				}))
			}
		},

		/**
		 * Clear playlist tracks and return to library view
		 */
		clearPlaylistTracks() {
			update((state) => ({
				...state,
				playlistTracks: [],
				selectedPlaylistId: null,
			}))
		},

		/**
		 * Add tracks to state without showing toast (for custom toast handling)
		 */
		addTracksToState(tracks: Track[]) {
			update((state) => ({
				...state,
				tracks: [...tracks, ...state.tracks],
			}))
		},

		/**
		 * Update tracks in state (for bulk edits, artwork changes, etc.)
		 */
		updateTracksInState(updatedTracks: Track[]) {
			const updateMap = new Map(updatedTracks.map((t) => [t.id, t]))
			update((state) => ({
				...state,
				tracks: state.tracks.map((t) => updateMap.get(t.id) ?? t),
				playlistTracks: state.playlistTracks.map((t) => updateMap.get(t.id) ?? t),
			}))
		},

		/**
		 * Remove tracks from state by ID
		 */
		removeTracksFromState(ids: string[]) {
			const idSet = new Set(ids)
			update((state) => ({
				...state,
				tracks: state.tracks.filter((t) => !idSet.has(t.id)),
				playlistTracks: state.playlistTracks.filter((t) => !idSet.has(t.id)),
			}))
		},

		/**
		 * Update category_id for a tag across all tracks and playlist tracks
		 */
		updateTagCategory(tagId: string, newCategoryId: string) {
			update((state) => ({
				...state,
				tracks: state.tracks.map((t) => ({
					...t,
					tags: t.tags.map((tag) => (tag.id === tagId ? { ...tag, category_id: newCategoryId } : tag)),
				})),
				playlistTracks: state.playlistTracks.map((t) => ({
					...t,
					tags: t.tags.map((tag) => (tag.id === tagId ? { ...tag, category_id: newCategoryId } : tag)),
				})),
			}))
		},

		/**
		 * Reset store to initial state
		 */
		reset() {
			set(initialState)
		},
	}
}

export const libraryStore = createLibraryStore()

// =============================================================================
// Derived Stores
// =============================================================================

/**
 * Filtered and sorted tracks
 */
export const sortedTracks = derived(libraryStore, ($library) => {
	let tracks = $library.tracks

	// Apply client-side search filter if needed
	if ($library.filter.search) {
		const search = $library.filter.search.toLowerCase()
		tracks = tracks.filter(
			(t) =>
				t.title?.toLowerCase().includes(search) ||
				t.artist?.toLowerCase().includes(search) ||
				t.album?.toLowerCase().includes(search)
		)
	}

	// Apply sorting
	return sortTracks(tracks, $library.sort)
})

/**
 * Track count
 */
export const trackCount = derived(sortedTracks, ($tracks) => $tracks.length)

/**
 * Loading state
 */
export const isLoading = derived(libraryStore, ($library) => $library.loading)

/**
 * Displayed tracks - shows playlist tracks when a playlist is selected,
 * otherwise shows the full library (filtered and sorted)
 */
export const displayedTracks = derived(libraryStore, ($library) => {
	// Check if tag filters are active
	const hasTagFilter = $library.filter.tag_ids && $library.filter.tag_ids.length > 0

	// Determine which tracks to use:
	// - If tag filters are active, use $library.tracks (contains filtered results from backend)
	// - If playlist is selected without tag filters, use playlistTracks
	// - Otherwise, use library tracks
	let tracks: Track[]
	if (hasTagFilter) {
		// Tag filters active: tracks contains filtered results (library or playlist)
		tracks = $library.tracks
	} else if ($library.selectedPlaylistId) {
		// Playlist selected without tag filters: use playlistTracks
		tracks = $library.playlistTracks
	} else {
		// Library view: use tracks
		tracks = $library.tracks
	}

	// Apply client-side search filter if needed
	if ($library.filter.search) {
		const search = $library.filter.search.toLowerCase()
		tracks = tracks.filter(
			(t) =>
				t.title?.toLowerCase().includes(search) ||
				t.artist?.toLowerCase().includes(search) ||
				t.album?.toLowerCase().includes(search)
		)
	}

	// Apply sorting
	return sortTracks(tracks, $library.sort)
})
