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

/** Human-readable sync diagnostics (status header + sync-log tail) for copy-to-clipboard. */
export async function getSyncDiagnostics(): Promise<string> {
	return invoke<string>('get_sync_diagnostics')
}

export async function syncNow(): Promise<void> {
	return invoke<void>('sync_now')
}

export async function pullNow(): Promise<void> {
	return invoke<void>('pull_now')
}

/** One-shot foreground sync (mobile): pull, then push if there are local edits. No-op when signed out. */
export async function syncForeground(): Promise<void> {
	return invoke<void>('sync_foreground')
}

/**
 * Arm opportunistic background sync (mobile: iOS BGTaskScheduler / Android WorkManager). Call after
 * sign-in and on launch when already signed in. The command is mobile-only; on desktop the invoke
 * rejects (unknown command), so callers swallow the error.
 */
export async function scheduleBackgroundSync(): Promise<void> {
	return invoke<void>('schedule_background_sync')
}

/** Cancel any scheduled background sync (mobile; called on sign-out). Mobile-only command. */
export async function cancelBackgroundSync(): Promise<void> {
	return invoke<void>('cancel_background_sync')
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

/** Permanently delete the user's account (all cloud data + the auth user) and sign out. */
export async function deleteAccount(): Promise<void> {
	return invoke<void>('delete_account')
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
