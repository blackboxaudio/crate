import { invoke } from '@tauri-apps/api/core'
import type { AppSettings } from '$lib/types'

/**
 * Get all application settings
 */
export async function getSettings(): Promise<AppSettings> {
	return invoke<AppSettings>('get_settings')
}

/**
 * Set a single setting by key
 */
export async function setSetting(key: string, value: string): Promise<void> {
	return invoke<void>('set_setting', { key, value })
}
