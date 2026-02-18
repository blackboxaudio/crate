import type { UnlistenFn } from '@tauri-apps/api/event'
import { onMenuAction, type MenuAction } from '$lib/api/menu'
import { isInputFocused } from '$lib/utils'
import type { SettingsPage } from '$lib/types'

// =============================================================================
// Types
// =============================================================================

export interface MenuActionHandlers {
	onImport: () => Promise<void>
	onCreatePlaylist: () => void
	onCreateFolder: () => void
	onSelectAll: () => void
	onPlayPause: () => void
	onStop: () => void
	onOpenSettings: (tab?: SettingsPage) => void
	onQuickExport: () => void
	onJumpToPlayingTrack: () => void
	onToggleView: () => void
	onToggleEditor: () => void
}

// =============================================================================
// Hook
// =============================================================================

/**
 * Set up menu action listener for native menu bar actions.
 *
 * Menu actions handled by frontend:
 * - about: Open settings modal (About tab)
 * - settings: Open settings modal
 * - import_tracks: Import audio files
 * - new_playlist: Create new playlist
 * - new_folder: Create new folder
 * - select_all: Select all (text in input, or tracks)
 * - play_pause: Toggle playback (when not typing)
 * - stop: Stop playback
 * - quick_export: Open quick export modal
 * - jump_to_playing: Jump to currently playing track
 * - toggle_editor: Toggle right sidebar editor
 * - documentation, report_issue: TODOs
 *
 * Menu actions handled by backend:
 * - quit: Exit application
 * - minimize: Minimize window
 * - zoom: Toggle maximize window
 *
 * @returns Promise of cleanup function to remove the listener
 */
export async function useMenuActions(handlers: MenuActionHandlers): Promise<() => void> {
	const {
		onImport,
		onCreatePlaylist,
		onCreateFolder,
		onSelectAll,
		onPlayPause,
		onStop,
		onOpenSettings,
		onQuickExport,
		onJumpToPlayingTrack,
		onToggleView,
		onToggleEditor,
	} = handlers

	let unlistenMenu: UnlistenFn | null = null

	function handleMenuAction(action: MenuAction): void {
		switch (action) {
			// App menu
			case 'about':
				onOpenSettings()
				break
			case 'settings':
				onOpenSettings()
				break

			// File menu
			case 'import_tracks':
				onImport()
				break
			case 'new_playlist':
				onCreatePlaylist()
				break
			case 'new_folder':
				onCreateFolder()
				break
			case 'quick_export':
				onQuickExport()
				break

			// Edit menu
			case 'select_all':
				if (isInputFocused()) {
					const input = document.activeElement as HTMLInputElement | HTMLTextAreaElement
					input.select()
				} else {
					onSelectAll()
				}
				break

			// Playback menu
			case 'play_pause':
				if (!isInputFocused()) {
					onPlayPause()
				}
				break
			case 'stop':
				onStop()
				break
			case 'jump_to_playing':
				onJumpToPlayingTrack()
				break

			// View menu
			case 'toggle_view':
				onToggleView()
				break
			case 'toggle_editor':
				onToggleEditor()
				break

			// View > Settings submenu
			case 'settings_general':
				onOpenSettings('general')
				break
			case 'settings_library':
				onOpenSettings('library')
				break
			case 'settings_discovery':
				onOpenSettings('discovery')
				break
			case 'settings_appearance':
				onOpenSettings('appearance')
				break
			case 'settings_sound':
				onOpenSettings('sound')
				break
			case 'settings_diagnostics':
				onOpenSettings('diagnostics')
				break

			// Help menu
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
