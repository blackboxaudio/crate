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

export interface MenuTranslations {
	// Menu titles
	file: string
	edit: string
	playback: string
	view: string
	window: string
	help: string
	// App menu items (about and quit use {appName} placeholder, formatted before sending)
	about: string
	settings: string
	quit: string
	// File menu items
	importTracks: string
	newPlaylist: string
	newFolder: string
	// Edit menu items
	undo: string
	redo: string
	cut: string
	copy: string
	paste: string
	selectAllTracks: string
	// Playback menu items
	playPause: string
	stop: string
	// View menu items
	toggleSidebar: string
	showDevTools: string
	// Window menu items
	minimize: string
	zoom: string
	// Help menu items
	documentation: string
	reportIssue: string
}

/**
 * Rebuild the application menu with translated labels
 */
export async function rebuildMenu(translations: MenuTranslations): Promise<void> {
	return invoke('rebuild_menu', { translations })
}
