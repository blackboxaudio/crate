import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MenuAction =
	// App menu
	| 'about'
	| 'settings'
	// File menu
	| 'import_tracks'
	| 'new_playlist'
	| 'new_folder'
	| 'quick_export'
	// Edit menu
	| 'select_all'
	// Playback menu
	| 'play_pause'
	| 'stop'
	| 'jump_to_playing'
	// View menu
	| 'toggle_sidebar'
	// View > Settings submenu
	| 'settings_general'
	| 'settings_library'
	| 'settings_appearance'
	| 'settings_sound'
	| 'settings_diagnostics'
	// Help menu
	| 'documentation'
	| 'report_issue'

/**
 * Listen for menu action events from the native menu bar
 */
export async function onMenuAction(handler: (action: MenuAction) => void): Promise<UnlistenFn> {
	return listen<string>('menu-action', (event) => {
		handler(event.payload as MenuAction)
	})
}
