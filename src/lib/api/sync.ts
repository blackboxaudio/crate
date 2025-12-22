import { invoke } from '@tauri-apps/api/core'
import type { SyncResult, DeviceInfo } from '$lib/types'

/**
 * Sync playlists to a USB device (incremental sync)
 */
export async function syncDevice(
	deviceId: string,
	deviceName: string,
	mountPoint: string,
	playlistIds: string[]
): Promise<SyncResult> {
	return invoke('sync_device', { deviceId, deviceName, mountPoint, playlistIds })
}

/**
 * Get playlists with pending changes for a device
 */
export async function getPendingSyncPlaylists(deviceId: string): Promise<string[]> {
	return invoke('get_pending_sync_playlists', { deviceId })
}

/**
 * Check if a device has any pending changes
 */
export async function hasPendingSyncChanges(deviceId: string): Promise<boolean> {
	return invoke('has_pending_sync_changes', { deviceId })
}

/**
 * Check if a sync is currently in progress
 */
export async function isSyncing(): Promise<boolean> {
	return invoke('is_syncing')
}

/**
 * Cancel the current sync operation
 */
export async function cancelSync(): Promise<void> {
	return invoke('cancel_sync')
}

/**
 * Get playlists containing a specific track
 */
export async function getPlaylistsContainingTrack(trackId: string): Promise<string[]> {
	return invoke('get_playlists_containing_track', { trackId })
}

/**
 * Get playlists containing any of the specified tracks
 */
export async function getPlaylistsContainingTracks(trackIds: string[]): Promise<string[]> {
	return invoke('get_playlists_containing_tracks', { trackIds })
}

/**
 * Get devices that have a specific playlist exported with sync enabled
 */
export async function getDevicesForPlaylist(playlistId: string): Promise<DeviceInfo[]> {
	return invoke('get_devices_for_playlist', { playlistId })
}

/**
 * Get devices that have any of the specified playlists exported with sync enabled
 */
export async function getDevicesForPlaylists(playlistIds: string[]): Promise<DeviceInfo[]> {
	return invoke('get_devices_for_playlists', { playlistIds })
}
