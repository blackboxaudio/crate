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
