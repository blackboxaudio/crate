import { invoke } from '@tauri-apps/api/core'
import type { DiscoveryRelease, MoveConflictResolution, MovePlaylistResult, Playlist, Track } from '$lib/types'

/**
 * Get all playlists for a given context
 */
export async function getPlaylists(context: string = 'library'): Promise<Playlist[]> {
	return invoke<Playlist[]>('get_playlists', { context })
}

/**
 * Create a new playlist
 */
export async function createPlaylist(name: string, parentId?: string, context: string = 'library'): Promise<Playlist> {
	return invoke<Playlist>('create_playlist', {
		name,
		parentId: parentId ?? null,
		context,
	})
}

/**
 * Create a new playlist folder
 */
export async function createFolder(name: string, parentId?: string, context: string = 'library'): Promise<Playlist> {
	return invoke<Playlist>('create_folder', {
		name,
		parentId: parentId ?? null,
		context,
	})
}

/**
 * Rename a playlist or folder
 */
export async function renamePlaylist(id: string, name: string): Promise<Playlist> {
	return invoke<Playlist>('rename_playlist', { id, name })
}

/**
 * Delete a playlist or folder
 */
export async function deletePlaylist(id: string, deleteTracksFromCollection: boolean = false): Promise<void> {
	return invoke<void>('delete_playlist', { id, deleteTracksFromCollection })
}

/**
 * Move a playlist to a different folder
 */
export async function movePlaylist(
	id: string,
	parentId: string | null,
	resolution?: MoveConflictResolution
): Promise<MovePlaylistResult> {
	return invoke<MovePlaylistResult>('move_playlist', { id, parentId, resolution })
}

/**
 * Get tracks in a playlist
 */
export async function getPlaylistTracks(playlistId: string): Promise<Track[]> {
	return invoke<Track[]>('get_playlist_tracks', { playlistId })
}

/**
 * Add tracks to a playlist
 */
export async function addToPlaylist(playlistId: string, trackIds: string[]): Promise<Playlist> {
	return invoke<Playlist>('add_to_playlist', { playlistId, trackIds })
}

/**
 * Remove tracks from a playlist
 */
export async function removeFromPlaylist(playlistId: string, trackIds: string[]): Promise<Playlist> {
	return invoke<Playlist>('remove_from_playlist', { playlistId, trackIds })
}

/**
 * Reorder tracks in a playlist
 */
export async function reorderPlaylist(playlistId: string, trackIds: string[]): Promise<void> {
	return invoke<void>('reorder_playlist', { playlistId, trackIds })
}

/**
 * Add discovery releases to a playlist
 */
export async function addReleasesToPlaylist(playlistId: string, releaseIds: string[]): Promise<Playlist> {
	return invoke<Playlist>('add_releases_to_playlist', { playlistId, releaseIds })
}

/**
 * Remove discovery releases from a playlist
 */
export async function removeReleasesFromPlaylist(playlistId: string, releaseIds: string[]): Promise<Playlist> {
	return invoke<Playlist>('remove_releases_from_playlist', { playlistId, releaseIds })
}

/**
 * Get discovery releases in a playlist
 */
export async function getPlaylistReleases(playlistId: string): Promise<DiscoveryRelease[]> {
	return invoke<DiscoveryRelease[]>('get_playlist_releases', { playlistId })
}
