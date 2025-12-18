import { writable, derived } from 'svelte/store';
import type { Track, TrackFilter, SortConfig } from '$lib/types';
import { sortTracks } from '$lib/utils/sorting';
import * as libraryApi from '$lib/api/library';

// =============================================================================
// State
// =============================================================================

interface LibraryState {
  tracks: Track[];
  loading: boolean;
  error: string | null;
  filter: TrackFilter;
  sort: SortConfig;
}

const initialState: LibraryState = {
  tracks: [],
  loading: false,
  error: null,
  filter: {},
  sort: {
    field: 'date_added',
    direction: 'desc'
  }
};

// =============================================================================
// Store
// =============================================================================

function createLibraryStore() {
  const { subscribe, set, update } = writable<LibraryState>(initialState);

  return {
    subscribe,

    /**
     * Load all tracks from the backend
     */
    async loadTracks(filter?: TrackFilter) {
      update((state) => ({ ...state, loading: true, error: null }));

      try {
        const tracks = await libraryApi.getTracks(filter);
        update((state) => ({
          ...state,
          tracks,
          loading: false,
          filter: filter ?? {}
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load tracks'
        }));
      }
    },

    /**
     * Import tracks from file paths
     */
    async importTracks(paths: string[]) {
      update((state) => ({ ...state, loading: true, error: null }));

      try {
        const imported = await libraryApi.importTracks(paths);
        update((state) => ({
          ...state,
          tracks: [...imported, ...state.tracks],
          loading: false
        }));
        return imported;
      } catch (error) {
        update((state) => ({
          ...state,
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to import tracks'
        }));
        return [];
      }
    },

    /**
     * Delete tracks by IDs
     */
    async deleteTracks(ids: string[]) {
      try {
        await libraryApi.deleteTracks(ids);
        update((state) => ({
          ...state,
          tracks: state.tracks.filter((t) => !ids.includes(t.id))
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to delete tracks'
        }));
      }
    },

    /**
     * Update sort configuration
     */
    setSort(sort: SortConfig) {
      update((state) => ({ ...state, sort }));
    },

    /**
     * Update filter
     */
    setFilter(filter: TrackFilter) {
      update((state) => ({ ...state, filter }));
    },

    /**
     * Update search query
     */
    setSearch(search: string) {
      update((state) => ({
        ...state,
        filter: { ...state.filter, search: search || undefined }
      }));
    },

    /**
     * Clear all filters
     */
    clearFilters() {
      update((state) => ({ ...state, filter: {} }));
    },

    /**
     * Reset store to initial state
     */
    reset() {
      set(initialState);
    }
  };
}

export const libraryStore = createLibraryStore();

// =============================================================================
// Derived Stores
// =============================================================================

/**
 * Filtered and sorted tracks
 */
export const sortedTracks = derived(libraryStore, ($library) => {
  let tracks = $library.tracks;

  // Apply client-side search filter if needed
  if ($library.filter.search) {
    const search = $library.filter.search.toLowerCase();
    tracks = tracks.filter(
      (t) =>
        t.title?.toLowerCase().includes(search) ||
        t.artist?.toLowerCase().includes(search) ||
        t.album?.toLowerCase().includes(search)
    );
  }

  // Apply sorting
  return sortTracks(tracks, $library.sort);
});

/**
 * Track count
 */
export const trackCount = derived(sortedTracks, ($tracks) => $tracks.length);

/**
 * Loading state
 */
export const isLoading = derived(libraryStore, ($library) => $library.loading);
