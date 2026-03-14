import { invoke } from '@tauri-apps/api/core'

export interface BackupCounts {
	tracks: number
	cues: number
	tag_categories: number
	tags: number
	playlists: number
	discovery_releases: number
	artwork_files: number
}

export interface BackupInfo {
	version: number
	app_version: string
	created_at: string
	counts: BackupCounts
}

export async function getBackupInfo(path: string): Promise<BackupInfo> {
	return invoke('get_backup_info', { path })
}

export async function createBackup(path: string): Promise<void> {
	return invoke('create_backup', { path })
}

export async function restoreFromBackup(path: string): Promise<void> {
	return invoke('restore_from_backup', { path })
}
