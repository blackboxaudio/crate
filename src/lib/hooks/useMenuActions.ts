import type { UnlistenFn } from '@tauri-apps/api/event'
import { get } from 'svelte/store'
import { onMenuAction, type MenuAction } from '$lib/api/menu'
import { uiStore } from '$lib/stores'
import { isInputFocused, isNativeDialogOpen } from '$lib/utils'
import type { SettingsPage } from '$lib/types'

// =============================================================================
// Types
// =============================================================================

export interface MenuActionHandlers {
	onImport: () => Promise<void>
	onAddRelease: () => void
	onCreatePlaylist: () => void
	onCreateFolder: () => void
	onSelectAll: () => void
	onPlayPause: () => void
	onStop: () => void
	onNextTrack: () => void
	onPreviousTrack: () => void
	onSeekForward: () => void
	onSeekBackward: () => void
	onFineSeekForward: () => void
	onFineSeekBackward: () => void
	onVolumeUp: () => void
	onVolumeDown: () => void
	onToggleMute: () => void
	onOpenSettings: (tab?: SettingsPage) => void
	onQuickExport: () => void
	onJumpToPlayingTrack: () => void
	onToggleView: () => void
	onToggleEditor: () => void
	onExpandAllReleases: () => void
	onCollapseAllReleases: () => void
	onRefreshMetadata: () => void
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
		onAddRelease,
		onCreatePlaylist,
		onCreateFolder,
		onSelectAll,
		onPlayPause,
		onStop,
		onNextTrack,
		onPreviousTrack,
		onSeekForward,
		onSeekBackward,
		onFineSeekForward,
		onFineSeekBackward,
		onVolumeUp,
		onVolumeDown,
		onToggleMute,
		onOpenSettings,
		onQuickExport,
		onJumpToPlayingTrack,
		onToggleView,
		onToggleEditor,
		onExpandAllReleases,
		onCollapseAllReleases,
		onRefreshMetadata,
	} = handlers

	let unlistenMenu: UnlistenFn | null = null

	function handleMenuAction(action: MenuAction): void {
		if (isNativeDialogOpen()) return

		// During onboarding, the 'about' action is handled by the layout's own listener
		// to show a simple about dialog instead of the full settings modal
		if (action === 'about' && get(uiStore).isOnboarding) return

		switch (action) {
			// App menu
			case 'about':
				onOpenSettings('about')
				break
			case 'settings':
				onOpenSettings()
				break

			// File menu
			case 'import_tracks':
				onImport()
				break
			case 'add_release':
				onAddRelease()
				break
			case 'refresh_metadata':
				onRefreshMetadata()
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
			case 'next_track':
				onNextTrack()
				break
			case 'previous_track':
				onPreviousTrack()
				break
			case 'seek_forward':
				onSeekForward()
				break
			case 'seek_backward':
				onSeekBackward()
				break
			case 'fine_seek_forward':
				onFineSeekForward()
				break
			case 'fine_seek_backward':
				onFineSeekBackward()
				break
			case 'volume_up':
				onVolumeUp()
				break
			case 'volume_down':
				onVolumeDown()
				break
			case 'mute':
				onToggleMute()
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
			case 'expand_all_releases':
				onExpandAllReleases()
				break
			case 'collapse_all_releases':
				onCollapseAllReleases()
				break

			// View > Settings submenu
			case 'settings_general':
				onOpenSettings('general')
				break
			case 'settings_appearance':
				onOpenSettings('appearance')
				break
			case 'settings_library':
				onOpenSettings('library')
				break
			case 'settings_discovery':
				onOpenSettings('discovery')
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
