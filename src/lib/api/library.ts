import { invoke } from '@tauri-apps/api/core'
import type { ImportResult, Track, TrackFilter, TrackUpdate } from '$lib/types'

/**
 * Import tracks from file paths into the library
 */
export async function importTracks(paths: string[]): Promise<ImportResult> {
	return invoke<ImportResult>('import_tracks', { paths })
}

/**
 * Get all tracks with optional filtering
 */
export async function getTracks(filter?: TrackFilter): Promise<Track[]> {
	return invoke<Track[]>('get_tracks', { filter: filter ?? null })
}

/**
 * Get a single track by ID
 */
export async function getTrack(id: string): Promise<Track> {
	return invoke<Track>('get_track', { id })
}

/**
 * Update track metadata
 */
export async function updateTrack(id: string, update: TrackUpdate): Promise<Track> {
	return invoke<Track>('update_track', { id, update })
}

/**
 * Delete tracks by IDs
 */
export async function deleteTracks(ids: string[]): Promise<void> {
	return invoke<void>('delete_tracks', { ids })
}

/**
 * Search tracks by query string
 */
export async function searchTracks(query: string): Promise<Track[]> {
	return invoke<Track[]>('search_tracks', { query })
}
