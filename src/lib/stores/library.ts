import { writable, derived } from 'svelte/store'
import type { Track, TrackFilter, SortConfig, ImportResult } from '$lib/types'
import { sortTracks } from '$lib/utils/sorting'
import * as libraryApi from '$lib/api/library'
import * as playlistsApi from '$lib/api/playlists'
import { toastStore } from './toast'

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
				update((state) => ({
					...state,
					loading: false,
					error: error instanceof Error ? error.message : 'Failed to load tracks',
				}))
			}
		},

		/**
		 * Import tracks from file paths
		 */
		async importTracks(paths: string[]): Promise<ImportResult> {
			update((state) => ({ ...state, loading: true, error: null }))

			try {
				const result = await libraryApi.importTracks(paths)

				update((state) => ({
					...state,
					tracks: [...result.tracks, ...state.tracks],
					loading: false,
				}))

				// Show appropriate toast based on result
				const successCount = result.tracks.length
				const failedCount = result.failed_count

				if (successCount > 0 && failedCount === 0) {
					// All succeeded
					toastStore.success(successCount === 1 ? '1 track imported' : `${successCount} tracks imported`)
				} else if (successCount > 0 && failedCount > 0) {
					// Partial success
					toastStore.warning(`${successCount} track${successCount !== 1 ? 's' : ''} imported, ${failedCount} failed`)
				} else if (successCount === 0 && failedCount > 0) {
					// All failed
					const firstError = result.errors[0] || 'Unknown error'
					toastStore.error(`Failed to import tracks: ${firstError}`)
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
				return { tracks: [], failed_count: paths.length, errors: [errorMessage] }
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
	// If a playlist is selected, show playlist tracks
	if ($library.selectedPlaylistId) {
		let tracks = $library.playlistTracks

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
	}

	// Otherwise, show all library tracks (filtered and sorted)
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
