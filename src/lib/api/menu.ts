import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MenuAction =
	// App menu
	| 'about'
	| 'settings'
	// File menu
	| 'import_tracks'
	| 'add_release'
	| 'new_playlist'
	| 'new_folder'
	| 'quick_export'
	// Edit menu
	| 'select_all'
	// Playback menu
	| 'play_pause'
	| 'stop'
	| 'next_track'
	| 'previous_track'
	| 'seek_forward'
	| 'seek_backward'
	| 'fine_seek_forward'
	| 'fine_seek_backward'
	| 'volume_up'
	| 'volume_down'
	| 'mute'
	| 'jump_to_playing'
	// View menu
	| 'toggle_view'
	| 'toggle_editor'
	| 'expand_all_releases'
	| 'collapse_all_releases'
	// View > Settings submenu
	| 'settings_general'
	| 'settings_library'
	| 'settings_discovery'
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
