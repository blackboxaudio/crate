import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MenuAction =
	// App menu
	| 'about'
	| 'settings'
	// File menu
	| 'import_tracks'
	| 'new_playlist'
	| 'new_folder'
	// Edit menu
	| 'undo'
	| 'redo'
	| 'cut'
	| 'copy'
	| 'paste'
	| 'select_all'
	// Playback menu
	| 'play_pause'
	| 'stop'
	// View menu
	| 'toggle_sidebar'
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
