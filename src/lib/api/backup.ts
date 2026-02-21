import { invoke } from '@tauri-apps/api/core'

export async function createBackup(path: string): Promise<void> {
	return invoke('create_backup', { path })
}

export async function restoreFromBackup(path: string): Promise<void> {
	return invoke('restore_from_backup', { path })
}
