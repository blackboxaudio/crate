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

/**
 * Set the enabled state of a native menu item by its ID
 */
export async function setMenuItemEnabled(id: string, enabled: boolean): Promise<void> {
	return invoke('set_menu_item_enabled', { id, enabled })
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
	addRelease: string
	newPlaylist: string
	newFolder: string
	quickExport: string
	// Edit menu items
	selectAll: string
	// Playback menu items
	playPause: string
	stop: string
	nextTrack: string
	previousTrack: string
	seekForward: string
	seekBackward: string
	fineSeekForward: string
	fineSeekBackward: string
	volumeUp: string
	volumeDown: string
	mute: string
	jumpToPlaying: string
	// View menu items
	toggleView: string
	toggleEditor: string
	showDevTools: string
	// Settings submenu
	settingsSubmenu: string
	settingsGeneral: string
	settingsLibrary: string
	settingsDiscovery: string
	settingsAppearance: string
	settingsSound: string
	settingsDiagnostics: string
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
