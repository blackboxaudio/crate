import { writable, derived } from 'svelte/store';
import type { Playlist, Track } from '$lib/types';
import * as playlistsApi from '$lib/api/playlists';

// =============================================================================
// State
// =============================================================================

interface PlaylistsState {
  playlists: Playlist[];
  loading: boolean;
  error: string | null;
}

const initialState: PlaylistsState = {
  playlists: [],
  loading: false,
  error: null
};

// =============================================================================
// Store
// =============================================================================

function createPlaylistsStore() {
  const { subscribe, set, update } = writable<PlaylistsState>(initialState);

  return {
    subscribe,

    /**
     * Load all playlists
     */
    async load() {
      update((state) => ({ ...state, loading: true, error: null }));

      try {
        const playlists = await playlistsApi.getPlaylists();
        update((state) => ({
          ...state,
          playlists,
          loading: false
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load playlists'
        }));
      }
    },

    /**
     * Create a new playlist
     */
    async createPlaylist(name: string, parentId?: string) {
      try {
        const playlist = await playlistsApi.createPlaylist(name, parentId);
        update((state) => ({
          ...state,
          playlists: [...state.playlists, playlist]
        }));
        return playlist;
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to create playlist'
        }));
        return null;
      }
    },

    /**
     * Create a new folder
     */
    async createFolder(name: string, parentId?: string) {
      try {
        const folder = await playlistsApi.createFolder(name, parentId);
        update((state) => ({
          ...state,
          playlists: [...state.playlists, folder]
        }));
        return folder;
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to create folder'
        }));
        return null;
      }
    },

    /**
     * Rename a playlist or folder
     */
    async rename(id: string, name: string) {
      try {
        const updated = await playlistsApi.renamePlaylist(id, name);
        update((state) => ({
          ...state,
          playlists: state.playlists.map((p) => (p.id === id ? updated : p))
        }));
        return updated;
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to rename'
        }));
        return null;
      }
    },

    /**
     * Delete a playlist or folder
     */
    async delete(id: string) {
      try {
        await playlistsApi.deletePlaylist(id);
        update((state) => ({
          ...state,
          playlists: state.playlists.filter((p) => p.id !== id)
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to delete'
        }));
      }
    },

    /**
     * Get tracks in a playlist
     */
    async getPlaylistTracks(playlistId: string): Promise<Track[]> {
      try {
        return await playlistsApi.getPlaylistTracks(playlistId);
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to get playlist tracks'
        }));
        return [];
      }
    },

    /**
     * Add tracks to a playlist
     */
    async addTracks(playlistId: string, trackIds: string[]) {
      try {
        await playlistsApi.addToPlaylist(playlistId, trackIds);
        // Update track count
        update((state) => ({
          ...state,
          playlists: state.playlists.map((p) => {
            if (p.id === playlistId) {
              return { ...p, track_count: p.track_count + trackIds.length };
            }
            return p;
          })
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to add tracks'
        }));
      }
    },

    /**
     * Remove tracks from a playlist
     */
    async removeTracks(playlistId: string, trackIds: string[]) {
      try {
        await playlistsApi.removeFromPlaylist(playlistId, trackIds);
        // Update track count
        update((state) => ({
          ...state,
          playlists: state.playlists.map((p) => {
            if (p.id === playlistId) {
              return { ...p, track_count: Math.max(0, p.track_count - trackIds.length) };
            }
            return p;
          })
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to remove tracks'
        }));
      }
    },

    /**
     * Reorder tracks in a playlist
     */
    async reorderTracks(playlistId: string, trackIds: string[]) {
      try {
        await playlistsApi.reorderPlaylist(playlistId, trackIds);
      } catch (error) {
        update((state) => ({
          ...state,
          error: error instanceof Error ? error.message : 'Failed to reorder tracks'
        }));
      }
    },

    /**
     * Reset store to initial state
     */
    reset() {
      set(initialState);
    }
  };
}

export const playlistsStore = createPlaylistsStore();

// =============================================================================
// Derived Stores
// =============================================================================

/**
 * Root level playlists (no parent)
 */
export const rootPlaylists = derived(playlistsStore, ($playlists) =>
  $playlists.playlists.filter((p) => p.parent_id === null)
);

/**
 * Get children of a playlist/folder
 */
export function getPlaylistChildren(
  playlists: Playlist[],
  parentId: string
): Playlist[] {
  return playlists.filter((p) => p.parent_id === parentId);
}

/**
 * Build playlist tree structure
 */
export interface PlaylistTreeNode {
  playlist: Playlist;
  children: PlaylistTreeNode[];
}

export function buildPlaylistTree(playlists: Playlist[]): PlaylistTreeNode[] {
  const rootItems = playlists.filter((p) => p.parent_id === null);

  function buildChildren(parentId: string): PlaylistTreeNode[] {
    return playlists
      .filter((p) => p.parent_id === parentId)
      .map((playlist) => ({
        playlist,
        children: buildChildren(playlist.id)
      }));
  }

  return rootItems.map((playlist) => ({
    playlist,
    children: buildChildren(playlist.id)
  }));
}
