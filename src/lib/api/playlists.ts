import { invoke } from '@tauri-apps/api/core'
import type { Playlist, Track } from '$lib/types'

/**
 * Get all playlists
 */
export async function getPlaylists(): Promise<Playlist[]> {
	return invoke<Playlist[]>('get_playlists')
}

/**
 * Create a new playlist
 */
export async function createPlaylist(name: string, parentId?: string): Promise<Playlist> {
	return invoke<Playlist>('create_playlist', {
		name,
		parentId: parentId ?? null,
	})
}

/**
 * Create a new playlist folder
 */
export async function createFolder(name: string, parentId?: string): Promise<Playlist> {
	return invoke<Playlist>('create_folder', {
		name,
		parentId: parentId ?? null,
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
export async function deletePlaylist(id: string): Promise<void> {
	return invoke<void>('delete_playlist', { id })
}

/**
 * Move a playlist to a different folder
 */
export async function movePlaylist(id: string, parentId: string | null): Promise<Playlist> {
	return invoke<Playlist>('move_playlist', { id, parentId })
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
export async function addToPlaylist(playlistId: string, trackIds: string[]): Promise<void> {
	return invoke<void>('add_to_playlist', { playlistId, trackIds })
}

/**
 * Remove tracks from a playlist
 */
export async function removeFromPlaylist(playlistId: string, trackIds: string[]): Promise<void> {
	return invoke<void>('remove_from_playlist', { playlistId, trackIds })
}

/**
 * Reorder tracks in a playlist
 */
export async function reorderPlaylist(playlistId: string, trackIds: string[]): Promise<void> {
	return invoke<void>('reorder_playlist', { playlistId, trackIds })
}
