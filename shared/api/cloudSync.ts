import { invoke } from '@tauri-apps/api/core'
import type { CloudSyncStatus, CloudDeviceRecord, LibraryRoot } from '../types'

// Auth + sync

export async function signIn(providerId: string): Promise<CloudSyncStatus> {
	return invoke<CloudSyncStatus>('sign_in', { providerId })
}

// Native mobile sign-in (two-step): `begin_sign_in` returns the consent URL + callback scheme
// for the platform auth session (ASWebAuthenticationSession / Custom Tabs); the frontend then
// hands the resulting `code`/`state` back via `complete_sign_in`.

export async function beginSignIn(providerId: string): Promise<{ authUrl: string; callbackScheme: string }> {
	return invoke<{ authUrl: string; callbackScheme: string }>('begin_sign_in', { providerId })
}

export async function completeSignIn(code: string, oauthState: string): Promise<CloudSyncStatus> {
	return invoke<CloudSyncStatus>('complete_sign_in', { code, oauthState })
}

export async function signOut(): Promise<void> {
	return invoke<void>('sign_out')
}

export async function getSyncStatus(): Promise<CloudSyncStatus> {
	return invoke<CloudSyncStatus>('get_sync_status')
}

export async function syncNow(): Promise<void> {
	return invoke<void>('sync_now')
}

export async function pullNow(): Promise<void> {
	return invoke<void>('pull_now')
}

// Devices

export async function listDevices(): Promise<CloudDeviceRecord[]> {
	return invoke<CloudDeviceRecord[]>('list_devices')
}

export async function renameDevice(name: string): Promise<void> {
	return invoke<void>('rename_device', { name })
}

export async function revokeDevice(deviceId: string): Promise<void> {
	return invoke<void>('revoke_device', { deviceId })
}

export async function deleteCloudVault(): Promise<void> {
	return invoke<void>('delete_cloud_vault')
}

// Library roots

export async function listLibraryRoots(): Promise<LibraryRoot[]> {
	return invoke<LibraryRoot[]>('list_library_roots')
}

export async function createLibraryRoot(name: string): Promise<string> {
	return invoke<string>('create_library_root', { name })
}

export async function renameLibraryRoot(id: string, name: string): Promise<void> {
	return invoke<void>('rename_library_root', { id, name })
}

export async function removeLibraryRoot(id: string): Promise<void> {
	return invoke<void>('remove_library_root', { id })
}

export async function setLibraryRootMapping(rootId: string, localPath: string): Promise<void> {
	return invoke<void>('set_library_root_mapping', { rootId, localPath })
}

export async function suggestLibraryRoots(): Promise<string[]> {
	return invoke<string[]>('suggest_library_roots')
}

// Track location

export async function locateTrack(trackId: string, localPath: string): Promise<void> {
	return invoke<void>('locate_track', { trackId, localPath })
}
