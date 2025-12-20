import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MenuAction =
	| 'import_tracks'
	| 'new_playlist'
	| 'new_folder'
	| 'select_all'
	| 'play_pause'
	| 'stop'
	| 'toggle_sidebar'
	| 'settings'
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
