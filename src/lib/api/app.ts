import { invoke } from '@tauri-apps/api/core'

export interface AppInfo {
	version: string
	environment: string
	isDev: boolean
	dataDir: string
}

/**
 * Get application info including version, environment, and data directory
 */
export async function getAppInfo(): Promise<AppInfo> {
	return invoke<AppInfo>('get_app_info')
}

/**
 * Open browser developer tools (only available in dev mode)
 */
export async function openDevTools(): Promise<void> {
	return invoke('open_dev_tools')
}

/**
 * Close browser developer tools (only available in dev mode)
 */
export async function closeDevTools(): Promise<void> {
	return invoke('close_dev_tools')
}
