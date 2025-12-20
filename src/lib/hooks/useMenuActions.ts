import type { UnlistenFn } from '@tauri-apps/api/event'
import { onMenuAction, type MenuAction } from '$lib/api/menu'
import { isInputFocused } from '$lib/utils'

// =============================================================================
// Types
// =============================================================================

export interface MenuActionHandlers {
	onImport: () => Promise<void>
	onCreatePlaylist: () => void
	onCreateFolder: () => void
	onSelectAll: () => void
	onDeselectAll: () => void
	onPlayPause: () => void
	onStop: () => void
	onOpenSettings: () => void
}

// =============================================================================
// Hook
// =============================================================================

/**
 * Set up menu action listener for native menu bar actions.
 *
 * Menu actions:
 * - import_tracks: Import audio files
 * - new_playlist: Create new playlist
 * - new_folder: Create new folder
 * - select_all: Select all tracks (when not typing)
 * - deselect_all: Clear selection
 * - play_pause: Toggle playback (when not typing)
 * - stop: Stop playback
 * - settings: Open settings modal
 * - toggle_sidebar, documentation, report_issue: TODOs
 *
 * @returns Promise of cleanup function to remove the listener
 */
export async function useMenuActions(handlers: MenuActionHandlers): Promise<() => void> {
	const {
		onImport,
		onCreatePlaylist,
		onCreateFolder,
		onSelectAll,
		onDeselectAll,
		onPlayPause,
		onStop,
		onOpenSettings,
	} = handlers

	let unlistenMenu: UnlistenFn | null = null

	function handleMenuAction(action: MenuAction): void {
		switch (action) {
			case 'import_tracks':
				onImport()
				break
			case 'new_playlist':
				onCreatePlaylist()
				break
			case 'new_folder':
				onCreateFolder()
				break
			case 'select_all':
				if (!isInputFocused()) {
					onSelectAll()
				}
				break
			case 'deselect_all':
				onDeselectAll()
				break
			case 'play_pause':
				if (!isInputFocused()) {
					onPlayPause()
				}
				break
			case 'stop':
				onStop()
				break
			case 'toggle_sidebar':
				// TODO: Implement sidebar toggle
				break
			case 'settings':
				onOpenSettings()
				break
			case 'documentation':
				// TODO: Open documentation
				break
			case 'report_issue':
				// TODO: Open issue reporting
				break
		}
	}

	unlistenMenu = await onMenuAction(handleMenuAction)

	return () => {
		unlistenMenu?.()
	}
}
