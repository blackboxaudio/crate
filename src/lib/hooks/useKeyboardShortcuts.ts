import { isInputFocused } from '$lib/utils'

// =============================================================================
// Types
// =============================================================================

export interface KeyboardShortcutHandlers {
	onPlayPause: () => void
	onFocusSearch: () => void
	onClearSelection: () => void
	onSelectAll: () => void
	onOpenSettings: () => void
	onToggleInspector: () => void
}

// =============================================================================
// Hook
// =============================================================================

/**
 * Set up global keyboard shortcuts for the application.
 *
 * Shortcuts:
 * - Space: toggle play/pause (when not typing)
 * - Cmd/Ctrl+F: focus search input
 * - Escape: clear selection
 * - Cmd/Ctrl+A: select all (text in input, or tracks)
 * - Cmd/Ctrl+,: open settings
 * - Cmd/Ctrl+I: toggle track inspector
 *
 * @returns Cleanup function to remove the event listener
 */
export function useKeyboardShortcuts(handlers: KeyboardShortcutHandlers): () => void {
	const { onPlayPause, onFocusSearch, onClearSelection, onSelectAll, onOpenSettings, onToggleInspector } = handlers

	function handleKeydown(e: KeyboardEvent): void {
		// Space: toggle play/pause
		if (e.code === 'Space' && !isInputFocused()) {
			e.preventDefault()
			onPlayPause()
		}

		// Cmd/Ctrl+F: focus search
		if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
			e.preventDefault()
			onFocusSearch()
		}

		// Escape: clear selection
		if (e.key === 'Escape') {
			onClearSelection()
		}

		// Cmd/Ctrl+A: select all (text in input, or tracks)
		if ((e.metaKey || e.ctrlKey) && e.key === 'a') {
			e.preventDefault()
			if (isInputFocused()) {
				const input = document.activeElement as HTMLInputElement | HTMLTextAreaElement
				input.select()
			} else {
				onSelectAll()
			}
		}

		// Cmd/Ctrl+,: open settings
		if ((e.metaKey || e.ctrlKey) && e.key === ',') {
			e.preventDefault()
			onOpenSettings()
		}

		// Cmd/Ctrl+I: toggle track inspector
		if ((e.metaKey || e.ctrlKey) && e.key === 'i') {
			e.preventDefault()
			onToggleInspector()
		}
	}

	window.addEventListener('keydown', handleKeydown)

	return () => {
		window.removeEventListener('keydown', handleKeydown)
	}
}
